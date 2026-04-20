//! Work queue dispatcher - polls business queues and creates executions.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use attune_common::{
    crypto::decrypt_json,
    models::{
        action::Action,
        enums::{ExecutionStatus, WorkQueueDispatchStatus, WorkQueueItemStatus},
        key::Key,
        pack::Pack,
        work_queue::{WorkQueue, WorkQueueConfig, WorkQueueItem, WorkQueueTunableValue},
    },
    mq::{ExecutionRequestedPayload, MessageEnvelope, MessageType, Publisher},
    repositories::{
        action::ActionRepository,
        execution::{CreateExecutionInput, ExecutionRepository},
        key::KeyRepository,
        pack::PackRepository,
        work_queue::{
            CreateWorkQueueDispatchInput, LeaseWorkQueueItemsInput, ReleaseWorkQueueLeaseInput,
            UpdateWorkQueueDispatchInput, WorkQueueDispatchRepository,
            WorkQueueDispatchSearchFilters, WorkQueueItemRepository, WorkQueueItemSearchFilters,
            WorkQueueRepository, WorkQueueSearchFilters,
        },
        Create, FindById, FindByRef, Update,
    },
};
use chrono::Utc;
use serde_json::{json, Map, Value as JsonValue};
use sqlx::PgPool;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_LEASE_DURATION: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_QUEUES_PER_POLL: u32 = 1_000;
const MAX_LEASED_DISPATCHES_PER_POLL: u32 = 1_000;

#[derive(Debug, Clone)]
struct QueueDispatcherConfig {
    poll_interval: Duration,
    lease_duration: Duration,
}

impl Default for QueueDispatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_POLL_INTERVAL,
            lease_duration: DEFAULT_LEASE_DURATION,
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedQueueContext {
    action: Action,
    parsed_config: WorkQueueConfig,
    concurrency: u32,
    batch_size: u32,
}

#[derive(Debug, Clone)]
struct PreparedDispatch {
    dispatch_id: i64,
    execution_id: i64,
    action_id: Option<i64>,
    action_ref: String,
    config: Option<JsonValue>,
}

/// Polling dispatcher for first-class work queues.
pub struct WorkQueueDispatcher {
    pool: PgPool,
    publisher: Arc<Publisher>,
    encryption_key: Option<String>,
    config: QueueDispatcherConfig,
}

impl WorkQueueDispatcher {
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, encryption_key: Option<String>) -> Self {
        Self {
            pool,
            publisher,
            encryption_key,
            config: QueueDispatcherConfig::default(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting work queue dispatcher (poll interval: {:?}, lease duration: {:?})",
            self.config.poll_interval, self.config.lease_duration
        );

        loop {
            if let Err(error) = self.poll_once().await {
                error!("Work queue dispatch poll failed: {error:#}");
            }
            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    async fn poll_once(&self) -> Result<()> {
        self.reconcile_leased_dispatches().await?;

        let queues = WorkQueueRepository::search(
            &self.pool,
            &WorkQueueSearchFilters {
                enabled: Some(true),
                limit: MAX_QUEUES_PER_POLL,
                ..Default::default()
            },
        )
        .await?;

        for queue in queues.rows {
            if let Err(error) = self.dispatch_ready_batches_for_queue(&queue).await {
                error!(
                    "Failed to dispatch queued work for queue '{}' ({}): {error:#}",
                    queue.r#ref, queue.id
                );
            }
        }

        Ok(())
    }

    async fn reconcile_leased_dispatches(&self) -> Result<()> {
        let leased_dispatches = WorkQueueDispatchRepository::search(
            &self.pool,
            &WorkQueueDispatchSearchFilters {
                statuses: Some(vec![WorkQueueDispatchStatus::Leased]),
                limit: MAX_LEASED_DISPATCHES_PER_POLL,
                ..Default::default()
            },
        )
        .await?;

        for dispatch in leased_dispatches.rows {
            let execution = ExecutionRepository::find_by_id(&self.pool, dispatch.execution).await?;

            let Some(execution) = execution else {
                warn!(
                    "Queue dispatch {} references missing execution {}, releasing leased items",
                    dispatch.id, dispatch.execution
                );
                Self::release_orphaned_dispatch_items(&self.pool, dispatch.execution).await?;
                WorkQueueDispatchRepository::update(
                    &self.pool,
                    dispatch.id,
                    UpdateWorkQueueDispatchInput {
                        status: Some(WorkQueueDispatchStatus::Released),
                        ..Default::default()
                    },
                )
                .await?;
                continue;
            };

            if execution.status == ExecutionStatus::Requested {
                let prepared = PreparedDispatch {
                    dispatch_id: dispatch.id,
                    execution_id: execution.id,
                    action_id: execution.action,
                    action_ref: execution.action_ref.clone(),
                    config: execution.config.clone(),
                };
                self.publish_dispatch(prepared).await?;
                continue;
            }

            WorkQueueDispatchRepository::update(
                &self.pool,
                dispatch.id,
                UpdateWorkQueueDispatchInput {
                    status: Some(WorkQueueDispatchStatus::Dispatched),
                    ..Default::default()
                },
            )
            .await?;
        }

        Ok(())
    }

    async fn release_orphaned_dispatch_items(pool: &PgPool, execution_id: i64) -> Result<()> {
        let items = WorkQueueItemRepository::search(
            pool,
            &WorkQueueItemSearchFilters {
                leased_execution: Some(execution_id),
                statuses: Some(vec![WorkQueueItemStatus::Leased]),
                limit: 1_000,
                ..Default::default()
            },
        )
        .await?;

        let mut lease_tokens = BTreeMap::new();
        for item in items.rows {
            if let Some(token) = item.lease_token {
                lease_tokens.insert(token, ());
            }
        }

        for lease_token in lease_tokens.into_keys() {
            WorkQueueItemRepository::release_lease(
                pool,
                ReleaseWorkQueueLeaseInput {
                    lease_token,
                    new_status: WorkQueueItemStatus::Retry,
                    leased_execution: None,
                    last_error: Some(json!({
                        "code": "dispatch_execution_missing",
                        "message": format!(
                            "queue dispatch execution {} was missing during recovery",
                            execution_id
                        ),
                    })),
                    ack_summary: None,
                },
            )
            .await?;
        }

        Ok(())
    }

    async fn dispatch_ready_batches_for_queue(&self, queue: &WorkQueue) -> Result<()> {
        let context =
            Self::resolve_queue_context(&self.pool, self.encryption_key.as_deref(), queue).await?;
        let active_dispatches = WorkQueueDispatchRepository::search(
            &self.pool,
            &WorkQueueDispatchSearchFilters {
                queue: Some(queue.id),
                statuses: Some(vec![
                    WorkQueueDispatchStatus::Leased,
                    WorkQueueDispatchStatus::Dispatched,
                ]),
                limit: 1,
                ..Default::default()
            },
        )
        .await?
        .total;

        let active_dispatches = active_dispatches.min(u64::from(u32::MAX)) as u32;
        if active_dispatches >= context.concurrency {
            debug!(
                "Queue '{}' is at dispatch capacity ({}/{})",
                queue.r#ref, active_dispatches, context.concurrency
            );
            return Ok(());
        }

        let open_slots = context.concurrency - active_dispatches;
        for _ in 0..open_slots {
            let Some(dispatch) = Self::prepare_next_dispatch(
                &self.pool,
                queue,
                &context,
                self.config.lease_duration,
            )
            .await?
            else {
                break;
            };

            self.publish_dispatch(dispatch).await?;
        }

        Ok(())
    }

    async fn resolve_queue_context(
        pool: &PgPool,
        encryption_key: Option<&str>,
        queue: &WorkQueue,
    ) -> Result<ResolvedQueueContext> {
        let action = if let Some(action_id) = queue.dispatch_action {
            ActionRepository::find_by_id(pool, action_id).await?
        } else {
            None
        };
        let action = match action {
            Some(action) => Some(action),
            None => ActionRepository::find_by_ref(pool, &queue.dispatch_action_ref).await?,
        }
        .ok_or_else(|| {
            anyhow!(
                "dispatch action '{}' for queue '{}' no longer exists",
                queue.dispatch_action_ref,
                queue.r#ref
            )
        })?;

        let pack = if let Some(pack_id) = queue.pack {
            PackRepository::find_by_id(pool, pack_id).await?
        } else if let Some(pack_ref) = &queue.pack_ref {
            PackRepository::find_by_ref(pool, pack_ref).await?
        } else {
            None
        };

        let parsed_config: WorkQueueConfig = serde_json::from_value(queue.config.clone())
            .with_context(|| format!("invalid config persisted for queue '{}'", queue.r#ref))?;

        let concurrency = Self::resolve_u32_tunable(
            pool,
            encryption_key,
            pack.as_ref(),
            parsed_config
                .dispatch
                .as_ref()
                .and_then(|dispatch| dispatch.concurrency.as_ref()),
            1,
            "concurrency",
            queue,
        )
        .await?;

        let batch_size_default =
            if queue.batch_mode == attune_common::models::WorkQueueBatchMode::Single {
                1
            } else {
                1
            };
        let batch_size = if queue.batch_mode == attune_common::models::WorkQueueBatchMode::Single {
            1
        } else {
            Self::resolve_u32_tunable(
                pool,
                encryption_key,
                pack.as_ref(),
                parsed_config
                    .dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.batch_size.as_ref()),
                batch_size_default,
                "batch_size",
                queue,
            )
            .await?
        };

        Ok(ResolvedQueueContext {
            action,
            parsed_config,
            concurrency,
            batch_size,
        })
    }

    async fn resolve_u32_tunable(
        pool: &PgPool,
        encryption_key: Option<&str>,
        pack: Option<&Pack>,
        tunable: Option<&WorkQueueTunableValue>,
        default: u32,
        name: &str,
        queue: &WorkQueue,
    ) -> Result<u32> {
        let Some(tunable) = tunable else {
            return Ok(default);
        };

        let resolved = Self::resolve_tunable_value(pool, encryption_key, pack, tunable).await?;
        let parsed_resolved = resolved.as_ref().and_then(Self::parse_positive_u32);
        let parsed = parsed_resolved
            .or_else(|| tunable.fallback.as_ref().and_then(Self::parse_positive_u32))
            .unwrap_or(default);

        if parsed == default && resolved.is_some() && parsed_resolved.is_none() {
            warn!(
                "Queue '{}' resolved non-positive/non-integer {} tunable {}, falling back to {}",
                queue.r#ref,
                name,
                resolved.as_ref().expect("resolved is_some() checked above"),
                default
            );
        }

        Ok(parsed)
    }

    async fn resolve_tunable_value(
        pool: &PgPool,
        encryption_key: Option<&str>,
        pack: Option<&Pack>,
        tunable: &WorkQueueTunableValue,
    ) -> Result<Option<JsonValue>> {
        use attune_common::models::WorkQueueTunableSource;

        let primary = match tunable.source {
            WorkQueueTunableSource::Literal => tunable.value.clone(),
            WorkQueueTunableSource::PackConfig => pack.and_then(|pack| {
                tunable
                    .path
                    .as_deref()
                    .and_then(|path| Self::json_path_get(&pack.config, path).cloned())
            }),
            WorkQueueTunableSource::Keystore => {
                let Some(key_ref) = tunable.key_ref.as_deref() else {
                    return Ok(tunable.fallback.clone());
                };
                let key = KeyRepository::find_by_ref(pool, key_ref).await?;
                match key {
                    Some(key) => {
                        let value = Self::resolve_key_value(&key, encryption_key)?;
                        if let Some(path) = tunable.path.as_deref() {
                            Self::json_path_get(&value, path).cloned()
                        } else {
                            Some(value)
                        }
                    }
                    None => None,
                }
            }
        };

        Ok(primary.or_else(|| tunable.fallback.clone()))
    }

    fn resolve_key_value(key: &Key, encryption_key: Option<&str>) -> Result<JsonValue> {
        if !key.encrypted {
            return Ok(key.value.clone());
        }

        let encryption_key = encryption_key.ok_or_else(|| {
            anyhow!(
                "encrypted key '{}' requires security.encryption_key for queue dispatch",
                key.r#ref
            )
        })?;
        decrypt_json(&key.value, encryption_key)
            .with_context(|| format!("failed to decrypt queue tunable key '{}'", key.r#ref))
    }

    fn parse_positive_u32(value: &JsonValue) -> Option<u32> {
        match value {
            JsonValue::Number(number) => number
                .as_u64()
                .or_else(|| number.as_i64().and_then(|value| u64::try_from(value).ok()))
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0),
            JsonValue::String(value) => value.parse::<u32>().ok().filter(|value| *value > 0),
            _ => None,
        }
    }

    async fn prepare_next_dispatch(
        pool: &PgPool,
        queue: &WorkQueue,
        context: &ResolvedQueueContext,
        lease_duration: Duration,
    ) -> Result<Option<PreparedDispatch>> {
        let batch_limit = if queue.batch_mode == attune_common::models::WorkQueueBatchMode::Single {
            1
        } else {
            context.batch_size.max(1)
        };

        let lease_token = Uuid::new_v4();
        let lease_expires_at = Utc::now()
            + chrono::Duration::from_std(lease_duration)
                .unwrap_or_else(|_| chrono::Duration::hours(24));

        let mut tx = pool.begin().await?;
        let items = WorkQueueItemRepository::lease_next_batch(
            &mut *tx,
            LeaseWorkQueueItemsInput {
                queue: queue.id,
                ready_statuses: vec![WorkQueueItemStatus::Queued, WorkQueueItemStatus::Retry],
                limit: i64::from(batch_limit),
                leased_execution: None,
                lease_token,
                lease_expires_at,
            },
        )
        .await?;

        if items.is_empty() {
            tx.commit().await?;
            return Ok(None);
        }

        let execution_config = Self::build_execution_config(queue, &context.parsed_config, &items)?;
        let execution = ExecutionRepository::create(
            &mut *tx,
            CreateExecutionInput {
                action: Some(context.action.id),
                action_ref: context.action.r#ref.clone(),
                config: Some(execution_config),
                env_vars: None,
                parent: None,
                enforcement: None,
                executor: None,
                worker: None,
                status: ExecutionStatus::Requested,
                result: None,
                workflow_task: None,
            },
        )
        .await?;

        WorkQueueItemRepository::attach_execution_to_lease(&mut *tx, lease_token, execution.id)
            .await?;

        let dispatch = WorkQueueDispatchRepository::create(
            &mut *tx,
            CreateWorkQueueDispatchInput {
                queue: queue.id,
                queue_ref: queue.r#ref.clone(),
                execution: execution.id,
                status: WorkQueueDispatchStatus::Leased,
                leased_item_count: items.len() as i32,
            },
        )
        .await?;

        tx.commit().await?;

        Ok(Some(PreparedDispatch {
            dispatch_id: dispatch.id,
            execution_id: execution.id,
            action_id: Some(context.action.id),
            action_ref: context.action.r#ref.clone(),
            config: execution.config.clone(),
        }))
    }

    fn build_execution_config(
        queue: &WorkQueue,
        parsed_config: &WorkQueueConfig,
        items: &[WorkQueueItem],
    ) -> Result<JsonValue> {
        if items.is_empty() {
            return Err(anyhow!(
                "cannot build execution config for queue '{}' without leased items",
                queue.r#ref
            ));
        }

        let input_mapping = parsed_config.input_mapping.as_ref();
        let include_queue_metadata = input_mapping
            .map(|mapping| mapping.include_queue_metadata)
            .unwrap_or(false);

        match queue.batch_mode {
            attune_common::models::WorkQueueBatchMode::Single => {
                let item = items
                    .first()
                    .ok_or_else(|| anyhow!("single-item queue dispatch had no leased item"))?;
                Ok(Self::build_single_item_config(
                    queue,
                    parsed_config,
                    item,
                    include_queue_metadata,
                ))
            }
            attune_common::models::WorkQueueBatchMode::Batch => Ok(Self::build_batch_config(
                queue,
                parsed_config,
                items,
                include_queue_metadata,
            )),
        }
    }

    fn build_single_item_config(
        queue: &WorkQueue,
        parsed_config: &WorkQueueConfig,
        item: &WorkQueueItem,
        include_queue_metadata: bool,
    ) -> JsonValue {
        let single_item_path = parsed_config
            .input_mapping
            .as_ref()
            .and_then(|mapping| mapping.single_item_path.as_deref());

        let mut root = if single_item_path.is_none() && item.payload.is_object() {
            item.payload.clone()
        } else {
            json!({})
        };

        if let Some(path) = single_item_path {
            Self::json_path_set(&mut root, path, item.payload.clone());
        } else if !item.payload.is_object() {
            Self::json_path_set(&mut root, "item", item.payload.clone());
        }

        if include_queue_metadata {
            Self::json_path_set(
                &mut root,
                "queue",
                Self::build_queue_metadata(queue, parsed_config, std::slice::from_ref(item)),
            );
        }

        root
    }

    fn build_batch_config(
        queue: &WorkQueue,
        parsed_config: &WorkQueueConfig,
        items: &[WorkQueueItem],
        include_queue_metadata: bool,
    ) -> JsonValue {
        let items_path = parsed_config
            .input_mapping
            .as_ref()
            .and_then(|mapping| mapping.items_path.as_deref())
            .unwrap_or("items");

        let mut root = json!({});
        let payloads = JsonValue::Array(items.iter().map(|item| item.payload.clone()).collect());
        Self::json_path_set(&mut root, items_path, payloads);

        if include_queue_metadata {
            Self::json_path_set(
                &mut root,
                "queue",
                Self::build_queue_metadata(queue, parsed_config, items),
            );
        }

        root
    }

    fn build_queue_metadata(
        queue: &WorkQueue,
        parsed_config: &WorkQueueConfig,
        items: &[WorkQueueItem],
    ) -> JsonValue {
        let ack_version = parsed_config
            .ack_contract
            .as_ref()
            .map(|ack| ack.version)
            .unwrap_or(1);

        json!({
            "id": queue.id,
            "ref": queue.r#ref,
            "batch_mode": queue.batch_mode,
            "leased_item_count": items.len(),
            "ack_contract_version": ack_version,
            "items": items.iter().map(|item| {
                json!({
                    "id": item.id,
                    "item_key": item.item_key,
                    "priority": item.priority,
                    "metadata": item.metadata,
                    "enqueue_source": item.enqueue_source,
                    "attempt_count": item.attempt_count,
                    "requested_by_identity": item.requested_by_identity,
                    "requested_by_execution": item.requested_by_execution,
                    "requested_by_enforcement": item.requested_by_enforcement,
                })
            }).collect::<Vec<_>>(),
        })
    }

    async fn publish_dispatch(&self, dispatch: PreparedDispatch) -> Result<()> {
        let payload = ExecutionRequestedPayload {
            execution_id: dispatch.execution_id,
            action_id: dispatch.action_id,
            action_ref: dispatch.action_ref,
            parent_id: None,
            enforcement_id: None,
            config: dispatch.config,
        };

        let envelope = MessageEnvelope::new(MessageType::ExecutionRequested, payload)
            .with_source("executor-queue-dispatcher");
        self.publisher.publish_envelope(&envelope).await?;

        WorkQueueDispatchRepository::update(
            &self.pool,
            dispatch.dispatch_id,
            UpdateWorkQueueDispatchInput {
                status: Some(WorkQueueDispatchStatus::Dispatched),
                ..Default::default()
            },
        )
        .await?;

        debug!(
            "Dispatched work queue execution {} via dispatch record {}",
            dispatch.execution_id, dispatch.dispatch_id
        );

        Ok(())
    }

    fn json_path_get<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
        let mut current = value;
        for segment in path.split('.').filter(|segment| !segment.is_empty()) {
            current = match current {
                JsonValue::Object(object) => object.get(segment)?,
                JsonValue::Array(array) => {
                    let index = segment.parse::<usize>().ok()?;
                    array.get(index)?
                }
                _ => return None,
            };
        }
        Some(current)
    }

    fn json_path_set(target: &mut JsonValue, path: &str, value: JsonValue) {
        if path.trim().is_empty() {
            *target = value;
            return;
        }

        let mut segments = path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .peekable();
        if segments.peek().is_none() {
            *target = value;
            return;
        }

        if !target.is_object() {
            *target = JsonValue::Object(Map::new());
        }

        let mut current = target;
        let mut value = Some(value);
        while let Some(segment) = segments.next() {
            let is_last = segments.peek().is_none();
            if is_last {
                if let JsonValue::Object(object) = current {
                    object.insert(
                        segment.to_string(),
                        value.take().expect("value only inserted once"),
                    );
                }
                return;
            }

            if !current.is_object() {
                *current = JsonValue::Object(Map::new());
            }

            let object = current
                .as_object_mut()
                .expect("current value must be an object");
            current = object
                .entry(segment.to_string())
                .or_insert_with(|| JsonValue::Object(Map::new()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::models::{enums::OwnerType, key::Key, WorkQueueBatchMode};

    fn sample_queue(batch_mode: WorkQueueBatchMode, config: JsonValue) -> WorkQueue {
        WorkQueue {
            id: 1,
            r#ref: "core.inbox".to_string(),
            pack: Some(10),
            pack_ref: Some("core".to_string()),
            is_adhoc: false,
            label: "Inbox".to_string(),
            description: None,
            enabled: true,
            dispatch_action: Some(11),
            dispatch_action_ref: "core.process".to_string(),
            default_priority: 0,
            allow_pending_update: false,
            update_strategy: attune_common::models::WorkQueueUpdateStrategy::Replace,
            batch_mode,
            config,
            created: Utc::now(),
            updated: Utc::now(),
        }
    }

    fn sample_item(id: i64, priority: i32, payload: JsonValue) -> WorkQueueItem {
        WorkQueueItem {
            id,
            queue: 1,
            queue_ref: "core.inbox".to_string(),
            item_key: Some(format!("item-{id}")),
            priority,
            status: WorkQueueItemStatus::Queued,
            payload,
            metadata: json!({"source": "test"}),
            enqueue_source: "test".to_string(),
            requested_by_identity: None,
            requested_by_execution: None,
            requested_by_enforcement: None,
            leased_execution: None,
            lease_token: None,
            lease_expires_at: None,
            attempt_count: 0,
            last_error: None,
            ack_summary: None,
            created: Utc::now(),
            updated: Utc::now(),
        }
    }

    #[test]
    fn build_batch_execution_config_maps_items_and_queue_metadata() {
        let queue = sample_queue(
            WorkQueueBatchMode::Batch,
            json!({
                "input_mapping": {
                    "items_path": "payload.items",
                    "include_queue_metadata": true
                },
                "ack_contract": {
                    "version": 2
                }
            }),
        );
        let parsed_config: WorkQueueConfig = serde_json::from_value(queue.config.clone()).unwrap();
        let items = vec![
            sample_item(1001, 10, json!({"order_id": 1001})),
            sample_item(1002, 5, json!({"order_id": 1002})),
        ];

        let config = WorkQueueDispatcher::build_execution_config(&queue, &parsed_config, &items)
            .expect("config should build");

        assert_eq!(
            WorkQueueDispatcher::json_path_get(&config, "payload.items"),
            Some(&json!([{"order_id": 1001}, {"order_id": 1002}]))
        );
        assert_eq!(
            WorkQueueDispatcher::json_path_get(&config, "queue.ack_contract_version")
                .and_then(JsonValue::as_i64),
            Some(2)
        );
        assert_eq!(
            WorkQueueDispatcher::json_path_get(&config, "queue.items.0.id")
                .and_then(JsonValue::as_i64),
            Some(1001)
        );
    }

    #[test]
    fn build_single_execution_config_preserves_object_payload_without_mapping() {
        let queue = sample_queue(WorkQueueBatchMode::Single, json!({}));
        let parsed_config: WorkQueueConfig = serde_json::from_value(queue.config.clone()).unwrap();
        let item = sample_item(42, 1, json!({"customer": "alice", "order_id": 42}));

        let config =
            WorkQueueDispatcher::build_execution_config(&queue, &parsed_config, &[item]).unwrap();

        assert_eq!(
            config,
            json!({
                "customer": "alice",
                "order_id": 42
            })
        );
    }

    #[test]
    fn resolve_key_value_decrypts_encrypted_tunables() {
        let encryption_key = "a".repeat(32);
        let encrypted = attune_common::crypto::encrypt_json(
            &json!({"dispatch": {"batch_size": 3}}),
            &encryption_key,
        )
        .expect("value should encrypt");
        let key = Key {
            id: 1,
            r#ref: "core.dispatch".to_string(),
            owner_type: OwnerType::System,
            owner: None,
            owner_identity: None,
            owner_pack: None,
            owner_pack_ref: None,
            owner_action: None,
            owner_action_ref: None,
            owner_sensor: None,
            owner_sensor_ref: None,
            name: "Dispatch".to_string(),
            encrypted: true,
            encryption_key_hash: Some("test".to_string()),
            value: encrypted,
            created: Utc::now(),
            updated: Utc::now(),
        };

        let decrypted =
            WorkQueueDispatcher::resolve_key_value(&key, Some(encryption_key.as_str())).unwrap();

        assert_eq!(decrypted, json!({"dispatch": {"batch_size": 3}}));
    }

    #[test]
    fn resolve_key_value_requires_encryption_key_for_encrypted_values() {
        let encryption_key = "a".repeat(32);
        let encrypted = attune_common::crypto::encrypt_json(
            &json!({"dispatch": {"batch_size": 3}}),
            &encryption_key,
        )
        .expect("value should encrypt");
        let key = Key {
            id: 1,
            r#ref: "core.dispatch".to_string(),
            owner_type: OwnerType::System,
            owner: None,
            owner_identity: None,
            owner_pack: None,
            owner_pack_ref: None,
            owner_action: None,
            owner_action_ref: None,
            owner_sensor: None,
            owner_sensor_ref: None,
            name: "Dispatch".to_string(),
            encrypted: true,
            encryption_key_hash: Some("test".to_string()),
            value: encrypted,
            created: Utc::now(),
            updated: Utc::now(),
        };

        let error = WorkQueueDispatcher::resolve_key_value(&key, None).unwrap_err();
        assert!(error
            .to_string()
            .contains("requires security.encryption_key"));
    }

    #[test]
    fn build_batch_execution_config_defaults_to_items_path_without_metadata() {
        let queue = sample_queue(WorkQueueBatchMode::Batch, json!({}));
        let parsed_config: WorkQueueConfig = serde_json::from_value(queue.config.clone()).unwrap();
        let items = vec![
            sample_item(1, 10, json!({"order_id": 1})),
            sample_item(2, 5, json!({"order_id": 2})),
        ];

        let config = WorkQueueDispatcher::build_execution_config(&queue, &parsed_config, &items)
            .expect("config should build");

        assert_eq!(
            config,
            json!({
                "items": [
                    {"order_id": 1},
                    {"order_id": 2}
                ]
            })
        );
    }

    #[test]
    fn json_path_helpers_support_nested_objects_and_arrays() {
        let mut value = json!({});
        WorkQueueDispatcher::json_path_set(&mut value, "payload.items", json!([1, 2, 3]));

        assert_eq!(
            WorkQueueDispatcher::json_path_get(&value, "payload.items.1"),
            Some(&json!(2))
        );
    }
}
