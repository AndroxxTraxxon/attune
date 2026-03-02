//! Trigger and Sensor repository for database operations
//!
//! This module provides CRUD operations and queries for Trigger and Sensor entities.

use crate::models::{trigger::*, Id, JsonSchema};
use crate::Result;
use serde_json::Value as JsonValue;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

// ============================================================================
// Trigger Search
// ============================================================================

/// Filters for [`TriggerRepository::list_search`].
///
/// All fields are optional and combinable (AND). Pagination is always applied.
#[derive(Debug, Clone, Default)]
pub struct TriggerSearchFilters {
    /// Filter by pack ID
    pub pack: Option<Id>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
    pub limit: u32,
    pub offset: u32,
}

/// Result of [`TriggerRepository::list_search`].
#[derive(Debug)]
pub struct TriggerSearchResult {
    pub rows: Vec<Trigger>,
    pub total: u64,
}

// ============================================================================
// Sensor Search
// ============================================================================

/// Filters for [`SensorRepository::list_search`].
///
/// All fields are optional and combinable (AND). Pagination is always applied.
#[derive(Debug, Clone, Default)]
pub struct SensorSearchFilters {
    /// Filter by pack ID
    pub pack: Option<Id>,
    /// Filter by trigger ID
    pub trigger: Option<Id>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
    pub limit: u32,
    pub offset: u32,
}

/// Result of [`SensorRepository::list_search`].
#[derive(Debug)]
pub struct SensorSearchResult {
    pub rows: Vec<Sensor>,
    pub total: u64,
}

/// Repository for Trigger operations
pub struct TriggerRepository;

impl Repository for TriggerRepository {
    type Entity = Trigger;

    fn table_name() -> &'static str {
        "triggers"
    }
}

/// Input for creating a new trigger
#[derive(Debug, Clone)]
pub struct CreateTriggerInput {
    pub r#ref: String,
    pub pack: Option<Id>,
    pub pack_ref: Option<String>,
    pub label: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub param_schema: Option<JsonSchema>,
    pub out_schema: Option<JsonSchema>,
    pub is_adhoc: bool,
}

/// Input for updating a trigger
#[derive(Debug, Clone, Default)]
pub struct UpdateTriggerInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub param_schema: Option<JsonSchema>,
    pub out_schema: Option<JsonSchema>,
}

#[async_trait::async_trait]
impl FindById for TriggerRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let trigger = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(trigger)
    }
}

#[async_trait::async_trait]
impl FindByRef for TriggerRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let trigger = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(trigger)
    }
}

#[async_trait::async_trait]
impl List for TriggerRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let triggers = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(triggers)
    }
}

#[async_trait::async_trait]
impl Create for TriggerRepository {
    type CreateInput = CreateTriggerInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let trigger = sqlx::query_as::<_, Trigger>(
            r#"
            INSERT INTO trigger (ref, pack, pack_ref, label, description, enabled,
                                 param_schema, out_schema, is_adhoc)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, ref, pack, pack_ref, label, description, enabled,
                      param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                      is_adhoc, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(&input.label)
        .bind(&input.description)
        .bind(input.enabled)
        .bind(&input.param_schema)
        .bind(&input.out_schema)
        .bind(input.is_adhoc)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            // Convert unique constraint violation to AlreadyExists error
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return crate::Error::already_exists("Trigger", "ref", &input.r#ref);
                }
            }
            e.into()
        })?;

        Ok(trigger)
    }
}

#[async_trait::async_trait]
impl Update for TriggerRepository {
    type UpdateInput = UpdateTriggerInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE trigger SET ");
        let mut has_updates = false;

        if let Some(label) = &input.label {
            query.push("label = ");
            query.push_bind(label);
            has_updates = true;
        }

        if let Some(description) = &input.description {
            if has_updates {
                query.push(", ");
            }
            query.push("description = ");
            query.push_bind(description);
            has_updates = true;
        }

        if let Some(enabled) = input.enabled {
            if has_updates {
                query.push(", ");
            }
            query.push("enabled = ");
            query.push_bind(enabled);
            has_updates = true;
        }

        if let Some(param_schema) = &input.param_schema {
            if has_updates {
                query.push(", ");
            }
            query.push("param_schema = ");
            query.push_bind(param_schema);
            has_updates = true;
        }

        if let Some(out_schema) = &input.out_schema {
            if has_updates {
                query.push(", ");
            }
            query.push("out_schema = ");
            query.push_bind(out_schema);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, label, description, enabled, param_schema, out_schema, webhook_enabled, webhook_key, webhook_config, is_adhoc, created, updated");

        let trigger = query
            .build_query_as::<Trigger>()
            .fetch_one(executor)
            .await
            .map_err(|e| {
                // Convert RowNotFound to NotFound error
                if matches!(e, sqlx::Error::RowNotFound) {
                    return crate::Error::not_found("trigger", "id", &id.to_string());
                }
                e.into()
            })?;

        Ok(trigger)
    }
}

#[async_trait::async_trait]
impl Delete for TriggerRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM trigger WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl TriggerRepository {
    /// Search triggers with all filters pushed into SQL.
    ///
    /// All filter fields are combinable (AND). Pagination is server-side.
    pub async fn list_search<'e, E>(
        db: E,
        filters: &TriggerSearchFilters,
    ) -> Result<TriggerSearchResult>
    where
        E: Executor<'e, Database = Postgres> + Copy + 'e,
    {
        let select_cols = "id, ref, pack, pack_ref, label, description, enabled, param_schema, out_schema, webhook_enabled, webhook_key, webhook_config, is_adhoc, created, updated";

        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new(format!("SELECT {select_cols} FROM trigger"));
        let mut count_qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM trigger");

        let mut has_where = false;

        macro_rules! push_condition {
            ($cond_prefix:expr, $value:expr) => {{
                if !has_where {
                    qb.push(" WHERE ");
                    count_qb.push(" WHERE ");
                    has_where = true;
                } else {
                    qb.push(" AND ");
                    count_qb.push(" AND ");
                }
                qb.push($cond_prefix);
                qb.push_bind($value.clone());
                count_qb.push($cond_prefix);
                count_qb.push_bind($value);
            }};
        }

        if let Some(pack_id) = filters.pack {
            push_condition!("pack = ", pack_id);
        }
        if let Some(enabled) = filters.enabled {
            push_condition!("enabled = ", enabled);
        }

        // Suppress unused-assignment warning from the macro's last expansion.
        let _ = has_where;

        // Count
        let total: i64 = count_qb.build_query_scalar().fetch_one(db).await?;
        let total = total.max(0) as u64;

        // Data query
        qb.push(" ORDER BY ref ASC");
        qb.push(" LIMIT ");
        qb.push_bind(filters.limit as i64);
        qb.push(" OFFSET ");
        qb.push_bind(filters.offset as i64);

        let rows: Vec<Trigger> = qb.build_query_as().fetch_all(db).await?;

        Ok(TriggerSearchResult { rows, total })
    }

    /// Find triggers by pack ID
    pub async fn find_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Trigger>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let triggers = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            WHERE pack = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(triggers)
    }

    /// Find enabled triggers
    pub async fn find_enabled<'e, E>(executor: E) -> Result<Vec<Trigger>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let triggers = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            WHERE enabled = true
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(triggers)
    }

    /// Find trigger by webhook key
    pub async fn find_by_webhook_key<'e, E>(
        executor: E,
        webhook_key: &str,
    ) -> Result<Option<Trigger>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let trigger = sqlx::query_as::<_, Trigger>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, enabled,
                   param_schema, out_schema, webhook_enabled, webhook_key, webhook_config,
                   is_adhoc, created, updated
            FROM trigger
            WHERE webhook_key = $1
            "#,
        )
        .bind(webhook_key)
        .fetch_optional(executor)
        .await?;

        Ok(trigger)
    }

    /// Enable webhooks for a trigger
    pub async fn enable_webhook<'e, E>(executor: E, trigger_id: Id) -> Result<WebhookInfo>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        #[derive(sqlx::FromRow)]
        struct WebhookResult {
            webhook_enabled: bool,
            webhook_key: String,
            webhook_url: String,
        }

        let result = sqlx::query_as::<_, WebhookResult>(
            r#"
            SELECT * FROM enable_trigger_webhook($1)
            "#,
        )
        .bind(trigger_id)
        .fetch_one(executor)
        .await?;

        Ok(WebhookInfo {
            enabled: result.webhook_enabled,
            webhook_key: result.webhook_key,
            webhook_url: result.webhook_url,
        })
    }

    /// Disable webhooks for a trigger
    pub async fn disable_webhook<'e, E>(executor: E, trigger_id: Id) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT disable_trigger_webhook($1)
            "#,
        )
        .bind(trigger_id)
        .fetch_one(executor)
        .await?;

        Ok(result)
    }

    /// Regenerate webhook key for a trigger
    pub async fn regenerate_webhook_key<'e, E>(
        executor: E,
        trigger_id: Id,
    ) -> Result<WebhookKeyRegenerate>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        #[derive(sqlx::FromRow)]
        struct RegenerateResult {
            webhook_key: String,
            previous_key_revoked: bool,
        }

        let result = sqlx::query_as::<_, RegenerateResult>(
            r#"
            SELECT * FROM regenerate_trigger_webhook_key($1)
            "#,
        )
        .bind(trigger_id)
        .fetch_one(executor)
        .await?;

        Ok(WebhookKeyRegenerate {
            webhook_key: result.webhook_key,
            previous_key_revoked: result.previous_key_revoked,
        })
    }

    // ========================================================================
    // Phase 3: Advanced Webhook Features
    // ========================================================================

    /// Update webhook configuration for a trigger
    pub async fn update_webhook_config<'e, E>(
        executor: E,
        trigger_id: Id,
        config: serde_json::Value,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query(
            r#"
            UPDATE trigger
            SET webhook_config = $2, updated = NOW()
            WHERE id = $1
            "#,
        )
        .bind(trigger_id)
        .bind(config)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// Log webhook event for auditing and analytics
    pub async fn log_webhook_event<'e, E>(executor: E, input: WebhookEventLogInput) -> Result<i64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO webhook_event_log (
                trigger_id, trigger_ref, webhook_key, event_id,
                source_ip, user_agent, payload_size_bytes, headers,
                status_code, error_message, processing_time_ms,
                hmac_verified, rate_limited, ip_allowed
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(input.trigger_id)
        .bind(input.trigger_ref)
        .bind(input.webhook_key)
        .bind(input.event_id)
        .bind(input.source_ip)
        .bind(input.user_agent)
        .bind(input.payload_size_bytes)
        .bind(input.headers)
        .bind(input.status_code)
        .bind(input.error_message)
        .bind(input.processing_time_ms)
        .bind(input.hmac_verified)
        .bind(input.rate_limited)
        .bind(input.ip_allowed)
        .fetch_one(executor)
        .await?;

        Ok(id)
    }
}

/// Webhook information returned when enabling webhooks
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookInfo {
    pub enabled: bool,
    pub webhook_key: String,
    pub webhook_url: String,
}

/// Webhook key regeneration result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookKeyRegenerate {
    pub webhook_key: String,
    pub previous_key_revoked: bool,
}

/// Input for logging webhook events
#[derive(Debug, Clone)]
pub struct WebhookEventLogInput {
    pub trigger_id: Id,
    pub trigger_ref: String,
    pub webhook_key: String,
    pub event_id: Option<Id>,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
    pub payload_size_bytes: Option<i32>,
    pub headers: Option<JsonValue>,
    pub status_code: i32,
    pub error_message: Option<String>,
    pub processing_time_ms: Option<i32>,
    pub hmac_verified: Option<bool>,
    pub rate_limited: bool,
    pub ip_allowed: Option<bool>,
}

// ============================================================================
// Sensor Repository
// ============================================================================

/// Repository for Sensor operations
pub struct SensorRepository;

impl Repository for SensorRepository {
    type Entity = Sensor;

    fn table_name() -> &'static str {
        "sensor"
    }
}

/// Input for creating a new sensor
#[derive(Debug, Clone)]
pub struct CreateSensorInput {
    pub r#ref: String,
    pub pack: Option<Id>,
    pub pack_ref: Option<String>,
    pub label: String,
    pub description: String,
    pub entrypoint: String,
    pub runtime: Id,
    pub runtime_ref: String,
    pub runtime_version_constraint: Option<String>,
    pub trigger: Id,
    pub trigger_ref: String,
    pub enabled: bool,
    pub param_schema: Option<JsonSchema>,
    pub config: Option<JsonValue>,
}

/// Input for updating a sensor
#[derive(Debug, Clone, Default)]
pub struct UpdateSensorInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub entrypoint: Option<String>,
    pub runtime: Option<Id>,
    pub runtime_ref: Option<String>,
    pub runtime_version_constraint: Option<Option<String>>,
    pub trigger: Option<Id>,
    pub trigger_ref: Option<String>,
    pub enabled: Option<bool>,
    pub param_schema: Option<JsonSchema>,
    pub config: Option<JsonValue>,
}

#[async_trait::async_trait]
impl FindById for SensorRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensor = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(sensor)
    }
}

#[async_trait::async_trait]
impl FindByRef for SensorRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensor = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(sensor)
    }
}

#[async_trait::async_trait]
impl List for SensorRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(sensors)
    }
}

#[async_trait::async_trait]
impl Create for SensorRepository {
    type CreateInput = CreateSensorInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensor = sqlx::query_as::<_, Sensor>(
            r#"
            INSERT INTO sensor (ref, pack, pack_ref, label, description, entrypoint,
                                runtime, runtime_ref, runtime_version_constraint,
                                trigger, trigger_ref, enabled,
                                param_schema, config)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id, ref, pack, pack_ref, label, description, entrypoint,
                      runtime, runtime_ref, runtime_version_constraint,
                      trigger, trigger_ref, enabled,
                      param_schema, config, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(&input.label)
        .bind(&input.description)
        .bind(&input.entrypoint)
        .bind(input.runtime)
        .bind(&input.runtime_ref)
        .bind(&input.runtime_version_constraint)
        .bind(input.trigger)
        .bind(&input.trigger_ref)
        .bind(input.enabled)
        .bind(&input.param_schema)
        .bind(&input.config)
        .fetch_one(executor)
        .await?;

        Ok(sensor)
    }
}

#[async_trait::async_trait]
impl Update for SensorRepository {
    type UpdateInput = UpdateSensorInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE sensor SET ");
        let mut has_updates = false;

        if let Some(label) = &input.label {
            query.push("label = ");
            query.push_bind(label);
            has_updates = true;
        }

        if let Some(description) = &input.description {
            if has_updates {
                query.push(", ");
            }
            query.push("description = ");
            query.push_bind(description);
            has_updates = true;
        }

        if let Some(entrypoint) = &input.entrypoint {
            if has_updates {
                query.push(", ");
            }
            query.push("entrypoint = ");
            query.push_bind(entrypoint);
            has_updates = true;
        }

        if let Some(enabled) = input.enabled {
            if has_updates {
                query.push(", ");
            }
            query.push("enabled = ");
            query.push_bind(enabled);
            has_updates = true;
        }

        if let Some(runtime) = input.runtime {
            if has_updates {
                query.push(", ");
            }
            query.push("runtime = ");
            query.push_bind(runtime);
            has_updates = true;
        }

        if let Some(runtime_ref) = &input.runtime_ref {
            if has_updates {
                query.push(", ");
            }
            query.push("runtime_ref = ");
            query.push_bind(runtime_ref);
            has_updates = true;
        }

        if let Some(runtime_version_constraint) = &input.runtime_version_constraint {
            if has_updates {
                query.push(", ");
            }
            query.push("runtime_version_constraint = ");
            query.push_bind(runtime_version_constraint);
            has_updates = true;
        }

        if let Some(trigger) = input.trigger {
            if has_updates {
                query.push(", ");
            }
            query.push("trigger = ");
            query.push_bind(trigger);
            has_updates = true;
        }

        if let Some(trigger_ref) = &input.trigger_ref {
            if has_updates {
                query.push(", ");
            }
            query.push("trigger_ref = ");
            query.push_bind(trigger_ref);
            has_updates = true;
        }

        if let Some(param_schema) = &input.param_schema {
            if has_updates {
                query.push(", ");
            }
            query.push("param_schema = ");
            query.push_bind(param_schema);
            has_updates = true;
        }

        if let Some(config) = &input.config {
            if has_updates {
                query.push(", ");
            }
            query.push("config = ");
            query.push_bind(config);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, label, description, entrypoint, runtime, runtime_ref, runtime_version_constraint, trigger, trigger_ref, enabled, param_schema, config, created, updated");

        let sensor = query.build_query_as::<Sensor>().fetch_one(executor).await?;

        Ok(sensor)
    }
}

#[async_trait::async_trait]
impl Delete for SensorRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM sensor WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl SensorRepository {
    /// Search sensors with all filters pushed into SQL.
    ///
    /// All filter fields are combinable (AND). Pagination is server-side.
    pub async fn list_search<'e, E>(
        db: E,
        filters: &SensorSearchFilters,
    ) -> Result<SensorSearchResult>
    where
        E: Executor<'e, Database = Postgres> + Copy + 'e,
    {
        let select_cols = "id, ref, pack, pack_ref, label, description, entrypoint, runtime, runtime_ref, runtime_version_constraint, trigger, trigger_ref, enabled, param_schema, config, created, updated";

        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new(format!("SELECT {select_cols} FROM sensor"));
        let mut count_qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM sensor");

        let mut has_where = false;

        macro_rules! push_condition {
            ($cond_prefix:expr, $value:expr) => {{
                if !has_where {
                    qb.push(" WHERE ");
                    count_qb.push(" WHERE ");
                    has_where = true;
                } else {
                    qb.push(" AND ");
                    count_qb.push(" AND ");
                }
                qb.push($cond_prefix);
                qb.push_bind($value.clone());
                count_qb.push($cond_prefix);
                count_qb.push_bind($value);
            }};
        }

        if let Some(pack_id) = filters.pack {
            push_condition!("pack = ", pack_id);
        }
        if let Some(trigger_id) = filters.trigger {
            push_condition!("trigger = ", trigger_id);
        }
        if let Some(enabled) = filters.enabled {
            push_condition!("enabled = ", enabled);
        }

        // Suppress unused-assignment warning from the macro's last expansion.
        let _ = has_where;

        // Count
        let total: i64 = count_qb.build_query_scalar().fetch_one(db).await?;
        let total = total.max(0) as u64;

        // Data query
        qb.push(" ORDER BY ref ASC");
        qb.push(" LIMIT ");
        qb.push_bind(filters.limit as i64);
        qb.push(" OFFSET ");
        qb.push_bind(filters.offset as i64);

        let rows: Vec<Sensor> = qb.build_query_as().fetch_all(db).await?;

        Ok(SensorSearchResult { rows, total })
    }

    /// Find sensors by trigger ID
    pub async fn find_by_trigger<'e, E>(executor: E, trigger_id: Id) -> Result<Vec<Sensor>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            WHERE trigger = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(trigger_id)
        .fetch_all(executor)
        .await?;

        Ok(sensors)
    }

    /// Find enabled sensors
    pub async fn find_enabled<'e, E>(executor: E) -> Result<Vec<Sensor>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            WHERE enabled = true
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(sensors)
    }

    /// Find sensors by pack ID
    pub async fn find_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Sensor>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, runtime_version_constraint,
                   trigger, trigger_ref, enabled,
                   param_schema, config, created, updated
            FROM sensor
            WHERE pack = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(sensors)
    }
}
