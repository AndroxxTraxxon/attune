-- Seed Default Runtimes
-- Description: Inserts default runtime configurations for actions and sensors
-- This should be run after migrations to populate the runtime table with core runtimes

SET search_path TO attune, public;

-- ============================================================================
-- ACTION RUNTIMES
-- ============================================================================

-- Python 3 Action Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.action.python3',
    'core',
    'Python 3 Action Runtime',
    'Execute actions using Python 3.x interpreter',
    'action',
    '["python3"]'::jsonb,
    '{
        "method": "system",
        "package_manager": "pip",
        "requirements_file": "requirements.txt"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Shell Action Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.action.shell',
    'core',
    'Shell Action Runtime',
    'Execute actions using system shell (bash/sh)',
    'action',
    '["bash", "sh"]'::jsonb,
    '{
        "method": "system",
        "shell": "/bin/bash"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Node.js Action Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.action.nodejs',
    'core',
    'Node.js Action Runtime',
    'Execute actions using Node.js runtime',
    'action',
    '["nodejs", "node"]'::jsonb,
    '{
        "method": "system",
        "package_manager": "npm",
        "requirements_file": "package.json"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Native Action Runtime (for compiled Rust binaries and other native executables)
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.action.native',
    'core',
    'Native Action Runtime',
    'Execute actions as native compiled binaries',
    'action',
    '["native"]'::jsonb,
    '{
        "method": "binary",
        "description": "Native executable - no runtime installation required"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- ============================================================================
-- SENSOR RUNTIMES
-- ============================================================================

-- Python 3 Sensor Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.sensor.python3',
    'core',
    'Python 3 Sensor Runtime',
    'Execute sensors using Python 3.x interpreter',
    'sensor',
    '["python3"]'::jsonb,
    '{
        "method": "system",
        "package_manager": "pip",
        "requirements_file": "requirements.txt"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Shell Sensor Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.sensor.shell',
    'core',
    'Shell Sensor Runtime',
    'Execute sensors using system shell (bash/sh)',
    'sensor',
    '["bash", "sh"]'::jsonb,
    '{
        "method": "system",
        "shell": "/bin/bash"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Node.js Sensor Runtime
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.sensor.nodejs',
    'core',
    'Node.js Sensor Runtime',
    'Execute sensors using Node.js runtime',
    'sensor',
    '["nodejs", "node"]'::jsonb,
    '{
        "method": "system",
        "package_manager": "npm",
        "requirements_file": "package.json"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- Native Sensor Runtime (for compiled Rust binaries and other native executables)
INSERT INTO attune.runtime (
    ref,
    pack_ref,
    name,
    description,
    runtime_type,
    distributions,
    installation
) VALUES (
    'core.sensor.native',
    'core',
    'Native Sensor Runtime',
    'Execute sensors as native compiled binaries',
    'sensor',
    '["native"]'::jsonb,
    '{
        "method": "binary",
        "description": "Native executable - no runtime installation required"
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    updated = NOW();

-- ============================================================================
-- VERIFICATION
-- ============================================================================

-- Display seeded runtimes
DO $$
DECLARE
    runtime_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO runtime_count FROM attune.runtime WHERE pack_ref = 'core';
    RAISE NOTICE 'Seeded % core runtime(s)', runtime_count;
END $$;

-- Show summary
SELECT
    runtime_type,
    COUNT(*) as count,
    ARRAY_AGG(ref ORDER BY ref) as refs
FROM attune.runtime
WHERE pack_ref = 'core'
GROUP BY runtime_type
ORDER BY runtime_type;
