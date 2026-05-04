-- Migration: Pack System
-- Description: Creates pack, runtime, and runtime_version tables
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
-- PACK REGISTRY INDEX TABLE
-- ============================================================================

CREATE TABLE pack_registry_index (
    id BIGSERIAL PRIMARY KEY,
    name TEXT,
    url TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    headers JSONB NOT NULL DEFAULT '{}'::jsonb,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT pack_registry_index_url_not_empty CHECK (btrim(url) <> ''),
    CONSTRAINT pack_registry_index_position_non_negative CHECK (position >= 0)
);

-- Indexes
CREATE INDEX idx_pack_registry_index_order
    ON pack_registry_index (enabled, position, id);

-- Trigger
CREATE TRIGGER update_pack_registry_index_updated
    BEFORE UPDATE ON pack_registry_index
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE pack_registry_index IS 'Ordered API-managed pack index configuration';
COMMENT ON COLUMN pack_registry_index.url IS 'Index file URL (https://, http://, or file://)';
COMMENT ON COLUMN pack_registry_index.position IS 'Search order position; lower positions are checked first';
COMMENT ON COLUMN pack_registry_index.headers IS 'Optional HTTP headers for fetching authenticated index files';

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
    aliases TEXT[] NOT NULL DEFAULT '{}'::text[],

    distributions JSONB NOT NULL,
    installation JSONB,
    installers JSONB DEFAULT '[]'::jsonb,

    -- Execution configuration: describes how to execute actions using this runtime,
    -- how to create isolated environments, and how to install dependencies.
    --
    -- Structure:
    -- {
    --   "interpreter": {
    --     "binary": "python3",          -- interpreter binary name or path
    --     "args": [],                   -- additional args before the action file
    --     "file_extension": ".py"       -- file extension this runtime handles
    --   },
    --   "environment": {               -- optional: isolated environment config
    --     "env_type": "virtualenv",     -- "virtualenv", "node_modules", "none"
    --     "dir_name": ".venv",          -- directory name relative to pack dir
    --     "create_command": ["python3", "-m", "venv", "{env_dir}"],
    --     "interpreter_path": "{env_dir}/bin/python3"  -- overrides interpreter.binary
    --   },
    --   "dependencies": {              -- optional: dependency management config
    --     "manifest_file": "requirements.txt",
    --     "install_command": ["{interpreter}", "-m", "pip", "install", "-r", "{manifest_path}"]
    --   }
    -- }
    --
    -- Template variables:
    --   {pack_dir}      - absolute path to the pack directory
    --   {env_dir}       - resolved environment directory (pack_dir/dir_name)
    --   {interpreter}   - resolved interpreter path
    --   {action_file}   - absolute path to the action script file
    --   {manifest_path} - absolute path to the dependency manifest file
    execution_config JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Whether this runtime was auto-registered by an agent
    -- (vs. loaded from a pack's YAML file during pack registration)
    auto_detected BOOLEAN NOT NULL DEFAULT FALSE,

    -- Detection metadata for auto-discovered runtimes.
    -- Stores how the agent discovered this runtime (binary path, version, etc.)
    -- enables re-verification on restart.
    -- Example: { "detected_path": "/usr/bin/ruby", "detected_name": "ruby",
    --            "detected_version": "3.3.0" }
    detection_config JSONB NOT NULL DEFAULT '{}'::jsonb,

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
CREATE INDEX idx_runtime_execution_config ON runtime USING GIN (execution_config);
CREATE INDEX idx_runtime_auto_detected ON runtime(auto_detected);
CREATE INDEX idx_runtime_detection_config ON runtime USING GIN (detection_config);
CREATE INDEX idx_runtime_aliases ON runtime USING GIN (aliases);

-- Trigger
CREATE TRIGGER update_runtime_updated
    BEFORE UPDATE ON runtime
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE runtime IS 'Runtime environments for executing actions and sensors (unified)';
COMMENT ON COLUMN runtime.ref IS 'Unique runtime reference (format: pack.name, e.g., core.python)';
COMMENT ON COLUMN runtime.name IS 'Runtime name (e.g., "Python", "Node.js", "Shell")';
COMMENT ON COLUMN runtime.aliases IS 'Lowercase alias names for this runtime (e.g., ["ruby", "rb"] for the Ruby runtime). Used for alias-aware matching during auto-detection and scheduling.';
COMMENT ON COLUMN runtime.distributions IS 'Runtime distribution metadata including verification commands, version requirements, and capabilities';
COMMENT ON COLUMN runtime.installation IS 'Installation requirements and instructions including package managers and setup steps';
COMMENT ON COLUMN runtime.installers IS 'Array of installer actions to create pack-specific runtime environments. Each installer defines commands to set up isolated environments (e.g., Python venv, npm install).';
COMMENT ON COLUMN runtime.execution_config IS 'Execution configuration: interpreter, environment setup, and dependency management. Drives how the worker executes actions and how pack install sets up environments.';
COMMENT ON COLUMN runtime.auto_detected IS 'Whether this runtime was auto-registered by an agent (true) vs. loaded from a pack YAML (false)';
COMMENT ON COLUMN runtime.detection_config IS 'Detection metadata for auto-discovered runtimes: binaries probed, version regex, detected path/version';

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
