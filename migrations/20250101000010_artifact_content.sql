-- Migration: Artifact Content System
-- Description: Enhances the artifact table with content fields (name, description,
--              content_type, size_bytes, structured data, visibility) and creates
--              the artifact_version table for versioned file/data storage.
--
--              The artifact table now serves as the "header" for a logical artifact,
--              while artifact_version rows hold the actual immutable content snapshots.
--              Progress-type artifacts store their live state directly in artifact.data
--              (append-style updates without creating new versions).
--
--              Execution association is recorded per-version on
--              artifact_version.execution (the parent artifact is associated with
--              its action/owner, not a specific execution).
--
-- Version: 20250101000010

-- ============================================================================
-- ENHANCE ARTIFACT TABLE
-- ============================================================================

-- Human-readable name (e.g. "Build Log", "Test Results")
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS name TEXT;

-- Optional longer description
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS description TEXT;

-- MIME content type (e.g. "application/json", "text/plain", "image/png")
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS content_type TEXT;

-- Total size in bytes of the latest version's content (NULL for progress artifacts)
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS size_bytes BIGINT;

-- Structured data for progress-type artifacts and small structured payloads.
-- Progress artifacts append entries here; file artifacts may store parsed metadata.
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS data JSONB;

-- Visibility: public artifacts are viewable by all authenticated users;
-- private artifacts are restricted based on the artifact's scope/owner.
-- The scope (identity, action, pack, etc.) + owner fields define who can access
-- a private artifact. Full RBAC enforcement is deferred — for now the column
-- enables filtering and is available for future permission checks.
ALTER TABLE artifact ADD COLUMN IF NOT EXISTS visibility artifact_visibility_enum NOT NULL DEFAULT 'private';

-- New indexes for the added columns
CREATE INDEX IF NOT EXISTS idx_artifact_name ON artifact(name);
CREATE INDEX IF NOT EXISTS idx_artifact_visibility ON artifact(visibility);
CREATE INDEX IF NOT EXISTS idx_artifact_visibility_scope ON artifact(visibility, scope, owner);

-- Comments for new columns
COMMENT ON COLUMN artifact.name IS 'Human-readable artifact name';
COMMENT ON COLUMN artifact.description IS 'Optional description of the artifact';
COMMENT ON COLUMN artifact.content_type IS 'MIME content type (e.g. application/json, text/plain)';
COMMENT ON COLUMN artifact.size_bytes IS 'Size of latest version content in bytes';
COMMENT ON COLUMN artifact.data IS 'Structured JSONB data for progress artifacts or metadata';
COMMENT ON COLUMN artifact.visibility IS 'Access visibility: public (all users) or private (scope/owner-restricted)';


-- ============================================================================
-- ARTIFACT_VERSION TABLE
-- ============================================================================
-- Each row is an immutable snapshot of artifact content. File-type artifacts get
-- a new version on each upload; progress-type artifacts do NOT use versions
-- (they update artifact.data directly).

CREATE TABLE artifact_version (
    id BIGSERIAL PRIMARY KEY,

    -- Parent artifact
    artifact BIGINT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,

    -- Monotonically increasing version number within the artifact (1-based)
    version INTEGER NOT NULL,

    -- Optional execution that produced this version. Plain BIGINT (no FK)
    -- because `execution` is a TimescaleDB hypertable. This is the canonical
    -- per-version association used for cleanup, "show me the log version
    -- emitted by execution N", and finalize_file_artifacts scans.
    execution BIGINT,

    -- MIME content type for this specific version (may differ from parent)
    content_type TEXT,

    -- Size of the content in bytes
    size_bytes BIGINT,

    -- Binary content (file uploads, DB-stored). NULL for file-backed versions.
    content BYTEA,

    -- Structured content (JSON payloads, parsed results, etc.)
    content_json JSONB,

    -- Relative path from artifacts_dir root for disk-stored content.
    -- When set, content BYTEA is NULL — file lives on shared volume.
    -- Pattern: {ref_slug}/v{version}.{ext}
    -- e.g., "mypack/build_log/v1.txt"
    file_path TEXT,

    -- Free-form metadata about this version (e.g. commit hash, build number)
    meta JSONB,

    -- Who or what created this version (identity ref, action ref, "system", etc.)
    created_by TEXT,

    -- Immutable — no updated column
    created TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique constraint: one version number per artifact
ALTER TABLE artifact_version
    ADD CONSTRAINT uq_artifact_version_artifact_version UNIQUE (artifact, version);

-- Indexes
CREATE INDEX idx_artifact_version_artifact ON artifact_version(artifact);
CREATE INDEX idx_artifact_version_artifact_version ON artifact_version(artifact, version DESC);
CREATE INDEX idx_artifact_version_created ON artifact_version(created DESC);
CREATE INDEX idx_artifact_version_file_path ON artifact_version(file_path) WHERE file_path IS NOT NULL;
CREATE INDEX idx_artifact_version_execution ON artifact_version(execution) WHERE execution IS NOT NULL;
CREATE INDEX idx_artifact_version_artifact_execution ON artifact_version(artifact, execution) WHERE execution IS NOT NULL;

-- Comments
COMMENT ON TABLE artifact_version IS 'Immutable content snapshots for artifacts (file uploads, structured data)';
COMMENT ON COLUMN artifact_version.artifact IS 'Parent artifact this version belongs to';
COMMENT ON COLUMN artifact_version.version IS 'Version number (1-based, monotonically increasing per artifact)';
COMMENT ON COLUMN artifact_version.content_type IS 'MIME content type for this version';
COMMENT ON COLUMN artifact_version.size_bytes IS 'Size of content in bytes';
COMMENT ON COLUMN artifact_version.content IS 'Binary content (file data)';
COMMENT ON COLUMN artifact_version.content_json IS 'Structured JSON content';
COMMENT ON COLUMN artifact_version.meta IS 'Free-form metadata about this version';
COMMENT ON COLUMN artifact_version.created_by IS 'Who created this version (identity ref, action ref, system)';
COMMENT ON COLUMN artifact_version.file_path IS 'Relative path from artifacts_dir root for disk-stored content. When set, content BYTEA is NULL — file lives on shared volume.';


-- ============================================================================
-- HELPER FUNCTION: next_artifact_version
-- ============================================================================
-- Returns the next version number for an artifact (MAX(version) + 1, or 1 if none).

CREATE OR REPLACE FUNCTION next_artifact_version(p_artifact_id BIGINT)
RETURNS INTEGER AS $$
DECLARE
    v_next INTEGER;
BEGIN
    SELECT COALESCE(MAX(version), 0) + 1
    INTO v_next
    FROM artifact_version
    WHERE artifact = p_artifact_id;

    RETURN v_next;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION next_artifact_version IS 'Returns the next version number for the given artifact';


-- ============================================================================
-- RETENTION ENFORCEMENT FUNCTION
-- ============================================================================
-- Called after inserting a new version to enforce the artifact retention policy.
-- For 'versions' policy: deletes oldest versions beyond the limit.
-- Time-based policies (days/hours/minutes) are handled by a scheduled job (not this trigger).

CREATE OR REPLACE FUNCTION enforce_artifact_retention()
RETURNS TRIGGER AS $$
DECLARE
    v_policy artifact_retention_enum;
    v_limit INTEGER;
    v_count INTEGER;
BEGIN
    SELECT retention_policy, retention_limit
    INTO v_policy, v_limit
    FROM artifact
    WHERE id = NEW.artifact;

    IF v_policy = 'versions' AND v_limit > 0 THEN
        -- Count existing versions
        SELECT COUNT(*) INTO v_count
        FROM artifact_version
        WHERE artifact = NEW.artifact;

        -- If over limit, delete the oldest ones
        IF v_count > v_limit THEN
            DELETE FROM artifact_version
            WHERE id IN (
                SELECT id
                FROM artifact_version
                WHERE artifact = NEW.artifact
                ORDER BY version ASC
                LIMIT (v_count - v_limit)
            );
        END IF;
    END IF;

    -- Update parent artifact size_bytes with the new version's size
    UPDATE artifact
    SET size_bytes = NEW.size_bytes,
        content_type = COALESCE(NEW.content_type, content_type)
    WHERE id = NEW.artifact;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_enforce_artifact_retention
    AFTER INSERT ON artifact_version
    FOR EACH ROW
    EXECUTE FUNCTION enforce_artifact_retention();

COMMENT ON FUNCTION enforce_artifact_retention IS 'Enforces version-count retention policy and syncs size to parent artifact';
