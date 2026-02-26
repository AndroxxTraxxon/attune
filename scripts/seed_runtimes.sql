-- Seed Default Runtimes
-- Description: Inserts default runtime configurations for the core pack
-- This should be run after migrations to populate the runtime table with core runtimes
--
-- Runtimes are unified (no action/sensor distinction). Whether a runtime can
-- execute actions is determined by the presence of an execution_config with an
-- interpreter. The builtin runtime has no execution_config and is used only for
-- internal sensors (timers, webhooks, etc.).
--
-- The execution_config JSONB column drives how the worker executes actions and
-- how pack installation sets up environments. Template variables:
--   {pack_dir}      - absolute path to the pack directory
--   {env_dir}       - resolved environment directory (runtime_envs_dir/pack_ref/runtime_name)
--   {interpreter}   - resolved interpreter path
--   {action_file}   - absolute path to the action script file
--   {manifest_path} - absolute path to the dependency manifest file

SET search_path TO attune, public;

-- ============================================================================
-- UNIFIED RUNTIMES (5 total)
-- ============================================================================

-- Python 3 Runtime
INSERT INTO runtime (
    ref,
    pack_ref,
    name,
    description,
    distributions,
    installation,
    execution_config
) VALUES (
    'core.python',
    'core',
    'Python',
    'Python 3 runtime for actions and sensors with automatic environment management',
    '{
        "verification": {
            "commands": [
                {"binary": "python3", "args": ["--version"], "exit_code": 0, "pattern": "Python 3\\\\.", "priority": 1},
                {"binary": "python", "args": ["--version"], "exit_code": 0, "pattern": "Python 3\\\\.", "priority": 2}
            ]
        },
        "min_version": "3.8",
        "recommended_version": "3.11"
    }'::jsonb,
    '{
        "package_managers": ["pip", "pipenv", "poetry"],
        "virtual_env_support": true
    }'::jsonb,
    '{
        "interpreter": {
            "binary": "python3",
            "args": ["-u"],
            "file_extension": ".py"
        },
        "environment": {
            "env_type": "virtualenv",
            "dir_name": ".venv",
            "create_command": ["python3", "-m", "venv", "{env_dir}"],
            "interpreter_path": "{env_dir}/bin/python3"
        },
        "dependencies": {
            "manifest_file": "requirements.txt",
            "install_command": ["{interpreter}", "-m", "pip", "install", "-r", "{manifest_path}"]
        }
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    execution_config = EXCLUDED.execution_config,
    updated = NOW();

-- Shell Runtime
INSERT INTO runtime (
    ref,
    pack_ref,
    name,
    description,
    distributions,
    installation,
    execution_config
) VALUES (
    'core.shell',
    'core',
    'Shell',
    'Shell (bash/sh) runtime for script execution - always available',
    '{
        "verification": {
            "commands": [
                {"binary": "sh", "args": ["--version"], "exit_code": 0, "optional": true, "priority": 1},
                {"binary": "bash", "args": ["--version"], "exit_code": 0, "optional": true, "priority": 2}
            ],
            "always_available": true
        }
    }'::jsonb,
    '{
        "interpreters": ["sh", "bash", "dash"],
        "portable": true
    }'::jsonb,
    '{
        "interpreter": {
            "binary": "/bin/bash",
            "args": [],
            "file_extension": ".sh"
        }
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    execution_config = EXCLUDED.execution_config,
    updated = NOW();

-- Node.js Runtime
INSERT INTO runtime (
    ref,
    pack_ref,
    name,
    description,
    distributions,
    installation,
    execution_config
) VALUES (
    'core.nodejs',
    'core',
    'Node.js',
    'Node.js runtime for JavaScript-based actions and sensors',
    '{
        "verification": {
            "commands": [
                {"binary": "node", "args": ["--version"], "exit_code": 0, "pattern": "v\\\\d+\\\\.\\\\d+\\\\.\\\\d+", "priority": 1}
            ]
        },
        "min_version": "16.0.0",
        "recommended_version": "20.0.0"
    }'::jsonb,
    '{
        "package_managers": ["npm", "yarn", "pnpm"],
        "module_support": true
    }'::jsonb,
    '{
        "interpreter": {
            "binary": "node",
            "args": [],
            "file_extension": ".js"
        },
        "environment": {
            "env_type": "node_modules",
            "dir_name": "node_modules",
            "create_command": ["sh", "-c", "mkdir -p {env_dir} && cp {manifest_path} {env_dir}/ 2>/dev/null || true"],
            "interpreter_path": null
        },
        "dependencies": {
            "manifest_file": "package.json",
            "install_command": ["npm", "install", "--prefix", "{env_dir}"]
        },
        "env_vars": {
            "NODE_PATH": "{env_dir}/node_modules"
        }
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    execution_config = EXCLUDED.execution_config,
    updated = NOW();

-- Native Runtime (for compiled binaries: Rust, Go, C, etc.)
INSERT INTO runtime (
    ref,
    pack_ref,
    name,
    description,
    distributions,
    installation,
    execution_config
) VALUES (
    'core.native',
    'core',
    'Native',
    'Native compiled runtime (Rust, Go, C, etc.) - always available',
    '{
        "verification": {
            "always_available": true,
            "check_required": false
        },
        "languages": ["rust", "go", "c", "c++"]
    }'::jsonb,
    '{
        "build_required": false,
        "system_native": true
    }'::jsonb,
    '{
        "interpreter": {
            "binary": "/bin/sh",
            "args": ["-c"],
            "file_extension": null
        }
    }'::jsonb
) ON CONFLICT (ref) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    distributions = EXCLUDED.distributions,
    installation = EXCLUDED.installation,
    execution_config = EXCLUDED.execution_config,
    updated = NOW();

-- Builtin Runtime (for internal sensors: timers, webhooks, etc.)
-- NOTE: No execution_config - this runtime cannot execute actions.
-- The worker skips runtimes without execution_config when loading.
INSERT INTO runtime (
    ref,
    pack_ref,
    name,
    description,
    distributions,
    installation
) VALUES (
    'core.builtin',
    'core',
    'Builtin',
    'Built-in sensor runtime for native Attune sensors (timers, webhooks, etc.)',
    '{
        "verification": {
            "always_available": true,
            "check_required": false
        },
        "type": "builtin"
    }'::jsonb,
    '{
        "method": "builtin",
        "included_with_service": true
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
    SELECT COUNT(*) INTO runtime_count FROM runtime WHERE pack_ref = 'core';
    RAISE NOTICE 'Seeded % core runtime(s)', runtime_count;
END $$;

-- Show summary
SELECT
    ref,
    name,
    CASE WHEN execution_config IS NOT NULL AND execution_config != '{}'::jsonb
         THEN 'yes' ELSE 'no' END AS executable
FROM runtime
WHERE pack_ref = 'core'
ORDER BY ref;
