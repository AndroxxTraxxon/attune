-- Migration: Add Pack Runtime Environments
-- Description: Adds support for per-pack isolated runtime environments with installer metadata
-- Version: 20260203000002

-- ============================================================================
-- PART 1: Add installer metadata to runtime table
-- ============================================================================

-- Add installers field to runtime table for environment setup instructions
ALTER TABLE runtime ADD COLUMN IF NOT EXISTS installers JSONB DEFAULT '[]'::jsonb;

COMMENT ON COLUMN runtime.installers IS 'Array of installer actions to create pack-specific runtime environments. Each installer defines commands to set up isolated environments (e.g., Python venv, npm install).

Structure:
{
  "installers": [
    {
      "name": "create_environment",
      "description": "Create isolated runtime environment",
      "command": "python3",
      "args": ["-m", "venv", "{env_path}"],
      "cwd": "{pack_path}",
      "env": {},
      "order": 1
    },
    {
      "name": "install_dependencies",
      "description": "Install pack dependencies",
      "command": "{env_path}/bin/pip",
      "args": ["install", "-r", "{pack_path}/requirements.txt"],
      "cwd": "{pack_path}",
      "env": {},
      "order": 2,
      "optional": false
    }
  ]
}

Template variables:
  {env_path}   - Full path to environment directory (e.g., /opt/attune/packenvs/mypack/python)
  {pack_path}  - Full path to pack directory (e.g., /opt/attune/packs/mypack)
  {pack_ref}   - Pack reference (e.g., mycompany.monitoring)
  {runtime_ref} - Runtime reference (e.g., core.python)
  {runtime_name} - Runtime name (e.g., Python)
';

-- ============================================================================
-- PART 2: Create pack_environment table
-- ============================================================================

-- PackEnvironmentStatus enum
DO $$ BEGIN
    CREATE TYPE pack_environment_status_enum AS ENUM (
        'pending',      -- Environment creation scheduled
        'installing',   -- Currently installing
        'ready',        -- Environment ready for use
        'failed',       -- Installation failed
        'outdated'      -- Pack updated, environment needs rebuild
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE pack_environment_status_enum IS 'Status of pack runtime environment installation';

-- Pack environment table
CREATE TABLE IF NOT EXISTS pack_environment (
    id BIGSERIAL PRIMARY KEY,
    pack BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT NOT NULL,
    runtime BIGINT NOT NULL REFERENCES runtime(id) ON DELETE CASCADE,
    runtime_ref TEXT NOT NULL,
    env_path TEXT NOT NULL,
    status pack_environment_status_enum NOT NULL DEFAULT 'pending',
    installed_at TIMESTAMPTZ,
    last_verified TIMESTAMPTZ,
    install_log TEXT,
    install_error TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(pack, runtime)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_pack_environment_pack ON pack_environment(pack);
CREATE INDEX IF NOT EXISTS idx_pack_environment_runtime ON pack_environment(runtime);
CREATE INDEX IF NOT EXISTS idx_pack_environment_status ON pack_environment(status);
CREATE INDEX IF NOT EXISTS idx_pack_environment_pack_ref ON pack_environment(pack_ref);
CREATE INDEX IF NOT EXISTS idx_pack_environment_runtime_ref ON pack_environment(runtime_ref);
CREATE INDEX IF NOT EXISTS idx_pack_environment_pack_runtime ON pack_environment(pack, runtime);

-- Trigger for updated timestamp
CREATE TRIGGER update_pack_environment_updated
    BEFORE UPDATE ON pack_environment
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE pack_environment IS 'Tracks pack-specific runtime environments for dependency isolation';
COMMENT ON COLUMN pack_environment.pack IS 'Pack that owns this environment';
COMMENT ON COLUMN pack_environment.pack_ref IS 'Pack reference for quick lookup';
COMMENT ON COLUMN pack_environment.runtime IS 'Runtime used for this environment';
COMMENT ON COLUMN pack_environment.runtime_ref IS 'Runtime reference for quick lookup';
COMMENT ON COLUMN pack_environment.env_path IS 'Filesystem path to the environment directory (e.g., /opt/attune/packenvs/mypack/python)';
COMMENT ON COLUMN pack_environment.status IS 'Current installation status';
COMMENT ON COLUMN pack_environment.installed_at IS 'When the environment was successfully installed';
COMMENT ON COLUMN pack_environment.last_verified IS 'Last time the environment was verified as working';
COMMENT ON COLUMN pack_environment.install_log IS 'Installation output logs';
COMMENT ON COLUMN pack_environment.install_error IS 'Error message if installation failed';
COMMENT ON COLUMN pack_environment.metadata IS 'Additional metadata (installed packages, versions, etc.)';

-- ============================================================================
-- PART 3: Update existing runtimes with installer metadata
-- ============================================================================

-- Python runtime installers
UPDATE runtime
SET installers = jsonb_build_object(
    'base_path_template', '/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}',
    'installers', jsonb_build_array(
        jsonb_build_object(
            'name', 'create_venv',
            'description', 'Create Python virtual environment',
            'command', 'python3',
            'args', jsonb_build_array('-m', 'venv', '{env_path}'),
            'cwd', '{pack_path}',
            'env', jsonb_build_object(),
            'order', 1,
            'optional', false
        ),
        jsonb_build_object(
            'name', 'upgrade_pip',
            'description', 'Upgrade pip to latest version',
            'command', '{env_path}/bin/pip',
            'args', jsonb_build_array('install', '--upgrade', 'pip'),
            'cwd', '{pack_path}',
            'env', jsonb_build_object(),
            'order', 2,
            'optional', true
        ),
        jsonb_build_object(
            'name', 'install_requirements',
            'description', 'Install pack Python dependencies',
            'command', '{env_path}/bin/pip',
            'args', jsonb_build_array('install', '-r', '{pack_path}/requirements.txt'),
            'cwd', '{pack_path}',
            'env', jsonb_build_object(),
            'order', 3,
            'optional', false,
            'condition', jsonb_build_object(
                'file_exists', '{pack_path}/requirements.txt'
            )
        )
    ),
    'executable_templates', jsonb_build_object(
        'python', '{env_path}/bin/python',
        'pip', '{env_path}/bin/pip'
    )
)
WHERE ref = 'core.python';

-- Node.js runtime installers
UPDATE runtime
SET installers = jsonb_build_object(
    'base_path_template', '/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}',
    'installers', jsonb_build_array(
        jsonb_build_object(
            'name', 'npm_install',
            'description', 'Install Node.js dependencies',
            'command', 'npm',
            'args', jsonb_build_array('install', '--prefix', '{env_path}'),
            'cwd', '{pack_path}',
            'env', jsonb_build_object(
                'NODE_PATH', '{env_path}/node_modules'
            ),
            'order', 1,
            'optional', false,
            'condition', jsonb_build_object(
                'file_exists', '{pack_path}/package.json'
            )
        )
    ),
    'executable_templates', jsonb_build_object(
        'node', 'node',
        'npm', 'npm'
    ),
    'env_vars', jsonb_build_object(
        'NODE_PATH', '{env_path}/node_modules'
    )
)
WHERE ref = 'core.nodejs';

-- Shell runtime (no environment needed, uses system shell)
UPDATE runtime
SET installers = jsonb_build_object(
    'base_path_template', '/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}',
    'installers', jsonb_build_array(),
    'executable_templates', jsonb_build_object(
        'sh', 'sh',
        'bash', 'bash'
    ),
    'requires_environment', false
)
WHERE ref = 'core.shell';

-- Native runtime (no environment needed, binaries are standalone)
UPDATE runtime
SET installers = jsonb_build_object(
    'base_path_template', '/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}',
    'installers', jsonb_build_array(),
    'executable_templates', jsonb_build_object(),
    'requires_environment', false
)
WHERE ref = 'core.native';

-- Built-in sensor runtime (internal, no environment)
UPDATE runtime
SET installers = jsonb_build_object(
    'installers', jsonb_build_array(),
    'requires_environment', false
)
WHERE ref = 'core.sensor.builtin';

-- ============================================================================
-- PART 4: Add helper functions
-- ============================================================================

-- Function to get environment path for a pack/runtime combination
CREATE OR REPLACE FUNCTION get_pack_environment_path(p_pack_ref TEXT, p_runtime_ref TEXT)
RETURNS TEXT AS $$
DECLARE
    v_runtime_name TEXT;
    v_base_template TEXT;
    v_result TEXT;
BEGIN
    -- Get runtime name and base path template
    SELECT
        LOWER(name),
        installers->>'base_path_template'
    INTO v_runtime_name, v_base_template
    FROM runtime
    WHERE ref = p_runtime_ref;

    IF v_base_template IS NULL THEN
        v_base_template := '/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}';
    END IF;

    -- Replace template variables
    v_result := v_base_template;
    v_result := REPLACE(v_result, '{pack_ref}', p_pack_ref);
    v_result := REPLACE(v_result, '{runtime_ref}', p_runtime_ref);
    v_result := REPLACE(v_result, '{runtime_name_lower}', v_runtime_name);

    RETURN v_result;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION get_pack_environment_path IS 'Calculate the filesystem path for a pack runtime environment';

-- Function to check if a runtime requires an environment
CREATE OR REPLACE FUNCTION runtime_requires_environment(p_runtime_ref TEXT)
RETURNS BOOLEAN AS $$
DECLARE
    v_requires BOOLEAN;
BEGIN
    SELECT COALESCE((installers->>'requires_environment')::boolean, true)
    INTO v_requires
    FROM runtime
    WHERE ref = p_runtime_ref;

    RETURN COALESCE(v_requires, false);
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION runtime_requires_environment IS 'Check if a runtime needs a pack-specific environment';

-- ============================================================================
-- PART 5: Create view for environment status
-- ============================================================================

CREATE OR REPLACE VIEW v_pack_environment_status AS
SELECT
    pe.id,
    pe.pack,
    p.ref AS pack_ref,
    p.label AS pack_name,
    pe.runtime,
    r.ref AS runtime_ref,
    r.name AS runtime_name,
    pe.env_path,
    pe.status,
    pe.installed_at,
    pe.last_verified,
    CASE
        WHEN pe.status = 'ready' AND pe.last_verified < NOW() - INTERVAL '7 days' THEN true
        ELSE false
    END AS needs_verification,
    CASE
        WHEN pe.status = 'ready' THEN 'healthy'
        WHEN pe.status = 'failed' THEN 'unhealthy'
        WHEN pe.status IN ('pending', 'installing') THEN 'provisioning'
        WHEN pe.status = 'outdated' THEN 'needs_update'
        ELSE 'unknown'
    END AS health_status,
    pe.install_error,
    pe.created,
    pe.updated
FROM pack_environment pe
JOIN pack p ON pe.pack = p.id
JOIN runtime r ON pe.runtime = r.id;

COMMENT ON VIEW v_pack_environment_status IS 'Consolidated view of pack environment status with health indicators';

-- ============================================================================
-- SUMMARY
-- ============================================================================

-- Display summary of changes
DO $$
BEGIN
    RAISE NOTICE 'Pack environment system migration complete.';
    RAISE NOTICE '';
    RAISE NOTICE 'New table: pack_environment (tracks installed environments)';
    RAISE NOTICE 'New column: runtime.installers (environment setup instructions)';
    RAISE NOTICE 'New functions: get_pack_environment_path, runtime_requires_environment';
    RAISE NOTICE 'New view: v_pack_environment_status';
    RAISE NOTICE '';
    RAISE NOTICE 'Environment paths will be: /opt/attune/packenvs/{pack_ref}/{runtime}';
END $$;
