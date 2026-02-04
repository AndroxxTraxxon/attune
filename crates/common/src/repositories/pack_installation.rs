//! Pack Installation Repository
//!
//! This module provides database operations for pack installation metadata.

use crate::error::Result;
use crate::models::{CreatePackInstallation, Id, PackInstallation};
use sqlx::PgPool;

/// Repository for pack installation metadata operations
pub struct PackInstallationRepository {
    pool: PgPool,
}

impl PackInstallationRepository {
    /// Create a new PackInstallationRepository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new pack installation record
    pub async fn create(&self, data: CreatePackInstallation) -> Result<PackInstallation> {
        let installation = sqlx::query_as::<_, PackInstallation>(
            r#"
            INSERT INTO pack_installation (
                pack_id, source_type, source_url, source_ref,
                checksum, checksum_verified, installed_by,
                installation_method, storage_path, meta
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(data.pack_id)
        .bind(&data.source_type)
        .bind(&data.source_url)
        .bind(&data.source_ref)
        .bind(&data.checksum)
        .bind(data.checksum_verified)
        .bind(data.installed_by)
        .bind(&data.installation_method)
        .bind(&data.storage_path)
        .bind(data.meta.unwrap_or_else(|| serde_json::json!({})))
        .fetch_one(&self.pool)
        .await?;

        Ok(installation)
    }

    /// Get pack installation by ID
    pub async fn get_by_id(&self, id: Id) -> Result<Option<PackInstallation>> {
        let installation =
            sqlx::query_as::<_, PackInstallation>("SELECT * FROM pack_installation WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(installation)
    }

    /// Get pack installation by pack ID
    pub async fn get_by_pack_id(&self, pack_id: Id) -> Result<Option<PackInstallation>> {
        let installation = sqlx::query_as::<_, PackInstallation>(
            "SELECT * FROM pack_installation WHERE pack_id = $1",
        )
        .bind(pack_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(installation)
    }

    /// List all pack installations
    pub async fn list(&self) -> Result<Vec<PackInstallation>> {
        let installations = sqlx::query_as::<_, PackInstallation>(
            "SELECT * FROM pack_installation ORDER BY installed_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(installations)
    }

    /// List pack installations by source type
    pub async fn list_by_source_type(&self, source_type: &str) -> Result<Vec<PackInstallation>> {
        let installations = sqlx::query_as::<_, PackInstallation>(
            "SELECT * FROM pack_installation WHERE source_type = $1 ORDER BY installed_at DESC",
        )
        .bind(source_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(installations)
    }

    /// Update pack installation checksum
    pub async fn update_checksum(
        &self,
        id: Id,
        checksum: &str,
        verified: bool,
    ) -> Result<PackInstallation> {
        let installation = sqlx::query_as::<_, PackInstallation>(
            r#"
            UPDATE pack_installation
            SET checksum = $2, checksum_verified = $3
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(checksum)
        .bind(verified)
        .fetch_one(&self.pool)
        .await?;

        Ok(installation)
    }

    /// Update pack installation metadata
    pub async fn update_meta(&self, id: Id, meta: serde_json::Value) -> Result<PackInstallation> {
        let installation = sqlx::query_as::<_, PackInstallation>(
            r#"
            UPDATE pack_installation
            SET meta = $2
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(meta)
        .fetch_one(&self.pool)
        .await?;

        Ok(installation)
    }

    /// Delete pack installation by ID
    pub async fn delete(&self, id: Id) -> Result<()> {
        sqlx::query("DELETE FROM pack_installation WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Delete pack installation by pack ID
    pub async fn delete_by_pack_id(&self, pack_id: Id) -> Result<()> {
        sqlx::query("DELETE FROM pack_installation WHERE pack_id = $1")
            .bind(pack_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Check if a pack has installation metadata
    pub async fn exists_for_pack(&self, pack_id: Id) -> Result<bool> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM pack_installation WHERE pack_id = $1")
                .bind(pack_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(count.0 > 0)
    }
}

#[cfg(test)]
mod tests {
    // Note: Integration tests should be added in tests/ directory
    // These would require a test database setup
}
