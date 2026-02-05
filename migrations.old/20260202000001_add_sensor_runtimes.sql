-- Migration: Add Sensor Runtimes
-- Description: Adds common sensor runtimes (Python, Node.js, Shell, Native) with verification metadata
-- Version: 20260202000001

-- ============================================================================
-- SENSOR RUNTIMES
-- ============================================================================

-- Insert Python sensor runtime
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions, installation)
VALUES (
    'core.sensor.python',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Python 3 sensor runtime with automatic environment management',
    'sensor',
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
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Insert Node.js sensor runtime
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions, installation)
VALUES (
    'core.sensor.nodejs',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Node.js sensor runtime for JavaScript-based sensors',
    'sensor',
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
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Insert Shell sensor runtime
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions, installation)
VALUES (
    'core.sensor.shell',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Shell (bash/sh) sensor runtime - always available',
    'sensor',
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
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Insert Native sensor runtime
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions, installation)
VALUES (
    'core.sensor.native',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Native compiled sensor runtime (Rust, Go, C, etc.) - always available',
    'sensor',
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
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Update existing builtin sensor runtime with verification metadata
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
    ),
    updated = NOW()
WHERE ref = 'core.sensor.builtin';

-- Add comments
COMMENT ON COLUMN runtime.distributions IS 'Runtime distribution metadata including verification commands, version requirements, and capabilities';
COMMENT ON COLUMN runtime.installation IS 'Installation requirements and instructions including package managers and setup steps';

-- Create index for efficient runtime verification queries
CREATE INDEX IF NOT EXISTS idx_runtime_type_sensor ON runtime(runtime_type) WHERE runtime_type = 'sensor';

-- Verification metadata structure documentation
/*
VERIFICATION METADATA STRUCTURE:

distributions->verification = {
  "commands": [                    // Array of verification commands to try (in priority order)
    {
      "binary": "python3",          // Binary name to execute
      "args": ["--version"],        // Arguments to pass
      "exit_code": 0,               // Expected exit code (0 = success)
      "pattern": "Python 3\.",      // Optional regex pattern to match in output
      "priority": 1,                // Lower = higher priority (try first)
      "optional": false             // If true, failure doesn't mean runtime unavailable
    }
  ],
  "always_available": false,       // If true, skip verification (shell, native)
  "check_required": true           // If false, assume available without checking
}

USAGE EXAMPLE:

To verify Python runtime availability:
1. Query: SELECT distributions->'verification'->'commands' FROM runtime WHERE ref = 'core.sensor.python'
2. Parse commands array
3. Try each command in priority order
4. If any command succeeds with expected exit_code and matches pattern (if provided), runtime is available
5. If all commands fail, runtime is not available

For always_available runtimes (shell, native):
1. Check distributions->'verification'->'always_available'
2. If true, skip verification and report as available
*/
