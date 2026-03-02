//! Rule repository for database operations
//!
//! This module provides CRUD operations and queries for Rule entities.

use crate::models::{rule::*, Id};
use crate::{Error, Result};
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

/// Filters for [`RuleRepository::list_search`].
///
/// All fields are optional and combinable (AND). Pagination is always applied.
#[derive(Debug, Clone, Default)]
pub struct RuleSearchFilters {
    /// Filter by pack ID
    pub pack: Option<Id>,
    /// Filter by action ID
    pub action: Option<Id>,
    /// Filter by trigger ID
    pub trigger: Option<Id>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
    pub limit: u32,
    pub offset: u32,
}

/// Result of [`RuleRepository::list_search`].
#[derive(Debug)]
pub struct RuleSearchResult {
    pub rows: Vec<Rule>,
    pub total: u64,
}

/// Input for restoring an ad-hoc rule during pack reinstallation.
/// Unlike `CreateRuleInput`, action and trigger IDs are optional because
/// the referenced entities may not exist yet or may have been removed.
#[derive(Debug, Clone)]
pub struct RestoreRuleInput {
    pub r#ref: String,
    pub pack: Id,
    pub pack_ref: String,
    pub label: String,
    pub description: String,
    pub action: Option<Id>,
    pub action_ref: String,
    pub trigger: Option<Id>,
    pub trigger_ref: String,
    pub conditions: serde_json::Value,
    pub action_params: serde_json::Value,
    pub trigger_params: serde_json::Value,
    pub enabled: bool,
}

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
    /// Search rules with all filters pushed into SQL.
    ///
    /// All filter fields are combinable (AND). Pagination is server-side.
    pub async fn list_search<'e, E>(db: E, filters: &RuleSearchFilters) -> Result<RuleSearchResult>
    where
        E: Executor<'e, Database = Postgres> + Copy + 'e,
    {
        let select_cols = "id, ref, pack, pack_ref, label, description, action, action_ref, trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated";

        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new(format!("SELECT {select_cols} FROM rule"));
        let mut count_qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM rule");

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
        if let Some(action_id) = filters.action {
            push_condition!("action = ", action_id);
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

        let rows: Vec<Rule> = qb.build_query_as().fetch_all(db).await?;

        Ok(RuleSearchResult { rows, total })
    }

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

    /// Find ad-hoc (user-created) rules belonging to a specific pack.
    /// Used to preserve custom rules during pack reinstallation.
    pub async fn find_adhoc_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Rule>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rules = sqlx::query_as::<_, Rule>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, action, action_ref,
                   trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc, created, updated
            FROM rule
            WHERE pack = $1 AND is_adhoc = true
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(rules)
    }

    /// Restore an ad-hoc rule after pack reinstallation.
    /// Accepts `Option<Id>` for action and trigger so the rule is preserved
    /// even if its referenced entities no longer exist.
    pub async fn restore_rule<'e, E>(executor: E, input: RestoreRuleInput) -> Result<Rule>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rule = sqlx::query_as::<_, Rule>(
            r#"
            INSERT INTO rule (ref, pack, pack_ref, label, description, action, action_ref,
                              trigger, trigger_ref, conditions, action_params, trigger_params, enabled, is_adhoc)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, true)
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

    /// Re-link rules whose action FK is NULL back to a newly recreated action,
    /// matched by `action_ref`. Used after pack reinstallation to fix rules
    /// from other packs that referenced actions in the reinstalled pack.
    pub async fn relink_action_by_ref<'e, E>(
        executor: E,
        action_ref: &str,
        action_id: Id,
    ) -> Result<u64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query(
            r#"
            UPDATE rule
            SET action = $1, updated = NOW()
            WHERE action IS NULL AND action_ref = $2
            "#,
        )
        .bind(action_id)
        .bind(action_ref)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// Re-link rules whose trigger FK is NULL back to a newly recreated trigger,
    /// matched by `trigger_ref`. Used after pack reinstallation to fix rules
    /// from other packs that referenced triggers in the reinstalled pack.
    pub async fn relink_trigger_by_ref<'e, E>(
        executor: E,
        trigger_ref: &str,
        trigger_id: Id,
    ) -> Result<u64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query(
            r#"
            UPDATE rule
            SET trigger = $1, updated = NOW()
            WHERE trigger IS NULL AND trigger_ref = $2
            "#,
        )
        .bind(trigger_id)
        .bind(trigger_ref)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}
