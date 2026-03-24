//! Artifact and ArtifactVersion repositories for database operations

use crate::models::{
    artifact::*,
    artifact_version::ArtifactVersion,
    enums::{ArtifactType, ArtifactVisibility, OwnerType, RetentionPolicyType},
};
use crate::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, FindByRef, List, Patch, Repository, Update};

// ============================================================================
// ArtifactRepository
// ============================================================================

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
    pub visibility: ArtifactVisibility,
    pub retention_policy: RetentionPolicyType,
    pub retention_limit: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub content_type: Option<String>,
    pub execution: Option<i64>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateArtifactInput {
    pub r#ref: Option<String>,
    pub scope: Option<OwnerType>,
    pub owner: Option<String>,
    pub r#type: Option<ArtifactType>,
    pub visibility: Option<ArtifactVisibility>,
    pub retention_policy: Option<RetentionPolicyType>,
    pub retention_limit: Option<i32>,
    pub name: Option<Patch<String>>,
    pub description: Option<Patch<String>>,
    pub content_type: Option<Patch<String>>,
    pub size_bytes: Option<i64>,
    pub execution: Option<Patch<i64>>,
    pub data: Option<Patch<serde_json::Value>>,
}

/// Filters for searching artifacts
#[derive(Debug, Clone, Default)]
pub struct ArtifactSearchFilters {
    pub scope: Option<OwnerType>,
    pub owner: Option<String>,
    pub r#type: Option<ArtifactType>,
    pub visibility: Option<ArtifactVisibility>,
    pub execution: Option<i64>,
    pub name_contains: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// Search result with total count
pub struct ArtifactSearchResult {
    pub rows: Vec<Artifact>,
    pub total: i64,
}

#[async_trait::async_trait]
impl FindById for ArtifactRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!("SELECT {} FROM artifact WHERE id = $1", SELECT_COLUMNS);
        sqlx::query_as::<_, Artifact>(&query)
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
        let query = format!("SELECT {} FROM artifact WHERE ref = $1", SELECT_COLUMNS);
        sqlx::query_as::<_, Artifact>(&query)
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
        let query = format!(
            "SELECT {} FROM artifact ORDER BY created DESC LIMIT 1000",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
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
        let query = format!(
            "INSERT INTO artifact (ref, scope, owner, type, visibility, retention_policy, retention_limit, \
             name, description, content_type, execution, data) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             RETURNING {}",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(&input.r#ref)
            .bind(input.scope)
            .bind(&input.owner)
            .bind(input.r#type)
            .bind(input.visibility)
            .bind(input.retention_policy)
            .bind(input.retention_limit)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.content_type)
            .bind(input.execution)
            .bind(&input.data)
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
        let mut query = QueryBuilder::new("UPDATE artifact SET ");
        let mut has_updates = false;

        macro_rules! push_field {
            ($field:expr, $col:expr) => {
                if let Some(val) = $field {
                    if has_updates {
                        query.push(", ");
                    }
                    query.push(concat!($col, " = ")).push_bind(val);
                    has_updates = true;
                }
            };
        }

        push_field!(&input.r#ref, "ref");
        push_field!(input.scope, "scope");
        push_field!(&input.owner, "owner");
        push_field!(input.r#type, "type");
        push_field!(input.visibility, "visibility");
        push_field!(input.retention_policy, "retention_policy");
        push_field!(input.retention_limit, "retention_limit");
        if let Some(name) = &input.name {
            if has_updates {
                query.push(", ");
            }
            query.push("name = ");
            match name {
                Patch::Set(value) => query.push_bind(value),
                Patch::Clear => query.push_bind(Option::<String>::None),
            };
            has_updates = true;
        }
        if let Some(description) = &input.description {
            if has_updates {
                query.push(", ");
            }
            query.push("description = ");
            match description {
                Patch::Set(value) => query.push_bind(value),
                Patch::Clear => query.push_bind(Option::<String>::None),
            };
            has_updates = true;
        }
        if let Some(content_type) = &input.content_type {
            if has_updates {
                query.push(", ");
            }
            query.push("content_type = ");
            match content_type {
                Patch::Set(value) => query.push_bind(value),
                Patch::Clear => query.push_bind(Option::<String>::None),
            };
            has_updates = true;
        }
        push_field!(input.size_bytes, "size_bytes");
        if let Some(exec_val) = input.execution {
            if has_updates {
                query.push(", ");
            }
            query.push("execution = ");
            match exec_val {
                Patch::Set(value) => query.push_bind(value),
                Patch::Clear => query.push_bind(Option::<i64>::None),
            };
            has_updates = true;
        }
        if let Some(data) = &input.data {
            if has_updates {
                query.push(", ");
            }
            query.push("data = ");
            match data {
                Patch::Set(value) => query.push_bind(value),
                Patch::Clear => query.push_bind(Option::<serde_json::Value>::None),
            };
            has_updates = true;
        }

        if !has_updates {
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(" RETURNING ");
        query.push(SELECT_COLUMNS);

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
    /// Search artifacts with filters and pagination
    pub async fn search<'e, E>(
        executor: E,
        filters: &ArtifactSearchFilters,
    ) -> Result<ArtifactSearchResult>
    where
        E: Executor<'e, Database = Postgres> + Copy + 'e,
    {
        // Build WHERE clauses
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx: usize = 0;

        if filters.scope.is_some() {
            param_idx += 1;
            conditions.push(format!("scope = ${}", param_idx));
        }
        if filters.owner.is_some() {
            param_idx += 1;
            conditions.push(format!("owner = ${}", param_idx));
        }
        if filters.r#type.is_some() {
            param_idx += 1;
            conditions.push(format!("type = ${}", param_idx));
        }
        if filters.visibility.is_some() {
            param_idx += 1;
            conditions.push(format!("visibility = ${}", param_idx));
        }
        if filters.execution.is_some() {
            param_idx += 1;
            conditions.push(format!("execution = ${}", param_idx));
        }
        if filters.name_contains.is_some() {
            param_idx += 1;
            conditions.push(format!("name ILIKE '%' || ${} || '%'", param_idx));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Count query
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM artifact {}", where_clause);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);

        // Bind params for count
        if let Some(scope) = filters.scope {
            count_query = count_query.bind(scope);
        }
        if let Some(ref owner) = filters.owner {
            count_query = count_query.bind(owner.clone());
        }
        if let Some(r#type) = filters.r#type {
            count_query = count_query.bind(r#type);
        }
        if let Some(visibility) = filters.visibility {
            count_query = count_query.bind(visibility);
        }
        if let Some(execution) = filters.execution {
            count_query = count_query.bind(execution);
        }
        if let Some(ref name) = filters.name_contains {
            count_query = count_query.bind(name.clone());
        }

        let total = count_query.fetch_one(executor).await?;

        // Data query
        let limit = filters.limit.min(1000);
        let offset = filters.offset;
        let data_sql = format!(
            "SELECT {} FROM artifact {} ORDER BY created DESC LIMIT {} OFFSET {}",
            SELECT_COLUMNS, where_clause, limit, offset
        );

        let mut data_query = sqlx::query_as::<_, Artifact>(&data_sql);

        if let Some(scope) = filters.scope {
            data_query = data_query.bind(scope);
        }
        if let Some(ref owner) = filters.owner {
            data_query = data_query.bind(owner.clone());
        }
        if let Some(r#type) = filters.r#type {
            data_query = data_query.bind(r#type);
        }
        if let Some(visibility) = filters.visibility {
            data_query = data_query.bind(visibility);
        }
        if let Some(execution) = filters.execution {
            data_query = data_query.bind(execution);
        }
        if let Some(ref name) = filters.name_contains {
            data_query = data_query.bind(name.clone());
        }

        let rows = data_query.fetch_all(executor).await?;

        Ok(ArtifactSearchResult { rows, total })
    }

    /// Find artifacts by scope
    pub async fn find_by_scope<'e, E>(executor: E, scope: OwnerType) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact WHERE scope = $1 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
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
        let query = format!(
            "SELECT {} FROM artifact WHERE owner = $1 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
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
        let query = format!(
            "SELECT {} FROM artifact WHERE type = $1 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(artifact_type)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Find artifacts by scope and owner
    pub async fn find_by_scope_and_owner<'e, E>(
        executor: E,
        scope: OwnerType,
        owner: &str,
    ) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact WHERE scope = $1 AND owner = $2 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(scope)
            .bind(owner)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Find artifacts by execution ID
    pub async fn find_by_execution<'e, E>(executor: E, execution_id: i64) -> Result<Vec<Artifact>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact WHERE execution = $1 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(execution_id)
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
        let query = format!(
            "SELECT {} FROM artifact WHERE retention_policy = $1 ORDER BY created DESC",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(retention_policy)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Append data to a progress-type artifact.
    ///
    /// If `artifact.data` is currently NULL, it is initialized as a JSON array
    /// containing the new entry. Otherwise the entry is appended to the existing
    /// array. This is done atomically in a single SQL statement.
    pub async fn append_progress<'e, E>(
        executor: E,
        id: i64,
        entry: &serde_json::Value,
    ) -> Result<Artifact>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "UPDATE artifact \
             SET data = CASE \
                 WHEN data IS NULL THEN jsonb_build_array($2::jsonb) \
                 ELSE data || jsonb_build_array($2::jsonb) \
             END, \
             updated = NOW() \
             WHERE id = $1 AND type = 'progress' \
             RETURNING {}",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(id)
            .bind(entry)
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }

    /// Replace the full data payload on a progress-type artifact (for "set" semantics).
    pub async fn set_data<'e, E>(executor: E, id: i64, data: &serde_json::Value) -> Result<Artifact>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "UPDATE artifact SET data = $2, updated = NOW() \
             WHERE id = $1 RETURNING {}",
            SELECT_COLUMNS
        );
        sqlx::query_as::<_, Artifact>(&query)
            .bind(id)
            .bind(data)
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }

    /// Update the size_bytes of an artifact (used by worker finalization to sync
    /// the parent artifact's size with the latest file-based version).
    pub async fn update_size_bytes<'e, E>(executor: E, id: i64, size_bytes: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result =
            sqlx::query("UPDATE artifact SET size_bytes = $1, updated = NOW() WHERE id = $2")
                .bind(size_bytes)
                .bind(id)
                .execute(executor)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

// ============================================================================
// ArtifactVersionRepository
// ============================================================================

use crate::models::artifact_version;

pub struct ArtifactVersionRepository;

impl Repository for ArtifactVersionRepository {
    type Entity = ArtifactVersion;
    fn table_name() -> &'static str {
        "artifact_version"
    }
}

#[derive(Debug, Clone)]
pub struct CreateArtifactVersionInput {
    pub artifact: i64,
    pub content_type: Option<String>,
    pub content: Option<Vec<u8>>,
    pub content_json: Option<serde_json::Value>,
    pub file_path: Option<String>,
    pub meta: Option<serde_json::Value>,
    pub created_by: Option<String>,
}

impl ArtifactVersionRepository {
    fn select_columns_with_alias(alias: &str) -> String {
        format!(
            "{alias}.id, {alias}.artifact, {alias}.version, {alias}.content_type, \
             {alias}.size_bytes, NULL::bytea AS content, {alias}.content_json, \
             {alias}.file_path, {alias}.meta, {alias}.created_by, {alias}.created"
        )
    }

    /// Find a version by ID (without binary content for performance)
    pub async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE id = $1",
            artifact_version::SELECT_COLUMNS
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Find a version by ID including binary content
    pub async fn find_by_id_with_content<'e, E>(
        executor: E,
        id: i64,
    ) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE id = $1",
            artifact_version::SELECT_COLUMNS_WITH_CONTENT
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// List all versions for an artifact (without binary content), newest first
    pub async fn list_by_artifact<'e, E>(
        executor: E,
        artifact_id: i64,
    ) -> Result<Vec<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 ORDER BY version DESC",
            artifact_version::SELECT_COLUMNS
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Get the latest version for an artifact (without binary content)
    pub async fn find_latest<'e, E>(
        executor: E,
        artifact_id: i64,
    ) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 ORDER BY version DESC LIMIT 1",
            artifact_version::SELECT_COLUMNS
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Get the latest version for an artifact (with binary content)
    pub async fn find_latest_with_content<'e, E>(
        executor: E,
        artifact_id: i64,
    ) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 ORDER BY version DESC LIMIT 1",
            artifact_version::SELECT_COLUMNS_WITH_CONTENT
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Get a specific version by artifact and version number (without binary content)
    pub async fn find_by_version<'e, E>(
        executor: E,
        artifact_id: i64,
        version: i32,
    ) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 AND version = $2",
            artifact_version::SELECT_COLUMNS
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .bind(version)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Get a specific version by artifact and version number (with binary content)
    pub async fn find_by_version_with_content<'e, E>(
        executor: E,
        artifact_id: i64,
        version: i32,
    ) -> Result<Option<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 AND version = $2",
            artifact_version::SELECT_COLUMNS_WITH_CONTENT
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .bind(version)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Create a new artifact version. The version number is auto-assigned
    /// (MAX(version) + 1) and the retention trigger fires after insert.
    pub async fn create<'e, E>(
        executor: E,
        input: CreateArtifactVersionInput,
    ) -> Result<ArtifactVersion>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let size_bytes = input.content.as_ref().map(|c| c.len() as i64).or_else(|| {
            input
                .content_json
                .as_ref()
                .map(|j| serde_json::to_string(j).unwrap_or_default().len() as i64)
        });

        let query = format!(
            "INSERT INTO artifact_version \
                 (artifact, version, content_type, size_bytes, content, content_json, file_path, meta, created_by) \
             VALUES ($1, next_artifact_version($1), $2, $3, $4, $5, $6, $7, $8) \
             RETURNING {}",
            artifact_version::SELECT_COLUMNS_WITH_CONTENT
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(input.artifact)
            .bind(&input.content_type)
            .bind(size_bytes)
            .bind(&input.content)
            .bind(&input.content_json)
            .bind(&input.file_path)
            .bind(&input.meta)
            .bind(&input.created_by)
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }

    /// Delete a specific version by ID
    pub async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM artifact_version WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all versions for an artifact
    pub async fn delete_all_for_artifact<'e, E>(executor: E, artifact_id: i64) -> Result<u64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM artifact_version WHERE artifact = $1")
            .bind(artifact_id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    /// Count versions for an artifact
    pub async fn count_by_artifact<'e, E>(executor: E, artifact_id: i64) -> Result<i64>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifact_version WHERE artifact = $1")
            .bind(artifact_id)
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }

    /// Update the size_bytes of a specific artifact version (used by worker finalization).
    pub async fn update_size_bytes<'e, E>(
        executor: E,
        version_id: i64,
        size_bytes: i64,
    ) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("UPDATE artifact_version SET size_bytes = $1 WHERE id = $2")
            .bind(size_bytes)
            .bind(version_id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Find all file-backed versions linked to an execution.
    /// Joins artifact_version → artifact on artifact.execution to find all
    /// file-based versions produced by a given execution.
    pub async fn find_file_versions_by_execution<'e, E>(
        executor: E,
        execution_id: i64,
    ) -> Result<Vec<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} \
             FROM artifact_version av \
             JOIN artifact a ON av.artifact = a.id \
             WHERE a.execution = $1 AND av.file_path IS NOT NULL",
            Self::select_columns_with_alias("av")
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(execution_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Find all file-backed versions for a specific artifact (used for disk cleanup on delete).
    pub async fn find_file_versions_by_artifact<'e, E>(
        executor: E,
        artifact_id: i64,
    ) -> Result<Vec<ArtifactVersion>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let query = format!(
            "SELECT {} FROM artifact_version WHERE artifact = $1 AND file_path IS NOT NULL",
            artifact_version::SELECT_COLUMNS
        );
        sqlx::query_as::<_, ArtifactVersion>(&query)
            .bind(artifact_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::ArtifactVersionRepository;

    #[test]
    fn aliased_select_columns_keep_null_content_expression_unqualified() {
        let columns = ArtifactVersionRepository::select_columns_with_alias("av");

        assert!(columns.contains("av.id"));
        assert!(columns.contains("av.file_path"));
        assert!(columns.contains("NULL::bytea AS content"));
        assert!(!columns.contains("av.NULL::bytea AS content"));
    }
}
