//! Rule Lifecycle Listener
//!
//! This module listens for rule lifecycle events (created, enabled, disabled)
//! and notifies the sensor manager to update sensor process lifecycles accordingly.

use anyhow::Result;
use attune_common::mq::{
    Connection, Consumer, ConsumerConfig, MessageEnvelope, MessageType, RuleCreatedPayload,
    RuleDisabledPayload, RuleEnabledPayload,
};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::sensor_manager::SensorManager;

/// Rule lifecycle listener
pub struct RuleLifecycleListener {
    db: PgPool,
    connection: Connection,
    sensor_manager: Arc<SensorManager>,
    consumer: Arc<RwLock<Option<Consumer>>>,
    task_handle: RwLock<Option<JoinHandle<()>>>,
}

impl RuleLifecycleListener {
    /// Create a new rule lifecycle listener
    pub fn new(db: PgPool, connection: Connection, sensor_manager: Arc<SensorManager>) -> Self {
        Self {
            db,
            connection,
            sensor_manager,
            consumer: Arc::new(RwLock::new(None)),
            task_handle: RwLock::new(None),
        }
    }

    /// Start listening for rule lifecycle events
    pub async fn start(&self) -> Result<()> {
        info!("Starting rule lifecycle listener");

        // Create consumer configuration
        let consumer_config = ConsumerConfig {
            queue: "attune.rules.lifecycle.queue".to_string(),
            tag: "sensor-rule-lifecycle".to_string(),
            prefetch_count: 10,
            auto_ack: false,
            exclusive: false,
        };

        // Create consumer
        let consumer = Consumer::new(&self.connection, consumer_config).await?;

        // Bind queue to exchange with routing keys
        let exchange = "attune.events";
        let queue = "attune.rules.lifecycle.queue";

        // Declare queue
        consumer
            .channel()
            .queue_declare(
                queue.into(),
                lapin::options::QueueDeclareOptions {
                    durable: true,
                    exclusive: false,
                    auto_delete: false,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await?;

        // Bind to routing keys
        for routing_key in &["rule.created", "rule.enabled", "rule.disabled"] {
            consumer
                .channel()
                .queue_bind(
                    queue.into(),
                    exchange.into(),
                    (*routing_key).into(),
                    lapin::options::QueueBindOptions::default(),
                    lapin::types::FieldTable::default(),
                )
                .await?;
            info!(
                "Bound queue {} to exchange {} with routing key {}",
                queue, exchange, routing_key
            );
        }

        // Store consumer reference (for cleanup on drop)
        *self.consumer.write().await = Some(consumer);

        // Clone references for the spawned task
        let db = self.db.clone();
        let sensor_manager = self.sensor_manager.clone();
        let consumer_ref = self.consumer.clone();

        // Start consuming messages in a background task.
        // Take the consumer out of the Arc<RwLock> so we don't hold the read lock
        // for the entire duration of consume_with_handler (which would deadlock stop()).
        let handle = tokio::spawn(async move {
            let consumer = consumer_ref.write().await.take();
            if let Some(consumer) = consumer {
                let result = consumer
                    .consume_with_handler::<JsonValue, _, _>(move |envelope| {
                        let db = db.clone();
                        let sensor_manager = sensor_manager.clone();

                        async move {
                            if let Err(e) =
                                Self::handle_message(&db, &sensor_manager, envelope).await
                            {
                                error!("Failed to handle rule lifecycle message: {}", e);
                                return Err(attune_common::mq::MqError::Other(format!(
                                    "Handler error: {}",
                                    e
                                )));
                            }
                            Ok(())
                        }
                    })
                    .await;

                if let Err(e) = result {
                    error!("Rule lifecycle listener stopped with error: {}", e);
                } else {
                    info!("Rule lifecycle listener stopped");
                }
            }
        });

        *self.task_handle.write().await = Some(handle);

        info!("Rule lifecycle listener started");

        Ok(())
    }

    /// Stop the listener
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping rule lifecycle listener");

        // Abort the consumer task first — this ends the consume_with_handler loop
        // and drops the Consumer (and its channel) inside the task.
        if let Some(handle) = self.task_handle.write().await.take() {
            handle.abort();
            let _ = handle.await; // wait for abort to complete
        }

        // Clean up any consumer that wasn't taken by the task (e.g. if task never started)
        if let Some(consumer) = self.consumer.write().await.take() {
            drop(consumer);
        }

        info!("Rule lifecycle listener stopped");

        Ok(())
    }

    /// Handle a rule lifecycle message
    async fn handle_message(
        db: &PgPool,
        sensor_manager: &Arc<SensorManager>,
        envelope: MessageEnvelope<JsonValue>,
    ) -> Result<()> {
        match envelope.message_type {
            MessageType::RuleCreated => {
                let payload: RuleCreatedPayload = serde_json::from_value(envelope.payload)?;
                Self::handle_rule_created(db, sensor_manager, payload).await?;
            }
            MessageType::RuleEnabled => {
                let payload: RuleEnabledPayload = serde_json::from_value(envelope.payload)?;
                Self::handle_rule_enabled(db, sensor_manager, payload).await?;
            }
            MessageType::RuleDisabled => {
                let payload: RuleDisabledPayload = serde_json::from_value(envelope.payload)?;
                Self::handle_rule_disabled(sensor_manager, db, payload).await?;
            }
            _ => {
                warn!("Unexpected message type: {:?}", envelope.message_type);
            }
        }

        Ok(())
    }

    /// Handle rule created event
    async fn handle_rule_created(
        _db: &PgPool,
        sensor_manager: &Arc<SensorManager>,
        payload: RuleCreatedPayload,
    ) -> Result<()> {
        info!(
            "Handling RuleCreated: rule={}, trigger={}",
            payload.rule_ref, payload.trigger_ref
        );

        // Notify sensor manager about rule change (may need to start sensors)
        if let Some(trigger_id) = payload.trigger_id {
            if let Err(e) = sensor_manager.handle_rule_change(trigger_id).await {
                error!(
                    "Failed to handle sensor lifecycle for trigger {}: {}",
                    trigger_id, e
                );
            }
        }

        Ok(())
    }

    /// Handle rule enabled event
    async fn handle_rule_enabled(
        db: &PgPool,
        sensor_manager: &Arc<SensorManager>,
        payload: RuleEnabledPayload,
    ) -> Result<()> {
        info!(
            "Handling RuleEnabled: rule={}, trigger={}",
            payload.rule_ref, payload.trigger_ref
        );

        // Fetch trigger_id from database
        let trigger_id = match Self::get_trigger_id_for_rule(db, payload.rule_id).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                warn!("Trigger not found for rule {}", payload.rule_id);
                return Ok(());
            }
            Err(e) => {
                error!(
                    "Failed to fetch trigger for rule {}: {}",
                    payload.rule_id, e
                );
                return Err(e);
            }
        };

        // Notify sensor manager about rule change (may need to start sensors)
        if let Err(e) = sensor_manager.handle_rule_change(trigger_id).await {
            error!(
                "Failed to handle sensor lifecycle for trigger {}: {}",
                trigger_id, e
            );
        }

        Ok(())
    }

    /// Handle rule disabled event
    async fn handle_rule_disabled(
        sensor_manager: &Arc<SensorManager>,
        db: &PgPool,
        payload: RuleDisabledPayload,
    ) -> Result<()> {
        info!(
            "Handling RuleDisabled: rule={}, trigger={}",
            payload.rule_ref, payload.trigger_ref
        );

        // Fetch trigger_id from database
        let trigger_id = match Self::get_trigger_id_for_rule(db, payload.rule_id).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                warn!("Trigger not found for rule {}", payload.rule_id);
                return Ok(());
            }
            Err(e) => {
                error!(
                    "Failed to fetch trigger for rule {}: {}",
                    payload.rule_id, e
                );
                return Err(e);
            }
        };

        // Notify sensor manager about rule change (may need to stop sensors)
        if let Err(e) = sensor_manager.handle_rule_change(trigger_id).await {
            error!(
                "Failed to handle sensor lifecycle for trigger {}: {}",
                trigger_id, e
            );
        }

        Ok(())
    }

    /// Helper function to get trigger_id for a rule
    async fn get_trigger_id_for_rule(db: &PgPool, rule_id: i64) -> Result<Option<i64>> {
        let trigger_id = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT trigger
            FROM rule
            WHERE id = $1
            "#,
        )
        .bind(rule_id)
        .fetch_optional(db)
        .await?;

        Ok(trigger_id)
    }
}
