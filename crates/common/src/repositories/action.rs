//! Action and Policy repository for database operations
//!
//! This module provides CRUD operations and queries for Action and Policy entities.

use crate::models::{action::*, enums::PolicyMethod, Id, JsonSchema};
use crate::{Error, Result};
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

/// Repository for Action operations
pub struct ActionRepository;

impl Repository for ActionRepository {
    type Entity = Action;

    fn table_name() -> &'static str {
        "action"
    }
}

/// Input for creating a new action
#[derive(Debug, Clone)]
pub struct CreateActionInput {
    pub r#ref: String,
    pub pack: Id,
    pub pack_ref: String,
    pub label: String,
    pub description: String,
    pub entrypoint: String,
    pub runtime: Option<Id>,
    pub param_schema: Option<JsonSchema>,
    pub out_schema: Option<JsonSchema>,
    pub is_adhoc: bool,
}

/// Input for updating an action
#[derive(Debug, Clone, Default)]
pub struct UpdateActionInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub entrypoint: Option<String>,
    pub runtime: Option<Id>,
    pub param_schema: Option<JsonSchema>,
    pub out_schema: Option<JsonSchema>,
}

#[async_trait::async_trait]
impl FindById for ActionRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(action)
    }
}

#[async_trait::async_trait]
impl FindByRef for ActionRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(action)
    }
}

#[async_trait::async_trait]
impl List for ActionRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }
}

#[async_trait::async_trait]
impl Create for ActionRepository {
    type CreateInput = CreateActionInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Validate ref format
        if !input
            .r#ref
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err(Error::validation(
                "Action ref must contain only alphanumeric characters, dots, underscores, and hyphens",
            ));
        }

        // Try to insert - database will enforce uniqueness constraint
        let action = sqlx::query_as::<_, Action>(
            r#"
            INSERT INTO action (ref, pack, pack_ref, label, description, entrypoint,
                                runtime, param_schema, out_schema, is_adhoc)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, ref, pack, pack_ref, label, description, entrypoint,
                      runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(&input.label)
        .bind(&input.description)
        .bind(&input.entrypoint)
        .bind(input.runtime)
        .bind(&input.param_schema)
        .bind(&input.out_schema)
        .bind(input.is_adhoc)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            // Convert unique constraint violation to AlreadyExists error
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return Error::already_exists("Action", "ref", &input.r#ref);
                }
            }
            e.into()
        })?;

        Ok(action)
    }
}

#[async_trait::async_trait]
impl Update for ActionRepository {
    type UpdateInput = UpdateActionInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build dynamic UPDATE query
        let mut query = QueryBuilder::new("UPDATE action SET ");
        let mut has_updates = false;

        if let Some(label) = &input.label {
            if has_updates {
                query.push(", ");
            }
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

        if let Some(runtime) = input.runtime {
            if has_updates {
                query.push(", ");
            }
            query.push("runtime = ");
            query.push_bind(runtime);
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
            // No updates requested, fetch and return existing action
            return Self::find_by_id(executor, id)
                .await?
                .ok_or_else(|| Error::not_found("action", "id", id.to_string()));
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, label, description, entrypoint, runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated");

        let action = query
            .build_query_as::<Action>()
            .fetch_one(executor)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => Error::not_found("action", "id", id.to_string()),
                _ => e.into(),
            })?;

        Ok(action)
    }
}

#[async_trait::async_trait]
impl Delete for ActionRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM action WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl ActionRepository {
    /// Find actions by pack ID
    pub async fn find_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Action>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE pack = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }

    /// Find actions by runtime ID
    pub async fn find_by_runtime<'e, E>(executor: E, runtime_id: Id) -> Result<Vec<Action>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE runtime = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(runtime_id)
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }

    /// Search actions by name/label
    pub async fn search<'e, E>(executor: E, query: &str) -> Result<Vec<Action>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let search_pattern = format!("%{}%", query.to_lowercase());
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE LOWER(ref) LIKE $1 OR LOWER(label) LIKE $1 OR LOWER(description) LIKE $1
            ORDER BY ref ASC
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }

    /// Find all workflow actions (actions where is_workflow = true)
    pub async fn find_workflows<'e, E>(executor: E) -> Result<Vec<Action>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE is_workflow = true
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }

    /// Find action by workflow definition ID
    pub async fn find_by_workflow_def<'e, E>(
        executor: E,
        workflow_def_id: Id,
    ) -> Result<Option<Action>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT id, ref, pack, pack_ref, label, description, entrypoint,
                   runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            FROM action
            WHERE workflow_def = $1
            "#,
        )
        .bind(workflow_def_id)
        .fetch_optional(executor)
        .await?;

        Ok(action)
    }

    /// Link an action to a workflow definition
    pub async fn link_workflow_def<'e, E>(
        executor: E,
        action_id: Id,
        workflow_def_id: Id,
    ) -> Result<Action>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let action = sqlx::query_as::<_, Action>(
            r#"
            UPDATE action
            SET is_workflow = true, workflow_def = $2, updated = NOW()
            WHERE id = $1
            RETURNING id, ref, pack, pack_ref, label, description, entrypoint,
                      runtime, param_schema, out_schema, is_workflow, workflow_def, is_adhoc, created, updated
            "#,
        )
        .bind(action_id)
        .bind(workflow_def_id)
        .fetch_one(executor)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => Error::not_found("action", "id", action_id.to_string()),
            _ => e.into(),
        })?;

        Ok(action)
    }
}

/// Repository for Policy operations
// ============================================================================
// Policy Repository
// ============================================================================

/// Repository for Policy operations
pub struct PolicyRepository;

impl Repository for PolicyRepository {
    type Entity = Policy;

    fn table_name() -> &'static str {
        "policies"
    }
}

/// Input for creating a new policy
#[derive(Debug, Clone)]
pub struct CreatePolicyInput {
    pub r#ref: String,
    pub pack: Option<Id>,
    pub pack_ref: Option<String>,
    pub action: Option<Id>,
    pub action_ref: Option<String>,
    pub parameters: Vec<String>,
    pub method: PolicyMethod,
    pub threshold: i32,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

/// Input for updating a policy
#[derive(Debug, Clone, Default)]
pub struct UpdatePolicyInput {
    pub parameters: Option<Vec<String>>,
    pub method: Option<PolicyMethod>,
    pub threshold: Option<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[async_trait::async_trait]
impl FindById for PolicyRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let policy = sqlx::query_as::<_, Policy>(
            r#"
            SELECT id, ref, pack, pack_ref, action, action_ref, parameters, method,
                   threshold, name, description, tags, created, updated
            FROM policies
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(policy)
    }
}

#[async_trait::async_trait]
impl FindByRef for PolicyRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let policy = sqlx::query_as::<_, Policy>(
            r#"
            SELECT id, ref, pack, pack_ref, action, action_ref, parameters, method,
                   threshold, name, description, tags, created, updated
            FROM policies
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(policy)
    }
}

#[async_trait::async_trait]
impl List for PolicyRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let policies = sqlx::query_as::<_, Policy>(
            r#"
            SELECT id, ref, pack, pack_ref, action, action_ref, parameters, method,
                   threshold, name, description, tags, created, updated
            FROM policies
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(policies)
    }
}

#[async_trait::async_trait]
impl Create for PolicyRepository {
    type CreateInput = CreatePolicyInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Try to insert - database will enforce uniqueness constraint
        let policy = sqlx::query_as::<_, Policy>(
            r#"
            INSERT INTO policies (ref, pack, pack_ref, action, action_ref, parameters,
                                 method, threshold, name, description, tags)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, ref, pack, pack_ref, action, action_ref, parameters, method,
                      threshold, name, description, tags, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(input.action)
        .bind(&input.action_ref)
        .bind(&input.parameters)
        .bind(input.method)
        .bind(input.threshold)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.tags)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            // Convert unique constraint violation to AlreadyExists error
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return Error::already_exists("Policy", "ref", &input.r#ref);
                }
            }
            e.into()
        })?;

        Ok(policy)
    }
}

#[async_trait::async_trait]
impl Update for PolicyRepository {
    type UpdateInput = UpdatePolicyInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let mut query = QueryBuilder::new("UPDATE policies SET ");
        let mut has_updates = false;

        if let Some(parameters) = &input.parameters {
            if has_updates {
                query.push(", ");
            }
            query.push("parameters = ");
            query.push_bind(parameters);
            has_updates = true;
        }

        if let Some(method) = input.method {
            if has_updates {
                query.push(", ");
            }
            query.push("method = ");
            query.push_bind(method);
            has_updates = true;
        }

        if let Some(threshold) = input.threshold {
            if has_updates {
                query.push(", ");
            }
            query.push("threshold = ");
            query.push_bind(threshold);
            has_updates = true;
        }

        if let Some(name) = &input.name {
            if has_updates {
                query.push(", ");
            }
            query.push("name = ");
            query.push_bind(name);
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

        if let Some(tags) = &input.tags {
            if has_updates {
                query.push(", ");
            }
            query.push("tags = ");
            query.push_bind(tags);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing policy
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, action, action_ref, parameters, method, threshold, name, description, tags, created, updated");

        let policy = query.build_query_as::<Policy>().fetch_one(executor).await?;

        Ok(policy)
    }
}

#[async_trait::async_trait]
impl Delete for PolicyRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM policies WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl PolicyRepository {
    /// Find policies by action ID
    pub async fn find_by_action<'e, E>(executor: E, action_id: Id) -> Result<Vec<Policy>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let policies = sqlx::query_as::<_, Policy>(
            r#"
            SELECT id, ref, pack, pack_ref, action, action_ref, parameters, method,
                   threshold, name, description, tags, created, updated
            FROM policies
            WHERE action = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(action_id)
        .fetch_all(executor)
        .await?;

        Ok(policies)
    }

    /// Find policies by tag
    pub async fn find_by_tag<'e, E>(executor: E, tag: &str) -> Result<Vec<Policy>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let policies = sqlx::query_as::<_, Policy>(
            r#"
            SELECT id, ref, pack, pack_ref, action, action_ref, parameters, method,
                   threshold, name, description, tags, created, updated
            FROM policies
            WHERE $1 = ANY(tags)
            ORDER BY ref ASC
            "#,
        )
        .bind(tag)
        .fetch_all(executor)
        .await?;

        Ok(policies)
    }
}
