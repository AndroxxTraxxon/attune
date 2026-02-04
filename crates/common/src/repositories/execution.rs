//! Execution repository for database operations

use crate::models::{enums::ExecutionStatus, execution::*, Id, JsonDict};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

pub struct ExecutionRepository;

impl Repository for ExecutionRepository {
    type Entity = Execution;
    fn table_name() -> &'static str {
        "executions"
    }
}

#[derive(Debug, Clone)]
pub struct CreateExecutionInput {
    pub action: Option<Id>,
    pub action_ref: String,
    pub config: Option<JsonDict>,
    pub parent: Option<Id>,
    pub enforcement: Option<Id>,
    pub executor: Option<Id>,
    pub status: ExecutionStatus,
    pub result: Option<JsonDict>,
    pub workflow_task: Option<WorkflowTaskMetadata>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateExecutionInput {
    pub status: Option<ExecutionStatus>,
    pub result: Option<JsonDict>,
    pub executor: Option<Id>,
    pub workflow_task: Option<WorkflowTaskMetadata>,
}

impl From<Execution> for UpdateExecutionInput {
    fn from(execution: Execution) -> Self {
        Self {
            status: Some(execution.status),
            result: execution.result,
            executor: execution.executor,
            workflow_task: execution.workflow_task,
        }
    }
}

#[async_trait::async_trait]
impl FindById for ExecutionRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Execution>(
            "SELECT id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated FROM execution WHERE id = $1"
        ).bind(id).fetch_optional(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for ExecutionRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Execution>(
            "SELECT id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated FROM execution ORDER BY created DESC LIMIT 1000"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for ExecutionRepository {
    type CreateInput = CreateExecutionInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Execution>(
            "INSERT INTO execution (action, action_ref, config, parent, enforcement, executor, status, result, workflow_task) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated"
        ).bind(input.action).bind(&input.action_ref).bind(&input.config).bind(input.parent).bind(input.enforcement).bind(input.executor).bind(input.status).bind(&input.result).bind(sqlx::types::Json(&input.workflow_task)).fetch_one(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Update for ExecutionRepository {
    type UpdateInput = UpdateExecutionInput;
    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query
        let mut query = QueryBuilder::new("UPDATE execution SET ");
        let mut has_updates = false;

        if let Some(status) = input.status {
            query.push("status = ").push_bind(status);
            has_updates = true;
        }
        if let Some(result) = &input.result {
            if has_updates {
                query.push(", ");
            }
            query.push("result = ").push_bind(result);
            has_updates = true;
        }
        if let Some(executor_id) = input.executor {
            if has_updates {
                query.push(", ");
            }
            query.push("executor = ").push_bind(executor_id);
            has_updates = true;
        }
        if let Some(workflow_task) = &input.workflow_task {
            if has_updates {
                query.push(", ");
            }
            query
                .push("workflow_task = ")
                .push_bind(sqlx::types::Json(workflow_task));
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(" RETURNING id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated");

        query
            .build_query_as::<Execution>()
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for ExecutionRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM execution WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl ExecutionRepository {
    pub async fn find_by_status<'e, E>(
        executor: E,
        status: ExecutionStatus,
    ) -> Result<Vec<Execution>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Execution>(
            "SELECT id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated FROM execution WHERE status = $1 ORDER BY created DESC"
        ).bind(status).fetch_all(executor).await.map_err(Into::into)
    }

    pub async fn find_by_enforcement<'e, E>(
        executor: E,
        enforcement_id: Id,
    ) -> Result<Vec<Execution>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Execution>(
            "SELECT id, action, action_ref, config, parent, enforcement, executor, status, result, workflow_task, created, updated FROM execution WHERE enforcement = $1 ORDER BY created DESC"
        ).bind(enforcement_id).fetch_all(executor).await.map_err(Into::into)
    }
}
