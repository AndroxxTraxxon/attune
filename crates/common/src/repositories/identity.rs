//! Identity and permission repository for database operations

use crate::models::{identity::*, Id, JsonDict};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

pub struct IdentityRepository;

impl Repository for IdentityRepository {
    type Entity = Identity;
    fn table_name() -> &'static str {
        "identities"
    }
}

#[derive(Debug, Clone)]
pub struct CreateIdentityInput {
    pub login: String,
    pub display_name: Option<String>,
    pub password_hash: Option<String>,
    pub attributes: JsonDict,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateIdentityInput {
    pub display_name: Option<String>,
    pub password_hash: Option<String>,
    pub attributes: Option<JsonDict>,
}

#[async_trait::async_trait]
impl FindById for IdentityRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Identity>(
            "SELECT id, login, display_name, password_hash, attributes, created, updated FROM identity WHERE id = $1"
        ).bind(id).fetch_optional(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for IdentityRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Identity>(
            "SELECT id, login, display_name, password_hash, attributes, created, updated FROM identity ORDER BY login ASC"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for IdentityRepository {
    type CreateInput = CreateIdentityInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Identity>(
            "INSERT INTO identity (login, display_name, password_hash, attributes) VALUES ($1, $2, $3, $4) RETURNING id, login, display_name, password_hash, attributes, created, updated"
        )
        .bind(&input.login)
        .bind(&input.display_name)
        .bind(&input.password_hash)
        .bind(&input.attributes)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            // Convert unique constraint violation to AlreadyExists error
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return crate::Error::already_exists("Identity", "login", &input.login);
                }
            }
            e.into()
        })
    }
}

#[async_trait::async_trait]
impl Update for IdentityRepository {
    type UpdateInput = UpdateIdentityInput;
    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query
        let mut query = QueryBuilder::new("UPDATE identity SET ");
        let mut has_updates = false;

        if let Some(display_name) = &input.display_name {
            query.push("display_name = ").push_bind(display_name);
            has_updates = true;
        }
        if let Some(password_hash) = &input.password_hash {
            if has_updates {
                query.push(", ");
            }
            query.push("password_hash = ").push_bind(password_hash);
            has_updates = true;
        }
        if let Some(attributes) = &input.attributes {
            if has_updates {
                query.push(", ");
            }
            query.push("attributes = ").push_bind(attributes);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(
            " RETURNING id, login, display_name, password_hash, attributes, created, updated",
        );

        query
            .build_query_as::<Identity>()
            .fetch_one(executor)
            .await
            .map_err(|e| {
                // Convert RowNotFound to NotFound error
                if matches!(e, sqlx::Error::RowNotFound) {
                    return crate::Error::not_found("identity", "id", &id.to_string());
                }
                e.into()
            })
    }
}

#[async_trait::async_trait]
impl Delete for IdentityRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM identity WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl IdentityRepository {
    pub async fn find_by_login<'e, E>(executor: E, login: &str) -> Result<Option<Identity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Identity>(
            "SELECT id, login, display_name, password_hash, attributes, created, updated FROM identity WHERE login = $1"
        ).bind(login).fetch_optional(executor).await.map_err(Into::into)
    }
}

// Permission Set Repository
pub struct PermissionSetRepository;

impl Repository for PermissionSetRepository {
    type Entity = PermissionSet;
    fn table_name() -> &'static str {
        "permission_set"
    }
}

#[derive(Debug, Clone)]
pub struct CreatePermissionSetInput {
    pub r#ref: String,
    pub pack: Option<Id>,
    pub pack_ref: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub grants: serde_json::Value,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatePermissionSetInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub grants: Option<serde_json::Value>,
}

#[async_trait::async_trait]
impl FindById for PermissionSetRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionSet>(
            "SELECT id, ref, pack, pack_ref, label, description, grants, created, updated FROM permission_set WHERE id = $1"
        ).bind(id).fetch_optional(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for PermissionSetRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionSet>(
            "SELECT id, ref, pack, pack_ref, label, description, grants, created, updated FROM permission_set ORDER BY ref ASC"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for PermissionSetRepository {
    type CreateInput = CreatePermissionSetInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionSet>(
            "INSERT INTO permission_set (ref, pack, pack_ref, label, description, grants) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, ref, pack, pack_ref, label, description, grants, created, updated"
        ).bind(&input.r#ref).bind(input.pack).bind(&input.pack_ref).bind(&input.label).bind(&input.description).bind(&input.grants).fetch_one(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Update for PermissionSetRepository {
    type UpdateInput = UpdatePermissionSetInput;
    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query
        let mut query = QueryBuilder::new("UPDATE permission_set SET ");
        let mut has_updates = false;

        if let Some(label) = &input.label {
            query.push("label = ").push_bind(label);
            has_updates = true;
        }
        if let Some(description) = &input.description {
            if has_updates {
                query.push(", ");
            }
            query.push("description = ").push_bind(description);
            has_updates = true;
        }
        if let Some(grants) = &input.grants {
            if has_updates {
                query.push(", ");
            }
            query.push("grants = ").push_bind(grants);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(
            " RETURNING id, ref, pack, pack_ref, label, description, grants, created, updated",
        );

        query
            .build_query_as::<PermissionSet>()
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for PermissionSetRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM permission_set WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

// Permission Assignment Repository
pub struct PermissionAssignmentRepository;

impl Repository for PermissionAssignmentRepository {
    type Entity = PermissionAssignment;
    fn table_name() -> &'static str {
        "permission_assignment"
    }
}

#[derive(Debug, Clone)]
pub struct CreatePermissionAssignmentInput {
    pub identity: Id,
    pub permset: Id,
}

#[async_trait::async_trait]
impl FindById for PermissionAssignmentRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionAssignment>(
            "SELECT id, identity, permset, created FROM permission_assignment WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for PermissionAssignmentRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionAssignment>(
            "SELECT id, identity, permset, created FROM permission_assignment ORDER BY created DESC"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for PermissionAssignmentRepository {
    type CreateInput = CreatePermissionAssignmentInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionAssignment>(
            "INSERT INTO permission_assignment (identity, permset) VALUES ($1, $2) RETURNING id, identity, permset, created"
        ).bind(input.identity).bind(input.permset).fetch_one(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for PermissionAssignmentRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM permission_assignment WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl PermissionAssignmentRepository {
    pub async fn find_by_identity<'e, E>(
        executor: E,
        identity_id: Id,
    ) -> Result<Vec<PermissionAssignment>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, PermissionAssignment>(
            "SELECT id, identity, permset, created FROM permission_assignment WHERE identity = $1",
        )
        .bind(identity_id)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }
}
