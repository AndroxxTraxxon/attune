//! Trigger and Sensor repository for database operations
//!
//! This module provides CRUD operations and queries for Trigger and Sensor entities.

use crate::models::{trigger::*, Id, JsonSchema};
use crate::Result;
use serde_json::Value as JsonValue;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

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
    pub enabled: Option<bool>,
    pub param_schema: Option<JsonSchema>,
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
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
                                runtime, runtime_ref, trigger, trigger_ref, enabled,
                                param_schema, config)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id, ref, pack, pack_ref, label, description, entrypoint,
                      runtime, runtime_ref, trigger, trigger_ref, enabled,
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

        if let Some(param_schema) = &input.param_schema {
            if has_updates {
                query.push(", ");
            }
            query.push("param_schema = ");
            query.push_bind(param_schema);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, label, description, entrypoint, runtime, runtime_ref, trigger, trigger_ref, enabled, param_schema, config, created, updated");

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
    /// Find sensors by trigger ID
    pub async fn find_by_trigger<'e, E>(executor: E, trigger_id: Id) -> Result<Vec<Sensor>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
                   runtime, runtime_ref, trigger, trigger_ref, enabled,
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
