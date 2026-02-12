-- Migration: Pack System
-- Description: Creates pack and runtime tables (runtime without runtime_type)
-- Version: 20250101000002

-- ============================================================================
-- PACK TABLE
-- ============================================================================

CREATE TABLE pack (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    description TEXT,
    version TEXT NOT NULL,
    conf_schema JSONB NOT NULL DEFAULT '{}'::jsonb,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    runtime_deps TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    dependencies TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    is_standard BOOLEAN NOT NULL DEFAULT FALSE,
    installers JSONB DEFAULT '[]'::jsonb,

    -- Installation metadata (nullable for non-installed packs)
    source_type TEXT,
    source_url TEXT,
    source_ref TEXT,
    checksum TEXT,
    checksum_verified BOOLEAN DEFAULT FALSE,
    installed_at TIMESTAMPTZ,
    installed_by BIGINT,
    installation_method TEXT,
    storage_path TEXT,

    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT pack_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT pack_ref_format CHECK (ref ~ '^[a-z][a-z0-9_-]+$'),
    CONSTRAINT pack_version_semver CHECK (
        version ~ '^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
    )
);

-- Indexes
CREATE INDEX idx_pack_ref ON pack(ref);
CREATE INDEX idx_pack_created ON pack(created DESC);
CREATE INDEX idx_pack_is_standard ON pack(is_standard) WHERE is_standard = TRUE;
CREATE INDEX idx_pack_is_standard_created ON pack(is_standard, created DESC);
CREATE INDEX idx_pack_version_created ON pack(version, created DESC);
CREATE INDEX idx_pack_config_gin ON pack USING GIN (config);
CREATE INDEX idx_pack_meta_gin ON pack USING GIN (meta);
CREATE INDEX idx_pack_tags_gin ON pack USING GIN (tags);
CREATE INDEX idx_pack_runtime_deps_gin ON pack USING GIN (runtime_deps);
CREATE INDEX idx_pack_dependencies_gin ON pack USING GIN (dependencies);
CREATE INDEX idx_pack_installed_at ON pack(installed_at DESC) WHERE installed_at IS NOT NULL;
CREATE INDEX idx_pack_installed_by ON pack(installed_by) WHERE installed_by IS NOT NULL;
CREATE INDEX idx_pack_source_type ON pack(source_type) WHERE source_type IS NOT NULL;

-- Trigger
CREATE TRIGGER update_pack_updated
    BEFORE UPDATE ON pack
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE pack IS 'Packs bundle related automation components';
COMMENT ON COLUMN pack.ref IS 'Unique pack reference identifier (e.g., "slack", "github")';
COMMENT ON COLUMN pack.label IS 'Human-readable pack name';
COMMENT ON COLUMN pack.version IS 'Semantic version of the pack';
COMMENT ON COLUMN pack.conf_schema IS 'JSON schema for pack configuration';
COMMENT ON COLUMN pack.config IS 'Pack configuration values';
COMMENT ON COLUMN pack.meta IS 'Pack metadata';
COMMENT ON COLUMN pack.runtime_deps IS 'Array of required runtime references (e.g., shell, python, nodejs)';
COMMENT ON COLUMN pack.dependencies IS 'Array of required pack references (e.g., core, utils)';
COMMENT ON COLUMN pack.is_standard IS 'Whether this is a core/built-in pack';
COMMENT ON COLUMN pack.source_type IS 'Installation source type (e.g., "git", "local", "registry")';
COMMENT ON COLUMN pack.source_url IS 'URL or path where pack was installed from';
COMMENT ON COLUMN pack.source_ref IS 'Git ref, version tag, or other source reference';
COMMENT ON COLUMN pack.checksum IS 'Content checksum for verification';
COMMENT ON COLUMN pack.checksum_verified IS 'Whether checksum has been verified';
COMMENT ON COLUMN pack.installed_at IS 'Timestamp when pack was installed';
COMMENT ON COLUMN pack.installed_by IS 'Identity ID of user who installed the pack';
COMMENT ON COLUMN pack.installation_method IS 'Method used for installation (e.g., "cli", "api", "auto")';
COMMENT ON COLUMN pack.storage_path IS 'Filesystem path where pack files are stored';

-- ============================================================================
-- RUNTIME TABLE
-- ============================================================================

CREATE TABLE runtime (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT,
    description TEXT,
    name TEXT NOT NULL,
    distributions JSONB NOT NULL,
    installation JSONB,
    installers JSONB DEFAULT '[]'::jsonb,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT runtime_ref_lowercase CHECK (ref = LOWER(ref))
);

-- Indexes
CREATE INDEX idx_runtime_ref ON runtime(ref);
CREATE INDEX idx_runtime_pack ON runtime(pack);
CREATE INDEX idx_runtime_created ON runtime(created DESC);
CREATE INDEX idx_runtime_name ON runtime(name);
CREATE INDEX idx_runtime_verification ON runtime USING GIN ((distributions->'verification'));

-- Trigger
CREATE TRIGGER update_runtime_updated
    BEFORE UPDATE ON runtime
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE runtime IS 'Runtime environments for executing actions and sensors (unified)';
COMMENT ON COLUMN runtime.ref IS 'Unique runtime reference (format: pack.name, e.g., core.python)';
COMMENT ON COLUMN runtime.name IS 'Runtime name (e.g., "Python", "Node.js", "Shell")';
COMMENT ON COLUMN runtime.distributions IS 'Runtime distribution metadata including verification commands, version requirements, and capabilities';
COMMENT ON COLUMN runtime.installation IS 'Installation requirements and instructions including package managers and setup steps';
COMMENT ON COLUMN runtime.installers IS 'Array of installer actions to create pack-specific runtime environments. Each installer defines commands to set up isolated environments (e.g., Python venv, npm install).';
