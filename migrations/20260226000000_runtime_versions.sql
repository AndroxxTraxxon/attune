-- Migration: Runtime Versions
-- Description: Adds support for multiple versions of the same runtime (e.g., Python 3.11, 3.12, 3.14).
--   - New `runtime_version` table to store version-specific execution configurations
--   - New `runtime_version_constraint` columns on action and sensor tables
-- Version: 20260226000000

-- ============================================================================
-- RUNTIME VERSION TABLE
-- ============================================================================

CREATE TABLE runtime_version (
    id BIGSERIAL PRIMARY KEY,
    runtime BIGINT NOT NULL REFERENCES runtime(id) ON DELETE CASCADE,
    runtime_ref TEXT NOT NULL,

    -- Semantic version string (e.g., "3.12.1", "20.11.0")
    version TEXT NOT NULL,

    -- Individual version components for efficient range queries.
    -- Nullable because some runtimes may use non-numeric versioning.
    version_major INT,
    version_minor INT,
    version_patch INT,

    -- Complete execution configuration for this specific version.
    -- This is NOT a diff/override — it is a full standalone config that can
    -- replace the parent runtime's execution_config when this version is selected.
    -- Structure is identical to runtime.execution_config (RuntimeExecutionConfig).
    execution_config JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Version-specific distribution/verification metadata.
    -- Structure mirrors runtime.distributions but with version-specific commands.
    -- Example: verification commands that check for a specific binary like python3.12.
    distributions JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Whether this version is the default for the parent runtime.
    -- At most one version per runtime should be marked as default.
    is_default BOOLEAN NOT NULL DEFAULT FALSE,

    -- Whether this version has been verified as available on the current system.
    available BOOLEAN NOT NULL DEFAULT TRUE,

    -- When this version was last verified (via running verification commands).
    verified_at TIMESTAMPTZ,

    -- Arbitrary version-specific metadata (e.g., EOL date, release notes URL,
    -- feature flags, platform-specific notes).
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,

    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT runtime_version_unique UNIQUE(runtime, version)
);

-- Indexes
CREATE INDEX idx_runtime_version_runtime ON runtime_version(runtime);
CREATE INDEX idx_runtime_version_runtime_ref ON runtime_version(runtime_ref);
CREATE INDEX idx_runtime_version_version ON runtime_version(version);
CREATE INDEX idx_runtime_version_available ON runtime_version(available) WHERE available = TRUE;
CREATE INDEX idx_runtime_version_is_default ON runtime_version(is_default) WHERE is_default = TRUE;
CREATE INDEX idx_runtime_version_components ON runtime_version(runtime, version_major, version_minor, version_patch);
CREATE INDEX idx_runtime_version_created ON runtime_version(created DESC);
CREATE INDEX idx_runtime_version_execution_config ON runtime_version USING GIN (execution_config);
CREATE INDEX idx_runtime_version_meta ON runtime_version USING GIN (meta);

-- Trigger
CREATE TRIGGER update_runtime_version_updated
    BEFORE UPDATE ON runtime_version
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE runtime_version IS 'Specific versions of a runtime (e.g., Python 3.11, 3.12) with version-specific execution configuration';
COMMENT ON COLUMN runtime_version.runtime IS 'Parent runtime this version belongs to';
COMMENT ON COLUMN runtime_version.runtime_ref IS 'Parent runtime ref (e.g., core.python) for display/filtering';
COMMENT ON COLUMN runtime_version.version IS 'Semantic version string (e.g., "3.12.1", "20.11.0")';
COMMENT ON COLUMN runtime_version.version_major IS 'Major version component for efficient range queries';
COMMENT ON COLUMN runtime_version.version_minor IS 'Minor version component for efficient range queries';
COMMENT ON COLUMN runtime_version.version_patch IS 'Patch version component for efficient range queries';
COMMENT ON COLUMN runtime_version.execution_config IS 'Complete execution configuration for this version (same structure as runtime.execution_config)';
COMMENT ON COLUMN runtime_version.distributions IS 'Version-specific distribution/verification metadata';
COMMENT ON COLUMN runtime_version.is_default IS 'Whether this is the default version for the parent runtime (at most one per runtime)';
COMMENT ON COLUMN runtime_version.available IS 'Whether this version has been verified as available on the system';
COMMENT ON COLUMN runtime_version.verified_at IS 'Timestamp of last availability verification';
COMMENT ON COLUMN runtime_version.meta IS 'Arbitrary version-specific metadata';

-- ============================================================================
-- ACTION TABLE: ADD RUNTIME VERSION CONSTRAINT
-- ============================================================================

ALTER TABLE action
    ADD COLUMN runtime_version_constraint TEXT;

COMMENT ON COLUMN action.runtime_version_constraint IS 'Semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0"). NULL means any version.';

-- ============================================================================
-- SENSOR TABLE: ADD RUNTIME VERSION CONSTRAINT
-- ============================================================================

ALTER TABLE sensor
    ADD COLUMN runtime_version_constraint TEXT;

COMMENT ON COLUMN sensor.runtime_version_constraint IS 'Semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0"). NULL means any version.';
