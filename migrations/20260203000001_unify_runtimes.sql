-- Migration: Unify Runtimes (Remove runtime_type distinction)
-- Description: Removes the runtime_type field and consolidates sensor/action runtimes
--              into a single unified runtime system. Both sensors and actions use the
--              same binaries and verification logic, so the distinction is redundant.
-- Version: 20260203000001

-- ============================================================================
-- STEP 0: Drop constraints that prevent unified runtime format
-- ============================================================================

-- Drop NOT NULL constraint from runtime_type to allow inserting unified runtimes
ALTER TABLE runtime ALTER COLUMN runtime_type DROP NOT NULL;

-- Drop the runtime_ref_format constraint (expects pack.type.name, we want pack.name)
ALTER TABLE runtime DROP CONSTRAINT IF EXISTS runtime_ref_format;

-- Drop the runtime_ref_lowercase constraint (will recreate after migration)
ALTER TABLE runtime DROP CONSTRAINT IF EXISTS runtime_ref_lowercase;

-- ============================================================================
-- STEP 1: Consolidate duplicate runtimes
-- ============================================================================

-- Consolidate Python runtimes (merge action and sensor into unified Python runtime)
DO $$
DECLARE
    v_pack_id BIGINT;
    v_python_runtime_id BIGINT;
BEGIN
    SELECT id INTO v_pack_id FROM pack WHERE ref = 'core';

    -- Insert or update unified Python runtime
    INSERT INTO runtime (ref, pack, pack_ref, description, name, distributions, installation)
    VALUES (
        'core.python',
        v_pack_id,
        'core',
        'Python 3 runtime for actions and sensors with automatic environment management',
        'Python',
        jsonb_build_object(
            'verification', jsonb_build_object(
                'commands', jsonb_build_array(
                    jsonb_build_object(
                        'binary', 'python3',
                        'args', jsonb_build_array('--version'),
                        'exit_code', 0,
                        'pattern', 'Python 3\.',
                        'priority', 1
                    ),
                    jsonb_build_object(
                        'binary', 'python',
                        'args', jsonb_build_array('--version'),
                        'exit_code', 0,
                        'pattern', 'Python 3\.',
                        'priority', 2
                    )
                )
            ),
            'min_version', '3.8',
            'recommended_version', '3.11'
        ),
        jsonb_build_object(
            'package_managers', jsonb_build_array('pip', 'pipenv', 'poetry'),
            'virtual_env_support', true
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        description = EXCLUDED.description,
        distributions = EXCLUDED.distributions,
        installation = EXCLUDED.installation,
        updated = NOW()
    RETURNING id INTO v_python_runtime_id;

    -- Migrate any references from old Python runtimes
    UPDATE action SET runtime = v_python_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.python', 'core.sensor.python')
    );

    UPDATE sensor SET runtime = v_python_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.python', 'core.sensor.python')
    );

    -- Delete old Python runtime entries
    DELETE FROM runtime WHERE ref IN ('core.action.python', 'core.sensor.python');
END $$;

-- Consolidate Node.js runtimes
DO $$
DECLARE
    v_pack_id BIGINT;
    v_nodejs_runtime_id BIGINT;
BEGIN
    SELECT id INTO v_pack_id FROM pack WHERE ref = 'core';

    INSERT INTO runtime (ref, pack, pack_ref, description, name, distributions, installation)
    VALUES (
        'core.nodejs',
        v_pack_id,
        'core',
        'Node.js runtime for JavaScript-based actions and sensors',
        'Node.js',
        jsonb_build_object(
            'verification', jsonb_build_object(
                'commands', jsonb_build_array(
                    jsonb_build_object(
                        'binary', 'node',
                        'args', jsonb_build_array('--version'),
                        'exit_code', 0,
                        'pattern', 'v\d+\.\d+\.\d+',
                        'priority', 1
                    )
                )
            ),
            'min_version', '16.0.0',
            'recommended_version', '20.0.0'
        ),
        jsonb_build_object(
            'package_managers', jsonb_build_array('npm', 'yarn', 'pnpm'),
            'module_support', true
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        description = EXCLUDED.description,
        distributions = EXCLUDED.distributions,
        installation = EXCLUDED.installation,
        updated = NOW()
    RETURNING id INTO v_nodejs_runtime_id;

    -- Migrate references
    UPDATE action SET runtime = v_nodejs_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.nodejs', 'core.sensor.nodejs', 'core.action.node')
    );

    UPDATE sensor SET runtime = v_nodejs_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.nodejs', 'core.sensor.nodejs', 'core.action.node')
    );

    -- Delete old Node.js entries
    DELETE FROM runtime WHERE ref IN ('core.action.nodejs', 'core.sensor.nodejs', 'core.action.node');
END $$;

-- Consolidate Shell runtimes
DO $$
DECLARE
    v_pack_id BIGINT;
    v_shell_runtime_id BIGINT;
BEGIN
    SELECT id INTO v_pack_id FROM pack WHERE ref = 'core';

    INSERT INTO runtime (ref, pack, pack_ref, description, name, distributions, installation)
    VALUES (
        'core.shell',
        v_pack_id,
        'core',
        'Shell (bash/sh) runtime for script execution - always available',
        'Shell',
        jsonb_build_object(
            'verification', jsonb_build_object(
                'commands', jsonb_build_array(
                    jsonb_build_object(
                        'binary', 'sh',
                        'args', jsonb_build_array('--version'),
                        'exit_code', 0,
                        'optional', true,
                        'priority', 1
                    ),
                    jsonb_build_object(
                        'binary', 'bash',
                        'args', jsonb_build_array('--version'),
                        'exit_code', 0,
                        'optional', true,
                        'priority', 2
                    )
                ),
                'always_available', true
            )
        ),
        jsonb_build_object(
            'interpreters', jsonb_build_array('sh', 'bash', 'dash'),
            'portable', true
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        description = EXCLUDED.description,
        distributions = EXCLUDED.distributions,
        installation = EXCLUDED.installation,
        updated = NOW()
    RETURNING id INTO v_shell_runtime_id;

    -- Migrate references
    UPDATE action SET runtime = v_shell_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.shell', 'core.sensor.shell')
    );

    UPDATE sensor SET runtime = v_shell_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.shell', 'core.sensor.shell')
    );

    -- Delete old Shell entries
    DELETE FROM runtime WHERE ref IN ('core.action.shell', 'core.sensor.shell');
END $$;

-- Consolidate Native runtimes
DO $$
DECLARE
    v_pack_id BIGINT;
    v_native_runtime_id BIGINT;
BEGIN
    SELECT id INTO v_pack_id FROM pack WHERE ref = 'core';

    INSERT INTO runtime (ref, pack, pack_ref, description, name, distributions, installation)
    VALUES (
        'core.native',
        v_pack_id,
        'core',
        'Native compiled runtime (Rust, Go, C, etc.) - always available',
        'Native',
        jsonb_build_object(
            'verification', jsonb_build_object(
                'always_available', true,
                'check_required', false
            ),
            'languages', jsonb_build_array('rust', 'go', 'c', 'c++')
        ),
        jsonb_build_object(
            'build_required', false,
            'system_native', true
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        description = EXCLUDED.description,
        distributions = EXCLUDED.distributions,
        installation = EXCLUDED.installation,
        updated = NOW()
    RETURNING id INTO v_native_runtime_id;

    -- Migrate references
    UPDATE action SET runtime = v_native_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.native', 'core.sensor.native')
    );

    UPDATE sensor SET runtime = v_native_runtime_id
    WHERE runtime IN (
        SELECT id FROM runtime WHERE ref IN ('core.action.native', 'core.sensor.native')
    );

    -- Delete old Native entries
    DELETE FROM runtime WHERE ref IN ('core.action.native', 'core.sensor.native');
END $$;

-- Handle builtin sensor runtime (keep as-is, it's truly sensor-specific)
UPDATE runtime
SET distributions = jsonb_build_object(
        'verification', jsonb_build_object(
            'always_available', true,
            'check_required', false
        ),
        'type', 'builtin'
    ),
    installation = jsonb_build_object(
        'method', 'builtin',
        'included_with_service', true
    )
WHERE ref = 'core.sensor.builtin';

-- ============================================================================
-- STEP 2: Drop runtime_type column and related objects
-- ============================================================================

-- Drop indexes that reference runtime_type
DROP INDEX IF EXISTS idx_runtime_type;
DROP INDEX IF EXISTS idx_runtime_pack_type;
DROP INDEX IF EXISTS idx_runtime_type_created;
DROP INDEX IF EXISTS idx_runtime_type_sensor;

-- Drop the runtime_type column
ALTER TABLE runtime DROP COLUMN IF EXISTS runtime_type;

-- Drop the enum type
DROP TYPE IF EXISTS runtime_type_enum;

-- ============================================================================
-- STEP 3: Update comments and create new indexes
-- ============================================================================

COMMENT ON TABLE runtime IS 'Runtime environments for executing actions and sensors (unified)';
COMMENT ON COLUMN runtime.ref IS 'Unique runtime reference (format: pack.name, e.g., core.python)';
COMMENT ON COLUMN runtime.name IS 'Runtime name (e.g., "Python", "Node.js", "Shell")';
COMMENT ON COLUMN runtime.distributions IS 'Runtime distribution metadata including verification commands, version requirements, and capabilities';
COMMENT ON COLUMN runtime.installation IS 'Installation requirements and instructions including package managers and setup steps';

-- Create new indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_runtime_name ON runtime(name);
CREATE INDEX IF NOT EXISTS idx_runtime_verification ON runtime USING gin ((distributions->'verification'));

-- ============================================================================
-- VERIFICATION METADATA STRUCTURE DOCUMENTATION
-- ============================================================================

COMMENT ON COLUMN runtime.distributions IS 'Runtime verification and capability metadata. Structure:
{
  "verification": {
    "commands": [                    // Array of verification commands (in priority order)
      {
        "binary": "python3",          // Binary name to execute
        "args": ["--version"],        // Arguments to pass
        "exit_code": 0,               // Expected exit code
        "pattern": "Python 3\\.",     // Optional regex pattern to match in output
        "priority": 1,                // Lower = higher priority
        "optional": false             // If true, failure is non-fatal
      }
    ],
    "always_available": false,       // If true, skip verification (shell, native)
    "check_required": true           // If false, assume available without checking
  },
  "min_version": "3.8",              // Minimum supported version
  "recommended_version": "3.11"      // Recommended version
}';

-- ============================================================================
-- SUMMARY
-- ============================================================================

-- Final runtime records (expected):
-- 1. core.python      - Python 3 runtime (unified)
-- 2. core.nodejs      - Node.js runtime (unified)
-- 3. core.shell       - Shell runtime (unified)
-- 4. core.native      - Native runtime (unified)
-- 5. core.sensor.builtin - Built-in sensor runtime (sensor-specific timers, etc.)

-- Display final state
DO $$
BEGIN
    RAISE NOTICE 'Runtime unification complete. Current runtimes:';
END $$;

SELECT ref, name,
       CASE
           WHEN distributions->'verification'->>'always_available' = 'true' THEN 'Always Available'
           WHEN jsonb_array_length(distributions->'verification'->'commands') > 0 THEN 'Requires Verification'
           ELSE 'Unknown'
       END as availability_check
FROM runtime
ORDER BY ref;
