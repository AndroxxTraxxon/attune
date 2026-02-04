//! Pack repository for database operations on packs
//!
//! This module provides CRUD operations and queries for Pack entities.

use crate::models::{pack::Pack, JsonDict, JsonSchema};
use crate::{Error, Result};
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Pagination, Repository, Update};

/// Repository for Pack operations
pub struct PackRepository;

impl Repository for PackRepository {
    type Entity = Pack;

    fn table_name() -> &'static str {
        "pack"
    }
}

/// Input for creating a new pack
#[derive(Debug, Clone)]
pub struct CreatePackInput {
    pub r#ref: String,
    pub label: String,
    pub description: Option<String>,
    pub version: String,
    pub conf_schema: JsonSchema,
    pub config: JsonDict,
    pub meta: JsonDict,
    pub tags: Vec<String>,
    pub runtime_deps: Vec<String>,
    pub is_standard: bool,
}

/// Input for updating a pack
#[derive(Debug, Clone, Default)]
pub struct UpdatePackInput {
    pub label: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub conf_schema: Option<JsonSchema>,
    pub config: Option<JsonDict>,
    pub meta: Option<JsonDict>,
    pub tags: Option<Vec<String>>,
    pub runtime_deps: Option<Vec<String>>,
    pub is_standard: Option<bool>,
}

#[async_trait::async_trait]
impl FindById for PackRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let pack = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(pack)
    }
}

#[async_trait::async_trait]
impl FindByRef for PackRepository {
    async fn find_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let pack = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            WHERE ref = $1
            "#,
        )
        .bind(ref_str)
        .fetch_optional(executor)
        .await?;

        Ok(pack)
    }
}

#[async_trait::async_trait]
impl List for PackRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let packs = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(packs)
    }
}

#[async_trait::async_trait]
impl Create for PackRepository {
    type CreateInput = CreatePackInput;

    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Validate ref format (alphanumeric, dots, underscores, hyphens)
        if !input
            .r#ref
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err(Error::validation(
                "Pack ref must contain only alphanumeric characters, dots, underscores, and hyphens",
            ));
        }

        // Try to insert - database will enforce uniqueness constraint
        let pack = sqlx::query_as::<_, Pack>(
            r#"
            INSERT INTO pack (ref, label, description, version, conf_schema, config, meta,
                              tags, runtime_deps, is_standard)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, ref, label, description, version, conf_schema, config, meta,
                      tags, runtime_deps, is_standard, created, updated
            "#,
        )
        .bind(&input.r#ref)
        .bind(&input.label)
        .bind(&input.description)
        .bind(&input.version)
        .bind(&input.conf_schema)
        .bind(&input.config)
        .bind(&input.meta)
        .bind(&input.tags)
        .bind(&input.runtime_deps)
        .bind(input.is_standard)
        .fetch_one(executor)
        .await
        .map_err(|e| {
            // Convert unique constraint violation to AlreadyExists error
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return Error::already_exists("Pack", "ref", &input.r#ref);
                }
            }
            e.into()
        })?;

        Ok(pack)
    }
}

#[async_trait::async_trait]
impl Update for PackRepository {
    type UpdateInput = UpdatePackInput;

    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build dynamic UPDATE query
        let mut query = QueryBuilder::new("UPDATE pack SET ");
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

        if let Some(version) = &input.version {
            if has_updates {
                query.push(", ");
            }
            query.push("version = ");
            query.push_bind(version);
            has_updates = true;
        }

        if let Some(conf_schema) = &input.conf_schema {
            if has_updates {
                query.push(", ");
            }
            query.push("conf_schema = ");
            query.push_bind(conf_schema);
            has_updates = true;
        }

        if let Some(config) = &input.config {
            if has_updates {
                query.push(", ");
            }
            query.push("config = ");
            query.push_bind(config);
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

        if let Some(tags) = &input.tags {
            if has_updates {
                query.push(", ");
            }
            query.push("tags = ");
            query.push_bind(tags);
            has_updates = true;
        }

        if let Some(runtime_deps) = &input.runtime_deps {
            if has_updates {
                query.push(", ");
            }
            query.push("runtime_deps = ");
            query.push_bind(runtime_deps);
            has_updates = true;
        }

        if let Some(is_standard) = input.is_standard {
            if has_updates {
                query.push(", ");
            }
            query.push("is_standard = ");
            query.push_bind(is_standard);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing pack
            return Self::find_by_id(executor, id)
                .await?
                .ok_or_else(|| Error::not_found("pack", "id", id.to_string()));
        }

        // Add updated timestamp
        query.push(", updated = NOW() WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING id, ref, label, description, version, conf_schema, config, meta, tags, runtime_deps, is_standard, created, updated");

        let pack = query
            .build_query_as::<Pack>()
            .fetch_one(executor)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => Error::not_found("pack", "id", id.to_string()),
                _ => e.into(),
            })?;

        Ok(pack)
    }
}

#[async_trait::async_trait]
impl Delete for PackRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM pack WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl PackRepository {
    /// List packs with pagination
    pub async fn list_paginated<'e, E>(executor: E, pagination: Pagination) -> Result<Vec<Pack>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let packs = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            ORDER BY ref ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(executor)
        .await?;

        Ok(packs)
    }

    /// Count total number of packs
    pub async fn count<'e, E>(executor: E) -> Result<i64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pack")
            .fetch_one(executor)
            .await?;

        Ok(count.0)
    }

    /// Find packs by tag
    pub async fn find_by_tag<'e, E>(executor: E, tag: &str) -> Result<Vec<Pack>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let packs = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            WHERE $1 = ANY(tags)
            ORDER BY ref ASC
            "#,
        )
        .bind(tag)
        .fetch_all(executor)
        .await?;

        Ok(packs)
    }

    /// Find standard packs
    pub async fn find_standard<'e, E>(executor: E) -> Result<Vec<Pack>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let packs = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            WHERE is_standard = true
            ORDER BY ref ASC
            "#,
        )
        .fetch_all(executor)
        .await?;

        Ok(packs)
    }

    /// Search packs by name/label (case-insensitive)
    pub async fn search<'e, E>(executor: E, query: &str) -> Result<Vec<Pack>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let search_pattern = format!("%{}%", query.to_lowercase());
        let packs = sqlx::query_as::<_, Pack>(
            r#"
            SELECT id, ref, label, description, version, conf_schema, config, meta,
                   tags, runtime_deps, is_standard, created, updated
            FROM pack
            WHERE LOWER(ref) LIKE $1 OR LOWER(label) LIKE $1 OR LOWER(description) LIKE $1
            ORDER BY ref ASC
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(executor)
        .await?;

        Ok(packs)
    }

    /// Check if a pack with the given ref exists
    pub async fn exists_by_ref<'e, E>(executor: E, ref_str: &str) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let exists: (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM pack WHERE ref = $1)")
                .bind(ref_str)
                .fetch_one(executor)
                .await?;

        Ok(exists.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pack_input() {
        let input = CreatePackInput {
            r#ref: "test.pack".to_string(),
            label: "Test Pack".to_string(),
            description: Some("A test pack".to_string()),
            version: "1.0.0".to_string(),
            conf_schema: serde_json::json!({}),
            config: serde_json::json!({}),
            meta: serde_json::json!({}),
            tags: vec!["test".to_string()],
            runtime_deps: vec![],
            is_standard: false,
        };

        assert_eq!(input.r#ref, "test.pack");
        assert_eq!(input.label, "Test Pack");
    }

    #[test]
    fn test_update_pack_input_default() {
        let input = UpdatePackInput::default();
        assert!(input.label.is_none());
        assert!(input.description.is_none());
        assert!(input.version.is_none());
    }
}
