//! Execution repository for database operations

use chrono::{DateTime, Utc};

use crate::models::{enums::ExecutionStatus, execution::*, Id, JsonDict};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

/// Filters for the [`ExecutionRepository::search`] query-builder method.
///
/// Every field is optional. When set, the corresponding `WHERE` clause is
/// appended to the query. Pagination (`limit`/`offset`) is always applied.
///
/// Filters that involve the `enforcement` table (`rule_ref`, `trigger_ref`)
/// cause a `LEFT JOIN enforcement` to be added automatically.
#[derive(Debug, Clone, Default)]
pub struct ExecutionSearchFilters {
    pub status: Option<ExecutionStatus>,
    pub action_ref: Option<String>,
    pub pack_name: Option<String>,
    pub rule_ref: Option<String>,
    pub trigger_ref: Option<String>,
    pub executor: Option<Id>,
    pub result_contains: Option<String>,
    pub enforcement: Option<Id>,
    pub parent: Option<Id>,
    pub top_level_only: bool,
    pub limit: u32,
    pub offset: u32,
}

/// Result of [`ExecutionRepository::search`].
///
/// Includes the matching rows *and* the total count (before LIMIT/OFFSET)
/// so the caller can build pagination metadata without a second round-trip.
#[derive(Debug)]
pub struct ExecutionSearchResult {
    pub rows: Vec<ExecutionWithRefs>,
    pub total: u64,
}

/// An execution row with optional `rule_ref` / `trigger_ref` populated from
/// the joined `enforcement` table. This avoids a separate in-memory lookup.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExecutionWithRefs {
    // — execution columns (same order as SELECT_COLUMNS) —
    pub id: Id,
    pub action: Option<Id>,
    pub action_ref: String,
    pub config: Option<JsonDict>,
    pub env_vars: Option<JsonDict>,
    pub parent: Option<Id>,
    pub enforcement: Option<Id>,
    pub executor: Option<Id>,
    pub status: ExecutionStatus,
    pub result: Option<JsonDict>,
    pub started_at: Option<DateTime<Utc>>,
    #[sqlx(json, default)]
    pub workflow_task: Option<WorkflowTaskMetadata>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    // — joined from enforcement —
    pub rule_ref: Option<String>,
    pub trigger_ref: Option<String>,
}

/// Column list for SELECT queries on the execution table.
///
/// Defined once to avoid drift between queries and the `Execution` model.
/// The execution table has DB-only columns (`is_workflow`, `workflow_def`) that
/// are NOT in the Rust struct, so `SELECT *` must never be used.
pub const SELECT_COLUMNS: &str = "\
    id, action, action_ref, config, env_vars, parent, enforcement, \
    executor, status, result, started_at, workflow_task, created, updated";

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
    pub env_vars: Option<JsonDict>,
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
    pub started_at: Option<DateTime<Utc>>,
    pub workflow_task: Option<WorkflowTaskMetadata>,
}

impl From<Execution> for UpdateExecutionInput {
    fn from(execution: Execution) -> Self {
        Self {
            status: Some(execution.status),
            result: execution.result,
            executor: execution.executor,
            started_at: execution.started_at,
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
        let sql = format!("SELECT {SELECT_COLUMNS} FROM execution WHERE id = $1");
        sqlx::query_as::<_, Execution>(&sql)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for ExecutionRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql =
            format!("SELECT {SELECT_COLUMNS} FROM execution ORDER BY created DESC LIMIT 1000");
        sqlx::query_as::<_, Execution>(&sql)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for ExecutionRepository {
    type CreateInput = CreateExecutionInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql = format!(
            "INSERT INTO execution \
             (action, action_ref, config, env_vars, parent, enforcement, executor, status, result, workflow_task) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             RETURNING {SELECT_COLUMNS}"
        );
        sqlx::query_as::<_, Execution>(&sql)
            .bind(input.action)
            .bind(&input.action_ref)
            .bind(&input.config)
            .bind(&input.env_vars)
            .bind(input.parent)
            .bind(input.enforcement)
            .bind(input.executor)
            .bind(input.status)
            .bind(&input.result)
            .bind(sqlx::types::Json(&input.workflow_task))
            .fetch_one(executor)
            .await
            .map_err(Into::into)
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
        if let Some(started_at) = input.started_at {
            if has_updates {
                query.push(", ");
            }
            query.push("started_at = ").push_bind(started_at);
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
        query.push(" RETURNING ");
        query.push(SELECT_COLUMNS);

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
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM execution WHERE status = $1 ORDER BY created DESC"
        );
        sqlx::query_as::<_, Execution>(&sql)
            .bind(status)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_enforcement<'e, E>(
        executor: E,
        enforcement_id: Id,
    ) -> Result<Vec<Execution>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM execution WHERE enforcement = $1 ORDER BY created DESC"
        );
        sqlx::query_as::<_, Execution>(&sql)
            .bind(enforcement_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Find all child executions for a given parent execution ID.
    ///
    /// Returns child executions ordered by creation time (ascending),
    /// which is the natural task execution order for workflows.
    pub async fn find_by_parent<'e, E>(executor: E, parent_id: Id) -> Result<Vec<Execution>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM execution WHERE parent = $1 ORDER BY created ASC"
        );
        sqlx::query_as::<_, Execution>(&sql)
            .bind(parent_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Search executions with all filters pushed into SQL.
    ///
    /// Builds a dynamic query with only the WHERE clauses that apply,
    /// a LEFT JOIN on `enforcement` when `rule_ref` or `trigger_ref` filters
    /// are present (or always, to populate those columns on the result),
    /// and proper LIMIT/OFFSET so pagination is server-side.
    ///
    /// Returns both the matching page of rows and the total count.
    pub async fn search<'e, E>(
        db: E,
        filters: &ExecutionSearchFilters,
    ) -> Result<ExecutionSearchResult>
    where
        E: Executor<'e, Database = Postgres> + Copy + 'e,
    {
        // We always LEFT JOIN enforcement so we can return rule_ref/trigger_ref
        // on every row without a second round-trip.
        let prefixed_select = SELECT_COLUMNS
            .split(", ")
            .map(|col| format!("e.{col}"))
            .collect::<Vec<_>>()
            .join(", ");

        let select_clause = format!(
            "{prefixed_select}, enf.rule_ref AS rule_ref, enf.trigger_ref AS trigger_ref"
        );

        let from_clause = "FROM execution e LEFT JOIN enforcement enf ON e.enforcement = enf.id";

        // ── Build WHERE clauses ──────────────────────────────────────────
        let mut conditions: Vec<String> = Vec::new();

        // We'll collect bind values to push into the QueryBuilder afterwards.
        // Because QueryBuilder doesn't let us interleave raw SQL and binds in
        // arbitrary order easily, we build the SQL string with numbered $N
        // placeholders and then bind in order.

        // Track the next placeholder index ($1, $2, …).
        // We can't use QueryBuilder's push_bind because we need the COUNT(*)
        // query to share the same WHERE clause text. Instead we build the
        // clause once and execute both queries with manual binds.

        // ── Use QueryBuilder for the data query ──────────────────────────
        let mut qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new(format!("SELECT {select_clause} {from_clause}"));
        let mut count_qb: QueryBuilder<'_, Postgres> =
            QueryBuilder::new(format!("SELECT COUNT(*) AS total {from_clause}"));

        // Helper: append the same condition to both builders.
        // We need a tiny state machine since push_bind moves the value.
        macro_rules! push_condition {
            ($cond_prefix:expr, $value:expr) => {{
                let needs_where = conditions.is_empty();
                conditions.push(String::new()); // just to track count
                if needs_where {
                    qb.push(" WHERE ");
                    count_qb.push(" WHERE ");
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

        macro_rules! push_raw_condition {
            ($cond:expr) => {{
                let needs_where = conditions.is_empty();
                conditions.push(String::new());
                if needs_where {
                    qb.push(concat!(" WHERE ", $cond));
                    count_qb.push(concat!(" WHERE ", $cond));
                } else {
                    qb.push(concat!(" AND ", $cond));
                    count_qb.push(concat!(" AND ", $cond));
                }
            }};
        }

        if let Some(status) = &filters.status {
            push_condition!("e.status = ", status.clone());
        }
        if let Some(action_ref) = &filters.action_ref {
            push_condition!("e.action_ref = ", action_ref.clone());
        }
        if let Some(pack_name) = &filters.pack_name {
            let pattern = format!("{pack_name}.%");
            push_condition!("e.action_ref LIKE ", pattern);
        }
        if let Some(enforcement_id) = filters.enforcement {
            push_condition!("e.enforcement = ", enforcement_id);
        }
        if let Some(parent_id) = filters.parent {
            push_condition!("e.parent = ", parent_id);
        }
        if filters.top_level_only {
            push_raw_condition!("e.parent IS NULL");
        }
        if let Some(executor_id) = filters.executor {
            push_condition!("e.executor = ", executor_id);
        }
        if let Some(rule_ref) = &filters.rule_ref {
            push_condition!("enf.rule_ref = ", rule_ref.clone());
        }
        if let Some(trigger_ref) = &filters.trigger_ref {
            push_condition!("enf.trigger_ref = ", trigger_ref.clone());
        }
        if let Some(search) = &filters.result_contains {
            let pattern = format!("%{}%", search.to_lowercase());
            push_condition!("LOWER(e.result::text) LIKE ", pattern);
        }

        // ── COUNT query ──────────────────────────────────────────────────
        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(db)
            .await?;
        let total = total.max(0) as u64;

        // ── Data query with ORDER BY + pagination ────────────────────────
        qb.push(" ORDER BY e.created DESC");
        qb.push(" LIMIT ");
        qb.push_bind(filters.limit as i64);
        qb.push(" OFFSET ");
        qb.push_bind(filters.offset as i64);

        let rows: Vec<ExecutionWithRefs> = qb
            .build_query_as()
            .fetch_all(db)
            .await?;

        Ok(ExecutionSearchResult { rows, total })
    }
}
