//! Rule repository for database operations
//!
//! This module provides CRUD operations and queries for Rule entities.

use crate::models::{rule::*, Id};
use crate::{Error, Result};
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

/// Repository for Rule operations
pub struct RuleRepository;

impl Repository for RuleRepository {
    type Entity = Rule;

    fn table_name() -> &'static str {
        "rules"
    }
}

/// Input for creating a new rule
#[derive(Debug, Clone)]
pub struct CreateRuleInput {
    pub r#ref: String,
    pub pack: Id,
    pub pack_ref: String,
    pub label: String,
    pub description: String,
    pub action: Id,
    pub action_ref: String,
    pub trigger: Id,
    pub trigger_ref: String,
    pub conditions: serde_json::Value,
    pub action_params: serde_json::Value,
    pub trigger_params: serde_json::Value,
    pub enabled: bool,
    pub is_adhoc: bool,
}

/// Input for updating a rule
#[derive(Debug, Clone, Default)]
pub struct UpdateRuleInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub conditions: Option<serde_json::Value>,
    pub action_params: Option<serde_json::Value>,
    pub trigger_params: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[async_trait::async_trait]
impl FindById for RuleRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rule = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(rule)
    }
}

#[async_trait::async_trait]
impl FindByRef for RuleRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rule = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(rule)
    }
}

#[async_trait::async_trait]
impl List for RuleRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }
}

#[async_trait::async_trait]
impl Create for RuleRepository {
    type CreateInput = CreateRuleInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rule = sqlx::query_as::<_, Rule>(
            r#"
            INSERT INTO rule (ref, pack, pack_ref, label, description, action, action_ref,
                              trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id, ref, pack, pack_ref, label, description, action, action_ref,
                      trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(&input.label)
        .bind(&input.description)
        .bind(input.action)
        .bind(&input.action_ref)
        .bind(input.trigger)
        .bind(&input.trigger_ref)
        .bind(&input.conditions)
        .bind(&input.action_params)
        .bind(&input.trigger_params)
        .bind(input.enabled)
        .bind(input.is_adhoc)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.is_unique_violation() {
                    return Error::already_exists("Rule", "ref", &input.r#ref);
                }
            }
            e.into()
        })?;

        Ok(rule)
    }
}

#[async_trait::async_trait]
impl Update for RuleRepository {
    type UpdateInput = UpdateRuleInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE rule SET ");
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

        if let Some(conditions) = &input.conditions {
            if has_updates {
                query.push(", ");
            }
            query.push("conditions = ");
            query.push_bind(conditions);
            has_updates = true;
        }

        if let Some(action_params) = &input.action_params {
            if has_updates {
                query.push(", ");
            }
            query.push("action_params = ");
            query.push_bind(action_params);
            has_updates = true;
        }

        if let Some(trigger_params) = &input.trigger_params {
            if has_updates {
                query.push(", ");
            }
            query.push("trigger_params = ");
            query.push_bind(trigger_params);
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

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, label, description, action, action_ref, trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated");

        let rule = query.build_query_as::<Rule>().fetch_one(executor).await?;

        Ok(rule)
    }
}

#[async_trait::async_trait]
impl Delete for RuleRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM rule WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl RuleRepository {
    /// Find rules by pack ID
    pub async fn find_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Rule>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE pack = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }

    /// Find rules by action ID
    pub async fn find_by_action<'e, E>(executor: E, action_id: Id) -> Result<Vec<Rule>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE action = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(action_id)
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }

    /// Find rules by trigger ID
    pub async fn find_by_trigger<'e, E>(executor: E, trigger_id: Id) -> Result<Vec<Rule>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE trigger = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(trigger_id)
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }

    /// Find enabled rules
    pub async fn find_enabled<'e, E>(executor: E) -> Result<Vec<Rule>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE enabled = true
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }
}
