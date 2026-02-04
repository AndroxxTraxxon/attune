//! Key/Secret repository for database operations

use crate::models::{key::*, Id, OwnerType};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

pub struct KeyRepository;

impl Repository for KeyRepository {
    type Entity = Key;
    fn table_name() -> &'static str {
        "key"
    }
}

#[derive(Debug, Clone)]
pub struct CreateKeyInput {
    pub r#ref: String,
    pub owner_type: OwnerType,
    pub owner: Option<String>,
    pub owner_identity: Option<Id>,
    pub owner_pack: Option<Id>,
    pub owner_pack_ref: Option<String>,
    pub owner_action: Option<Id>,
    pub owner_action_ref: Option<String>,
    pub owner_sensor: Option<Id>,
    pub owner_sensor_ref: Option<String>,
    pub name: String,
    pub encrypted: bool,
    pub encryption_key_hash: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateKeyInput {
    pub name: Option<String>,
    pub value: Option<String>,
    pub encrypted: Option<bool>,
    pub encryption_key_hash: Option<String>,
}

#[async_trait::async_trait]
impl FindById for KeyRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated FROM key WHERE id = $1"
        ).bind(id).fetch_optional(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for KeyRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated FROM key ORDER BY ref ASC"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for KeyRepository {
    type CreateInput = CreateKeyInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Key>(
            "INSERT INTO key (ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated"
        ).bind(&input.r#ref).bind(input.owner_type).bind(&input.owner).bind(input.owner_identity).bind(input.owner_pack).bind(&input.owner_pack_ref).bind(input.owner_action).bind(&input.owner_action_ref).bind(input.owner_sensor).bind(&input.owner_sensor_ref).bind(&input.name).bind(input.encrypted).bind(&input.encryption_key_hash).bind(&input.value).fetch_one(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Update for KeyRepository {
    type UpdateInput = UpdateKeyInput;
    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query
        let mut query = QueryBuilder::new("UPDATE key SET ");
        let mut has_updates = false;

        if let Some(name) = &input.name {
            query.push("name = ").push_bind(name);
            has_updates = true;
        }
        if let Some(value) = &input.value {
            if has_updates {
                query.push(", ");
            }
            query.push("value = ").push_bind(value);
            has_updates = true;
        }
        if let Some(encrypted) = input.encrypted {
            if has_updates {
                query.push(", ");
            }
            query.push("encrypted = ").push_bind(encrypted);
            has_updates = true;
        }
        if let Some(encryption_key_hash) = &input.encryption_key_hash {
            if has_updates {
                query.push(", ");
            }
            query
                .push("encryption_key_hash = ")
                .push_bind(encryption_key_hash);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(" RETURNING id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated");

        query
            .build_query_as::<Key>()
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for KeyRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM key WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl KeyRepository {
    pub async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Key>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated FROM key WHERE ref = $1"
        ).bind(ref_str).fetch_optional(executor).await.map_err(Into::into)
    }

    pub async fn find_by_owner_type<'e, E>(executor: E, owner_type: OwnerType) -> Result<Vec<Key>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref, owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted, encryption_key_hash, value, created, updated FROM key WHERE owner_type = $1 ORDER BY ref ASC"
        ).bind(owner_type).fetch_all(executor).await.map_err(Into::into)
    }
}
