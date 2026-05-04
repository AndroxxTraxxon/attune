//! Repository for API-managed pack registry indices.

use crate::models::PackRegistryIndex;
use crate::{Error, Result};
use sqlx::{Executor, Postgres};

use super::{Delete, FindById, List, Repository};

pub struct PackRegistryIndexRepository;

impl Repository for PackRegistryIndexRepository {
    type Entity = PackRegistryIndex;

    fn table_name() -> &'static str {
        "pack_registry_index"
    }
}

#[derive(Debug, Clone)]
pub struct CreatePackRegistryIndexInput {
    pub name: Option<String>,
    pub url: String,
    pub position: Option<i32>,
    pub enabled: bool,
    pub headers: serde_json::Value,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatePackRegistryIndexInput {
    pub name: Option<Option<String>>,
    pub url: Option<String>,
    pub position: Option<i32>,
    pub enabled: Option<bool>,
    pub headers: Option<serde_json::Value>,
}

const COLUMNS: &str = "id, name, url, position, enabled, headers, created, updated";

#[async_trait::async_trait]
impl FindById for PackRegistryIndexRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!("SELECT {} FROM pack_registry_index WHERE id = $1", COLUMNS);
        sqlx::query_as::<_, PackRegistryIndex>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for PackRegistryIndexRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM pack_registry_index ORDER BY position ASC, id ASC",
            COLUMNS
        );
        sqlx::query_as::<_, PackRegistryIndex>(&query)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for PackRegistryIndexRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM pack_registry_index WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl PackRegistryIndexRepository {
    pub async fn create<'e, E>(
        executor: E,
        input: CreatePackRegistryIndexInput,
    ) -> Result<PackRegistryIndex>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        validate_url(&input.url)?;
        if matches!(input.position, Some(position) if position < 0) {
            return Err(Error::validation("Index position must be non-negative"));
        }

        let query = format!(
            r#"
            INSERT INTO pack_registry_index (name, url, position, enabled, headers)
            VALUES ($1, $2, COALESCE($3, (SELECT COALESCE(MAX(position), -1) + 1 FROM pack_registry_index)), $4, $5)
            RETURNING {}
            "#,
            COLUMNS
        );
        sqlx::query_as::<_, PackRegistryIndex>(&query)
            .bind(input.name)
            .bind(input.url)
            .bind(input.position)
            .bind(input.enabled)
            .bind(input.headers)
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }

    pub async fn update<'e, E>(
        executor: E,
        id: i64,
        input: UpdatePackRegistryIndexInput,
    ) -> Result<PackRegistryIndex>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        if let Some(url) = &input.url {
            validate_url(url)?;
        }
        if matches!(input.position, Some(position) if position < 0) {
            return Err(Error::validation("Index position must be non-negative"));
        }

        let update_name = input.name.is_some();
        let name = input.name.flatten();

        let query = format!(
            r#"
            UPDATE pack_registry_index
            SET name = CASE WHEN $2 THEN $3 ELSE name END,
                url = COALESCE($4, url),
                position = COALESCE($5, position),
                enabled = COALESCE($6, enabled),
                headers = COALESCE($7, headers)
            WHERE id = $1
            RETURNING {}
            "#,
            COLUMNS
        );
        sqlx::query_as::<_, PackRegistryIndex>(&query)
            .bind(id)
            .bind(update_name)
            .bind(name)
            .bind(input.url)
            .bind(input.position)
            .bind(input.enabled)
            .bind(input.headers)
            .fetch_one(executor)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    Error::not_found("pack_registry_index", "id", id.to_string())
                }
                other => other.into(),
            })
    }
}

fn validate_url(url: &str) -> Result<()> {
    if url.trim().is_empty() {
        return Err(Error::validation("Index URL must not be empty"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("file://")) {
        return Err(Error::validation(
            "Index URL must start with https://, http://, or file://",
        ));
    }
    Ok(())
}
