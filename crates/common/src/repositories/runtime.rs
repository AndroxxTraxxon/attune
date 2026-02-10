//! Runtime and Worker repository for database operations
//!
//! This module provides CRUD operations and queries for Runtime and Worker entities.

use crate::models::{
    enums::{WorkerStatus, WorkerType},
    runtime::*,
    Id, JsonDict,
};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

/// Repository for Runtime operations
pub struct RuntimeRepository;

impl Repository for RuntimeRepository {
    type Entity = Runtime;

    fn table_name() -> &'static str {
        "runtime"
    }
}

/// Input for creating a new runtime
#[derive(Debug, Clone)]
pub struct CreateRuntimeInput {
    pub r#ref: String,
    pub pack: Option<Id>,
    pub pack_ref: Option<String>,
    pub description: Option<String>,
    pub name: String,
    pub distributions: JsonDict,
    pub installation: Option<JsonDict>,
}

/// Input for updating a runtime
#[derive(Debug, Clone, Default)]
pub struct UpdateRuntimeInput {
    pub description: Option<String>,
    pub name: Option<String>,
    pub distributions: Option<JsonDict>,
    pub installation: Option<JsonDict>,
}

#[async_trait::async_trait]
impl FindById for RuntimeRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let runtime = sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, created, updated
            FROM runtime
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(runtime)
    }
}

#[async_trait::async_trait]
impl FindByRef for RuntimeRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let runtime = sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, created, updated
            FROM runtime
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(runtime)
    }
}

#[async_trait::async_trait]
impl List for RuntimeRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let runtimes = sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, created, updated
            FROM runtime
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(runtimes)
    }
}

#[async_trait::async_trait]
impl Create for RuntimeRepository {
    type CreateInput = CreateRuntimeInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let runtime = sqlx::query_as::<_, Runtime>(
            r#"
            INSERT INTO runtime (ref, pack, pack_ref, description, name,
                                 distributions, installation, installers)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, ref, pack, pack_ref, description, name,
                      distributions, installation, installers, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(input.pack)
        .bind(&input.pack_ref)
        .bind(&input.description)
        .bind(&input.name)
        .bind(&input.distributions)
        .bind(&input.installation)
        .bind(serde_json::json!({}))
        .fetch_one(executor)
        .await?;

        Ok(runtime)
    }
}

#[async_trait::async_trait]
impl Update for RuntimeRepository {
    type UpdateInput = UpdateRuntimeInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE runtime SET ");
        let mut has_updates = false;

        if let Some(description) = &input.description {
            query.push("description = ");
            query.push_bind(description);
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

        if let Some(distributions) = &input.distributions {
            if has_updates {
                query.push(", ");
            }
            query.push("distributions = ");
            query.push_bind(distributions);
            has_updates = true;
        }

        if let Some(installation) = &input.installation {
            if has_updates {
                query.push(", ");
            }
            query.push("installation = ");
            query.push_bind(installation);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, pack, pack_ref, description, name, distributions, installation, installers, created, updated");

        let runtime = query
            .build_query_as::<Runtime>()
            .fetch_one(executor)
            .await?;

        Ok(runtime)
    }
}

#[async_trait::async_trait]
impl Delete for RuntimeRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM runtime WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl RuntimeRepository {
    /// Find runtimes by pack
    pub async fn find_by_pack<'e, E>(executor: E, pack_id: Id) -> Result<Vec<Runtime>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let runtimes = sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, created, updated
            FROM runtime
            WHERE pack = $1
            ORDER BY ref ASC
            "#,
        )
        .bind(pack_id)
        .fetch_all(executor)
        .await?;

        Ok(runtimes)
    }
}

// ============================================================================
// Worker Repository
// ============================================================================

/// Repository for Worker operations
pub struct WorkerRepository;

impl Repository for WorkerRepository {
    type Entity = Worker;

    fn table_name() -> &'static str {
        "worker"
    }
}

/// Input for creating a new worker
#[derive(Debug, Clone)]
pub struct CreateWorkerInput {
    pub name: String,
    pub worker_type: WorkerType,
    pub runtime: Option<Id>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub status: Option<WorkerStatus>,
    pub capabilities: Option<JsonDict>,
    pub meta: Option<JsonDict>,
}

/// Input for updating a worker
#[derive(Debug, Clone, Default)]
pub struct UpdateWorkerInput {
    pub name: Option<String>,
    pub status: Option<WorkerStatus>,
    pub capabilities: Option<JsonDict>,
    pub meta: Option<JsonDict>,
    pub host: Option<String>,
    pub port: Option<i32>,
}

#[async_trait::async_trait]
impl FindById for WorkerRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let worker = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(worker)
    }
}

#[async_trait::async_trait]
impl List for WorkerRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            ORDER BY name ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(workers)
    }
}

#[async_trait::async_trait]
impl Create for WorkerRepository {
    type CreateInput = CreateWorkerInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let worker = sqlx::query_as::<_, Worker>(
            r#"
            INSERT INTO worker (name, worker_type, runtime, host, port, status,
                                capabilities, meta)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, worker_type, runtime, host, port, status,
                      capabilities, meta, last_heartbeat, created, updated
            "#,
        )
        .bind(&input.name)
        .bind(input.worker_type)
        .bind(input.runtime)
        .bind(&input.host)
        .bind(input.port)
        .bind(input.status)
        .bind(&input.capabilities)
        .bind(&input.meta)
        .fetch_one(executor)
        .await?;

        Ok(worker)
    }
}

#[async_trait::async_trait]
impl Update for WorkerRepository {
    type UpdateInput = UpdateWorkerInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query

        let mut query = QueryBuilder::new("UPDATE worker SET ");
        let mut has_updates = false;

        if let Some(name) = &input.name {
            query.push("name = ");
            query.push_bind(name);
            has_updates = true;
        }

        if let Some(status) = input.status {
            if has_updates {
                query.push(", ");
            }
            query.push("status = ");
            query.push_bind(status);
            has_updates = true;
        }

        if let Some(capabilities) = &input.capabilities {
            if has_updates {
                query.push(", ");
            }
            query.push("capabilities = ");
            query.push_bind(capabilities);
            has_updates = true;
        }

        if let Some(meta) = &input.meta {
            if has_updates {
                query.push(", ");
            }
            query.push("meta = ");
            query.push_bind(meta);
            has_updates = true;
        }

        if let Some(host) = &input.host {
            if has_updates {
                query.push(", ");
            }
            query.push("host = ");
            query.push_bind(host);
            has_updates = true;
        }

        if let Some(port) = input.port {
            if has_updates {
                query.push(", ");
            }
            query.push("port = ");
            query.push_bind(port);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, name, worker_type, worker_role, runtime, host, port, status, capabilities, meta, last_heartbeat, created, updated");

        let worker = query.build_query_as::<Worker>().fetch_one(executor).await?;

        Ok(worker)
    }
}

#[async_trait::async_trait]
impl Delete for WorkerRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM worker WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl WorkerRepository {
    /// Find workers by status
    pub async fn find_by_status<'e, E>(executor: E, status: WorkerStatus) -> Result<Vec<Worker>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            WHERE status = $1
            ORDER BY name ASC
            "#,
        )
        .bind(status)
        .fetch_all(executor)
        .await?;

        Ok(workers)
    }

    /// Find workers by type
    pub async fn find_by_type<'e, E>(executor: E, worker_type: WorkerType) -> Result<Vec<Worker>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            WHERE worker_type = $1
            ORDER BY name ASC
            "#,
        )
        .bind(worker_type)
        .fetch_all(executor)
        .await?;

        Ok(workers)
    }

    /// Update worker heartbeat
    pub async fn update_heartbeat<'e, E>(executor: E, id: i64) -> Result<()>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query("UPDATE worker SET last_heartbeat = NOW() WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// Find workers by name
    pub async fn find_by_name<'e, E>(executor: E, name: &str) -> Result<Option<Worker>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let worker = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(executor)
        .await?;

        Ok(worker)
    }

    /// Find workers that can execute actions (role = 'action')
    pub async fn find_action_workers<'e, E>(executor: E) -> Result<Vec<Worker>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT id, name, worker_type, worker_role, runtime, host, port, status,
                   capabilities, meta, last_heartbeat, created, updated
            FROM worker
            WHERE worker_role = 'action'
            ORDER BY name ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(workers)
    }
}
