# Artifact Content System Implementation

**Date:** 2026-03-02
**Scope:** Database migration, models, repository, API routes, DTOs

## Summary

Implemented a full artifact content management system that allows actions to create, update, and manage artifact files and progress-style artifacts through the API. This builds on the existing `artifact` table (which previously only stored metadata) by adding content storage, versioning, and progress-append semantics.

## Changes

### Database Migration (`migrations/20250101000010_artifact_content.sql`)

- **Enhanced `artifact` table** with new columns:
  - `name` (TEXT) — human-readable artifact name
  - `description` (TEXT) — optional description
  - `content_type` (TEXT) — MIME type
  - `size_bytes` (BIGINT) — size of latest version content
  - `execution` (BIGINT, no FK) — links artifact to the execution that produced it
  - `data` (JSONB) — structured data for progress-type artifacts and metadata
- **Created `artifact_version` table** for immutable content snapshots:
  - `artifact` (FK to artifact, CASCADE delete)
  - `version` (INTEGER, 1-based, monotonically increasing)
  - `content` (BYTEA) — binary file content
  - `content_json` (JSONB) — structured JSON content
  - `meta` (JSONB) — free-form metadata per version
  - `created_by` (TEXT) — who created this version
  - Unique constraint on `(artifact, version)`
- **Helper function** `next_artifact_version()` — auto-assigns next version number
- **Retention trigger** `enforce_artifact_retention()` — auto-deletes oldest versions when count exceeds the artifact's retention limit; also syncs `size_bytes` and `content_type` back to the parent artifact

### Models (`crates/common/src/models.rs`)

- Enhanced `Artifact` struct with new fields: `name`, `description`, `content_type`, `size_bytes`, `execution`, `data`
- Added `SELECT_COLUMNS` constant for consistent query column lists
- Added `ArtifactVersion` model with `SELECT_COLUMNS` (excludes binary content for performance) and `SELECT_COLUMNS_WITH_CONTENT` (includes BYTEA payload)
- Added `ToSchema` derive to `RetentionPolicyType` enum (was missing, needed for OpenAPI)
- Added re-exports for `Artifact` and `ArtifactVersion` in models module

### Repository (`crates/common/src/repositories/artifact.rs`)

- Updated all `ArtifactRepository` queries to use `SELECT_COLUMNS` constant
- Extended `CreateArtifactInput` and `UpdateArtifactInput` with new fields
- Added `ArtifactSearchFilters` and `ArtifactSearchResult` for paginated search
- Added `search()` method with filters for scope, owner, type, execution, name
- Added `find_by_execution()` for listing artifacts by execution ID
- Added `append_progress()` — atomic JSON array append for progress artifacts
- Added `set_data()` — replace full data payload
- Used macro `push_field!` to DRY up the dynamic UPDATE query builder
- Created `ArtifactVersionRepository` with methods:
  - `find_by_id` / `find_by_id_with_content`
  - `list_by_artifact`
  - `find_latest` / `find_latest_with_content`
  - `find_by_version` / `find_by_version_with_content`
  - `create` (auto-assigns version number via `next_artifact_version()`)
  - `delete` / `delete_all_for_artifact` / `count_by_artifact`

### API DTOs (`crates/api/src/dto/artifact.rs`)

- `CreateArtifactRequest` — with defaults for retention policy (versions) and limit (5)
- `UpdateArtifactRequest` — partial update fields
- `AppendProgressRequest` — single JSON entry to append
- `SetDataRequest` — full data replacement
- `ArtifactResponse` / `ArtifactSummary` — full and list response types
- `CreateVersionJsonRequest` — JSON content for a new version
- `ArtifactVersionResponse` / `ArtifactVersionSummary` — version response types
- `ArtifactQueryParams` — filters with pagination
- Conversion `From` impls for all model → DTO conversions

### API Routes (`crates/api/src/routes/artifacts.rs`)

Endpoints mounted under `/api/v1/`:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/artifacts` | List artifacts with filters and pagination |
| POST | `/artifacts` | Create a new artifact |
| GET | `/artifacts/{id}` | Get artifact by ID |
| PUT | `/artifacts/{id}` | Update artifact metadata |
| DELETE | `/artifacts/{id}` | Delete artifact (cascades to versions) |
| GET | `/artifacts/ref/{ref}` | Get artifact by reference string |
| POST | `/artifacts/{id}/progress` | Append entry to progress artifact |
| PUT | `/artifacts/{id}/data` | Set/replace artifact data |
| GET | `/artifacts/{id}/download` | Download latest version content |
| GET | `/artifacts/{id}/versions` | List all versions |
| POST | `/artifacts/{id}/versions` | Create JSON content version |
| GET | `/artifacts/{id}/versions/latest` | Get latest version metadata |
| POST | `/artifacts/{id}/versions/upload` | Upload binary file (multipart) |
| GET | `/artifacts/{id}/versions/{version}` | Get version metadata |
| DELETE | `/artifacts/{id}/versions/{version}` | Delete a version |
| GET | `/artifacts/{id}/versions/{version}/download` | Download version content |
| GET | `/executions/{execution_id}/artifacts` | List artifacts for execution |

- File upload via multipart/form-data with 50 MB limit
- Content type auto-detection from multipart headers with explicit override
- Download endpoints serve binary with proper Content-Type and Content-Disposition headers
- All endpoints require authentication (`RequireAuth`)

### Wiring

- Added `axum` `multipart` feature to API crate's Cargo.toml
- Registered artifact routes in `routes/mod.rs` and `server.rs`
- Registered DTOs in `dto/mod.rs`
- Registered `ArtifactVersionRepository` in `repositories/mod.rs`

### Test Fixes

- Updated existing `repository_artifact_tests.rs` fixtures to include new fields in `CreateArtifactInput` and `UpdateArtifactInput`

## Design Decisions

1. **Progress vs File artifacts**: Progress artifacts use `artifact.data` (JSONB array, appended atomically in SQL). File artifacts use `artifact_version` rows. This avoids creating a version per progress tick.

2. **Binary in BYTEA**: For simplicity, binary content is stored in PostgreSQL BYTEA. A future enhancement could add external object storage (S3) for large files.

3. **Version auto-numbering**: Uses a SQL function (`next_artifact_version`) for safe concurrent version numbering.

4. **Retention enforcement via trigger**: The `enforce_artifact_retention` trigger runs after each version insert, keeping the version count within the configured limit automatically.

5. **No FK to execution**: Since execution is a TimescaleDB hypertable, `artifact.execution` is a plain BIGINT (consistent with other hypertable references in the project).

6. **SELECT_COLUMNS pattern**: Binary content is excluded from default queries for performance. Separate `*_with_content` methods exist for download endpoints.