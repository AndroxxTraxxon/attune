//! Artifact repository for database operations

use crate::models::{
    artifact::*,
    enums::{ArtifactType, OwnerType, RetentionPolicyType},
};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Repository, Update};

pub struct ArtifactRepository;

impl Repository for ArtifactRepository {
    type Entity = Artifact;
    fn table_name() -> &'static str {
        "artifact"
    }
}

#[derive(Debug, Clone)]
pub struct CreateArtifactInput {
    pub r#ref: String,
    pub scope: OwnerType,
    pub owner: String,
    pub r#type: ArtifactType,
    pub retention_policy: RetentionPolicyType,
    pub retention_limit: i32,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateArtifactInput {
    pub r#ref: Option<String>,
    pub scope: Option<OwnerType>,
    pub owner: Option<String>,
    pub r#type: Option<ArtifactType>,
    pub retention_policy: Option<RetentionPolicyType>,
    pub retention_limit: Option<i32>,
}

#[async_trait::async_trait]
impl FindById for ArtifactRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl FindByRef for ArtifactRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE ref = $1",
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for ArtifactRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             ORDER BY created DESC
             LIMIT 1000",
        )
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for ArtifactRepository {
    type CreateInput = CreateArtifactInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "INSERT INTO artifact (ref, scope, owner, type, retention_policy, retention_limit)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, ref, scope, owner, type, retention_policy, retention_limit, created, updated",
        )
        .bind(&input.r#ref)
        .bind(input.scope)
        .bind(&input.owner)
        .bind(input.r#type)
        .bind(input.retention_policy)
        .bind(input.retention_limit)
        .fetch_one(executor)
        .await
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Update for ArtifactRepository {
    type UpdateInput = UpdateArtifactInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query dynamically
        let mut query = QueryBuilder::new("UPDATE artifact SET ");
        let mut has_updates = false;

        if let Some(ref_value) = &input.r#ref {
            query.push("ref = ").push_bind(ref_value);
            has_updates = true;
        }
        if let Some(scope) = input.scope {
            if has_updates {
                query.push(", ");
            }
            query.push("scope = ").push_bind(scope);
            has_updates = true;
        }
        if let Some(owner) = &input.owner {
            if has_updates {
                query.push(", ");
            }
            query.push("owner = ").push_bind(owner);
            has_updates = true;
        }
        if let Some(artifact_type) = input.r#type {
            if has_updates {
                query.push(", ");
            }
            query.push("type = ").push_bind(artifact_type);
            has_updates = true;
        }
        if let Some(retention_policy) = input.retention_policy {
            if has_updates {
                query.push(", ");
            }
            query
                .push("retention_policy = ")
                .push_bind(retention_policy);
            has_updates = true;
        }
        if let Some(retention_limit) = input.retention_limit {
            if has_updates {
                query.push(", ");
            }
            query.push("retention_limit = ").push_bind(retention_limit);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(" RETURNING id, ref, scope, owner, type, retention_policy, retention_limit, created, updated");

        query
            .build_query_as::<Artifact>()
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for ArtifactRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM artifact WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl ArtifactRepository {
    /// Find artifacts by scope
    pub async fn find_by_scope<'e, E>(executor: E, scope: OwnerType) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE scope = $1
             ORDER BY created DESC",
        )
        .bind(scope)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// Find artifacts by owner
    pub async fn find_by_owner<'e, E>(executor: E, owner: &str) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE owner = $1
             ORDER BY created DESC",
        )
        .bind(owner)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// Find artifacts by type
    pub async fn find_by_type<'e, E>(
        executor: E,
        artifact_type: ArtifactType,
    ) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE type = $1
             ORDER BY created DESC",
        )
        .bind(artifact_type)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// Find artifacts by scope and owner (common query pattern)
    pub async fn find_by_scope_and_owner<'e, E>(
        executor: E,
        scope: OwnerType,
        owner: &str,
    ) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE scope = $1 AND owner = $2
             ORDER BY created DESC",
        )
        .bind(scope)
        .bind(owner)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// Find artifacts by retention policy
    pub async fn find_by_retention_policy<'e, E>(
        executor: E,
        retention_policy: RetentionPolicyType,
    ) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Artifact>(
            "SELECT id, ref, scope, owner, type, retention_policy, retention_limit, created, updated
             FROM artifact
             WHERE retention_policy = $1
             ORDER BY created DESC",
        )
        .bind(retention_policy)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }
}
