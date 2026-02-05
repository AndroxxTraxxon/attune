-- Migration: Unify Runtimes (Remove runtime_type distinction)
-- Description: Removes the runtime_type field and consolidates sensor/action runtimes
--              into a single unified runtime system. Both sensors and actions use the
--              same binaries and verification logic, so the distinction is redundant.
--              Runtime metadata is now loaded from YAML files in packs/core/runtimes/
-- Version: 20260203000001

-- ============================================================================
-- STEP 1: Drop constraints that prevent unified runtime format
-- ============================================================================

-- Drop NOT NULL constraint from runtime_type to allow migration
ALTER TABLE runtime ALTER COLUMN runtime_type DROP NOT NULL;

-- Drop the runtime_ref_format constraint (expects pack.type.name, we want pack.name)
ALTER TABLE runtime DROP CONSTRAINT IF EXISTS runtime_ref_format;

-- Drop the runtime_ref_lowercase constraint (will recreate after migration)
ALTER TABLE runtime DROP CONSTRAINT IF EXISTS runtime_ref_lowercase;

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
-- STEP 3: Clean up old runtime records (data will be reloaded from YAML)
-- ============================================================================

-- Remove all existing runtime records - they will be reloaded from YAML files
TRUNCATE TABLE runtime CASCADE;

-- ============================================================================
-- STEP 4: Update comments and create new indexes
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

-- Runtime records are now loaded from YAML files in packs/core/runtimes/:
-- 1. python.yaml         - Python 3 runtime (unified)
-- 2. nodejs.yaml         - Node.js runtime (unified)
-- 3. shell.yaml          - Shell runtime (unified)
-- 4. native.yaml         - Native runtime (unified)
-- 5. sensor_builtin.yaml - Built-in sensor runtime (sensor-specific timers, etc.)

DO $$
BEGIN
    RAISE NOTICE 'Runtime unification complete. Runtime records will be loaded from YAML files.';
END $$;
