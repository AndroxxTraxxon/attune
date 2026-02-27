//! Event and Enforcement repository for database operations
//!
//! This module provides CRUD operations and queries for Event and Enforcement entities.
//! Note: Events are immutable time-series data — there is no Update impl for EventRepository.

use chrono::{DateTime, Utc};

use crate::models::{
    enums::{EnforcementCondition, EnforcementStatus},
    event::*,
    Id, JsonDict,
};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

/// Repository for Event operations
pub struct EventRepository;

impl Repository for EventRepository {
    type Entity = Event;

    fn table_name() -> &'static str {
        "event"
    }
}

/// Input for creating a new event
#[derive(Debug, Clone)]
pub struct CreateEventInput {
    pub trigger: Option<Id>,
    pub trigger_ref: String,
    pub config: Option<JsonDict>,
    pub payload: Option<JsonDict>,
    pub source: Option<Id>,
    pub source_ref: Option<String>,
    pub rule: Option<Id>,
    pub rule_ref: Option<String>,
}

#[async_trait::async_trait]
impl FindById for EventRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let event = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, trigger, trigger_ref, config, payload, source, source_ref,
                   rule, rule_ref, created
            FROM event
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(event)
    }
}

#[async_trait::async_trait]
impl List for EventRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, trigger, trigger_ref, config, payload, source, source_ref,
                   rule, rule_ref, created
            FROM event
            ORDER BY created DESC
            LIMIT 1000
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(events)
    }
}

#[async_trait::async_trait]
impl Create for EventRepository {
    type CreateInput = CreateEventInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let event = sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO event (trigger, trigger_ref, config, payload, source, source_ref, rule, rule_ref)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, trigger, trigger_ref, config, payload, source, source_ref,
                      rule, rule_ref, created
            "#,
        )
        .bind(input.trigger)
        .bind(&input.trigger_ref)
        .bind(&input.config)
        .bind(&input.payload)
        .bind(input.source)
        .bind(&input.source_ref)
        .bind(input.rule)
        .bind(&input.rule_ref)
        .fetch_one(executor)
        .await?;

        Ok(event)
    }
}

#[async_trait::async_trait]
impl Delete for EventRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM event WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl EventRepository {
    /// Find events by trigger ID
    pub async fn find_by_trigger<'e, E>(executor: E, trigger_id: Id) -> Result<Vec<Event>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, trigger, trigger_ref, config, payload, source, source_ref,
                   rule, rule_ref, created
            FROM event
            WHERE trigger = $1
            ORDER BY created DESC
            LIMIT 1000
            "#,
        )
        .bind(trigger_id)
        .fetch_all(executor)
        .await?;

        Ok(events)
    }

    /// Find events by trigger ref
    pub async fn find_by_trigger_ref<'e, E>(executor: E, trigger_ref: &str) -> Result<Vec<Event>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, trigger, trigger_ref, config, payload, source, source_ref,
                   rule, rule_ref, created
            FROM event
            WHERE trigger_ref = $1
            ORDER BY created DESC
            LIMIT 1000
            "#,
        )
        .bind(trigger_ref)
        .fetch_all(executor)
        .await?;

        Ok(events)
    }
}

// ============================================================================
// Enforcement Repository
// ============================================================================

/// Repository for Enforcement operations
pub struct EnforcementRepository;

impl Repository for EnforcementRepository {
    type Entity = Enforcement;

    fn table_name() -> &'static str {
        "enforcement"
    }
}

/// Input for creating a new enforcement
#[derive(Debug, Clone)]
pub struct CreateEnforcementInput {
    pub rule: Option<Id>,
    pub rule_ref: String,
    pub trigger_ref: String,
    pub config: Option<JsonDict>,
    pub event: Option<Id>,
    pub status: EnforcementStatus,
    pub payload: JsonDict,
    pub condition: EnforcementCondition,
    pub conditions: serde_json::Value,
}

/// Input for updating an enforcement
#[derive(Debug, Clone, Default)]
pub struct UpdateEnforcementInput {
    pub status: Option<EnforcementStatus>,
    pub payload: Option<JsonDict>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[async_trait::async_trait]
impl FindById for EnforcementRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcement = sqlx::query_as::<_, Enforcement>(
            r#"
            SELECT id, rule, rule_ref, trigger_ref, config, event, status, payload,
                   condition, conditions, created, resolved_at
            FROM enforcement
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(enforcement)
    }
}

#[async_trait::async_trait]
impl List for EnforcementRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcements = sqlx::query_as::<_, Enforcement>(
            r#"
            SELECT id, rule, rule_ref, trigger_ref, config, event, status, payload,
                   condition, conditions, created, resolved_at
            FROM enforcement
            ORDER BY created DESC
            LIMIT 1000
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(enforcements)
    }
}

#[async_trait::async_trait]
impl Create for EnforcementRepository {
    type CreateInput = CreateEnforcementInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcement = sqlx::query_as::<_, Enforcement>(
            r#"
            INSERT INTO enforcement (rule, rule_ref, trigger_ref, config, event, status,
                                     payload, condition, conditions)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, rule, rule_ref, trigger_ref, config, event, status, payload,
                      condition, conditions, created, resolved_at
            "#,
        )
        .bind(input.rule)
        .bind(&input.rule_ref)
        .bind(&input.trigger_ref)
        .bind(&input.config)
        .bind(input.event)
        .bind(input.status)
        .bind(&input.payload)
        .bind(input.condition)
        .bind(&input.conditions)
        .fetch_one(executor)
        .await?;

        Ok(enforcement)
    }
}

#[async_trait::async_trait]
impl Update for EnforcementRepository {
    type UpdateInput = UpdateEnforcementInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE enforcement SET ");
        let mut has_updates = false;

        if let Some(status) = input.status {
            query.push("status = ");
            query.push_bind(status);
            has_updates = true;
        }

        if let Some(payload) = &input.payload {
            if has_updates {
                query.push(", ");
            }
            query.push("payload = ");
            query.push_bind(payload);
            has_updates = true;
        }

        if let Some(resolved_at) = input.resolved_at {
            if has_updates {
                query.push(", ");
            }
            query.push("resolved_at = ");
            query.push_bind(resolved_at);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(" WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, rule, rule_ref, trigger_ref, config, event, status, payload, condition, conditions, created, resolved_at");

        let enforcement = query
            .build_query_as::<Enforcement>()
            .fetch_one(executor)
            .await?;

        Ok(enforcement)
    }
}

#[async_trait::async_trait]
impl Delete for EnforcementRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM enforcement WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl EnforcementRepository {
    /// Find enforcements by rule ID
    pub async fn find_by_rule<'e, E>(executor: E, rule_id: Id) -> Result<Vec<Enforcement>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcements = sqlx::query_as::<_, Enforcement>(
            r#"
            SELECT id, rule, rule_ref, trigger_ref, config, event, status, payload,
                   condition, conditions, created, resolved_at
            FROM enforcement
            WHERE rule = $1
            ORDER BY created DESC
            "#,
        )
        .bind(rule_id)
        .fetch_all(executor)
        .await?;

        Ok(enforcements)
    }

    /// Find enforcements by status
    pub async fn find_by_status<'e, E>(
        executor: E,
        status: EnforcementStatus,
    ) -> Result<Vec<Enforcement>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcements = sqlx::query_as::<_, Enforcement>(
            r#"
            SELECT id, rule, rule_ref, trigger_ref, config, event, status, payload,
                   condition, conditions, created, resolved_at
            FROM enforcement
            WHERE status = $1
            ORDER BY created DESC
            "#,
        )
        .bind(status)
        .fetch_all(executor)
        .await?;

        Ok(enforcements)
    }

    /// Find enforcements by event ID
    pub async fn find_by_event<'e, E>(executor: E, event_id: Id) -> Result<Vec<Enforcement>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let enforcements = sqlx::query_as::<_, Enforcement>(
            r#"
            SELECT id, rule, rule_ref, trigger_ref, config, event, status, payload,
                   condition, conditions, created, resolved_at
            FROM enforcement
            WHERE event = $1
            ORDER BY created DESC
            "#,
        )
        .bind(event_id)
        .fetch_all(executor)
        .await?;

        Ok(enforcements)
    }
}
