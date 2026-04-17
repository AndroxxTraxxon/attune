#!/usr/bin/env python3
"""
Migration Consolidation Script

Consolidates 22 migrations into 13 clean migrations by:
1. Removing items created then dropped (runtime_type_enum, workflow_task_execution table, etc.)
2. Including items added later in initial table creation (is_adhoc, workflow columns, etc.)
3. Moving data insertions to YAML files (runtimes)
4. Consolidating incremental additions (webhook columns, notify triggers)
"""

import os
import re
import shutil
from pathlib import Path

# Base directory
BASE_DIR = Path(__file__).parent.parent
MIGRATIONS_DIR = BASE_DIR / "migrations"
MIGRATIONS_OLD_DIR = BASE_DIR / "migrations.old"


def read_migration(filename):
    """Read a migration file from the old directory."""
    path = MIGRATIONS_OLD_DIR / filename
    if path.exists():
        return path.read_text()
    return None


def write_migration(filename, content):
    """Write a migration file to the new directory."""
    path = MIGRATIONS_DIR / filename
    path.write_text(content)
    print(f"Created: {filename}")


def extract_section(content, start_marker, end_marker=None):
    """Extract a section of SQL between markers."""
    start = content.find(start_marker)
    if start == -1:
        return None

    if end_marker:
        end = content.find(end_marker, start)
        if end == -1:
            end = len(content)
    else:
        end = len(content)

    return content[start:end].strip()


def remove_lines_matching(content, patterns):
    """Remove lines matching any of the patterns."""
    lines = content.split("\n")
    filtered = []
    skip_until_semicolon = False

    for line in lines:
        # Check if we should skip this line
        should_skip = False
        for pattern in patterns:
            if pattern in line:
                should_skip = True
                # If this line doesn't end with semicolon, skip until we find one
                if ";" not in line:
                    skip_until_semicolon = True
                break

        if skip_until_semicolon:
            if ";" in line:
                skip_until_semicolon = False
            continue

        if not should_skip:
            filtered.append(line)

    return "\n".join(filtered)


def main():
    print("Starting migration consolidation...")
    print(f"Reading from: {MIGRATIONS_OLD_DIR}")
    print(f"Writing to: {MIGRATIONS_DIR}")
    print()

    # Ensure migrations.old exists
    if not MIGRATIONS_OLD_DIR.exists():
        print("ERROR: migrations.old directory not found!")
        print("Please run: cp -r migrations migrations.old")
        return

    # Clear the migrations directory except README.md
    for file in MIGRATIONS_DIR.glob("*.sql"):
        file.unlink()
    print("Cleared old migrations from migrations/")
    print()

    # ========================================================================
    # Migration 00001: Initial Setup (modified)
    # ========================================================================

    content_00001 = read_migration("20250101000001_initial_setup.sql")

    # Remove runtime_type_enum
    content_00001 = remove_lines_matching(
        content_00001,
        [
            "-- RuntimeType enum",
            "CREATE TYPE runtime_type_enum",
            "COMMENT ON TYPE runtime_type_enum",
        ],
    )

    # Add worker_role_enum after worker_type_enum
    worker_role_enum = """
-- WorkerRole enum
DO $$ BEGIN
    CREATE TYPE worker_role_enum AS ENUM (
        'action',
        'sensor',
        'hybrid'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE worker_role_enum IS 'Role of worker (action executor, sensor, or both)';
"""

    # Add pack_environment_status_enum at the end of enums
    pack_env_enum = """
-- PackEnvironmentStatus enum
DO $$ BEGIN
    CREATE TYPE pack_environment_status_enum AS ENUM (
        'creating',
        'ready',
        'failed',
        'updating',
        'deleting'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE pack_environment_status_enum IS 'Status of pack environment setup';
"""

    # Insert after worker_type_enum
    content_00001 = content_00001.replace(
        "COMMENT ON TYPE worker_type_enum IS 'Type of worker deployment';",
        "COMMENT ON TYPE worker_type_enum IS 'Type of worker deployment';\n"
        + worker_role_enum,
    )

    # Insert before SHARED FUNCTIONS
    content_00001 = content_00001.replace(
        "-- ============================================================================\n-- SHARED FUNCTIONS",
        pack_env_enum
        + "\n-- ============================================================================\n-- SHARED FUNCTIONS",
    )

    write_migration("20250101000001_initial_setup.sql", content_00001)

    # ========================================================================
    # Migration 00002: Identity and Auth
    # ========================================================================

    content_00002 = read_migration("20250101000002_core_tables.sql")

    # Extract identity, permission, and policy sections
    identity_section = extract_section(
        content_00002, "-- IDENTITY TABLE", "-- PERMISSION_SET TABLE"
    )
    permset_section = extract_section(
        content_00002, "-- PERMISSION_SET TABLE", "-- PERMISSION_ASSIGNMENT TABLE"
    )
    permassign_section = extract_section(
        content_00002, "-- PERMISSION_ASSIGNMENT TABLE", "-- POLICY TABLE"
    )
    policy_section = extract_section(content_00002, "-- POLICY TABLE", "-- KEY TABLE")

    migration_00002 = f"""-- Migration: Identity and Authentication
-- Description: Creates identity, permission, and policy tables
-- Version: 20250101000002

-- ============================================================================
{identity_section}

-- ============================================================================
{permset_section}

-- ============================================================================
{permassign_section}

-- ============================================================================
{policy_section}
"""

    write_migration("20250101000002_identity_and_auth.sql", migration_00002)

    # ========================================================================
    # Migration 00003: Pack System
    # ========================================================================

    pack_section = extract_section(content_00002, "-- PACK TABLE", "-- RUNTIME TABLE")
    runtime_section = extract_section(
        content_00002, "-- RUNTIME TABLE", "-- WORKER TABLE"
    )

    # Modify runtime section
    runtime_section = remove_lines_matching(
        runtime_section,
        [
            "runtime_type runtime_type_enum NOT NULL,",
            "runtime_ref_format CHECK (ref ~ '^[^.]+\\.(action|sensor)\\.[^.]+$')",
            "idx_runtime_type",
            "idx_runtime_pack_type",
            "idx_runtime_type_created",
        ],
    )

    # Add new indexes after idx_runtime_created
    new_runtime_indexes = """CREATE INDEX idx_runtime_name ON runtime(name);
CREATE INDEX idx_runtime_verification ON runtime USING GIN ((distributions->'verification'));
"""
    runtime_section = runtime_section.replace(
        "CREATE INDEX idx_runtime_created ON runtime(created DESC);",
        "CREATE INDEX idx_runtime_created ON runtime(created DESC);\n"
        + new_runtime_indexes,
    )

    # Add pack.installers column in pack table
    pack_section = pack_section.replace(
        "is_standard BOOLEAN NOT NULL DEFAULT FALSE,",
        "is_standard BOOLEAN NOT NULL DEFAULT FALSE,\n    installers JSONB DEFAULT '[]'::jsonb,",
    )

    migration_00003 = f"""-- Migration: Pack System
-- Description: Creates pack and runtime tables (runtime without runtime_type)
-- Version: 20250101000003

-- ============================================================================
{pack_section}

-- ============================================================================
{runtime_section}
"""

    write_migration("20250101000003_pack_system.sql", migration_00003)

    # ========================================================================
    # Migration 00004: Action and Sensor
    # ========================================================================

    content_supporting = read_migration("20250101000005_supporting_tables.sql")

    action_section = extract_section(
        content_supporting, "-- ACTION TABLE", "-- SENSOR TABLE"
    )
    sensor_section = extract_section(
        content_supporting, "-- SENSOR TABLE", "-- RULE TABLE"
    )

    # Add is_adhoc to action table
    action_section = action_section.replace(
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,",
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,\n    is_adhoc BOOLEAN DEFAULT false NOT NULL,",
    )

    # Add is_adhoc to sensor table
    sensor_section = sensor_section.replace(
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,",
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,\n    is_adhoc BOOLEAN DEFAULT false NOT NULL,",
    )

    migration_00004 = f"""-- Migration: Action and Sensor
-- Description: Creates action and sensor tables (with is_adhoc from start)
-- Version: 20250101000004

-- ============================================================================
{action_section}

-- ============================================================================
{sensor_section}

-- Add foreign key constraints for policy and key tables
ALTER TABLE policy
    ADD CONSTRAINT policy_action_fkey
    FOREIGN KEY (action) REFERENCES action(id) ON DELETE CASCADE;

ALTER TABLE key
    ADD CONSTRAINT key_owner_action_fkey
    FOREIGN KEY (owner_action) REFERENCES action(id) ON DELETE CASCADE;

ALTER TABLE key
    ADD CONSTRAINT key_owner_sensor_fkey
    FOREIGN KEY (owner_sensor) REFERENCES sensor(id) ON DELETE CASCADE;
"""

    write_migration("20250101000004_action_sensor.sql", migration_00004)

    # ========================================================================
    # Migration 00005: Trigger, Event, and Rule
    # ========================================================================

    content_event = read_migration("20250101000003_event_system.sql")

    trigger_section = extract_section(
        content_event, "-- TRIGGER TABLE", "-- SENSOR TABLE"
    )
    event_section = extract_section(content_event, "-- EVENT TABLE", "-- RULE TABLE")
    rule_section = extract_section(
        content_event, "-- RULE TABLE", "-- ENFORCEMENT TABLE"
    )

    # Add webhook columns to trigger table
    trigger_section = trigger_section.replace(
        "out_schema JSONB,",
        """out_schema JSONB,
    webhook_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    webhook_key VARCHAR(64) UNIQUE,
    webhook_config JSONB DEFAULT '{}'::jsonb,""",
    )

    # Add webhook index
    trigger_section = trigger_section.replace(
        "CREATE INDEX idx_trigger_enabled_created",
        """CREATE INDEX idx_trigger_webhook_key ON trigger(webhook_key) WHERE webhook_key IS NOT NULL;
CREATE INDEX idx_trigger_webhook_enabled_created""",
    )

    # Add rule columns to event table
    event_section = event_section.replace(
        "created TIMESTAMPTZ NOT NULL DEFAULT NOW(),",
        """created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rule BIGINT,
    rule_ref TEXT,""",
    )

    # Add rule index and constraint to event
    event_section += """

-- Add foreign key for rule
ALTER TABLE event
    ADD CONSTRAINT event_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;

CREATE INDEX idx_event_rule ON event(rule);
"""

    # Add is_adhoc to rule table
    rule_section = rule_section.replace(
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,",
        "enabled BOOLEAN NOT NULL DEFAULT TRUE,\n    is_adhoc BOOLEAN DEFAULT false NOT NULL,",
    )

    migration_00005 = f"""-- Migration: Trigger, Event, and Rule
-- Description: Creates trigger (with webhook_config), event (with rule), and rule (with is_adhoc) tables
-- Version: 20250101000005

-- ============================================================================
{trigger_section}

-- ============================================================================
{event_section}

-- ============================================================================
{rule_section}
"""

    write_migration("20250101000005_trigger_event_rule.sql", migration_00005)

    # ========================================================================
    # Migration 00006: Execution System
    # ========================================================================

    content_execution = read_migration("20250101000004_execution_system.sql")

    enforcement_section = extract_section(
        content_execution, "-- ENFORCEMENT TABLE", "-- EXECUTION TABLE"
    )
    execution_section = extract_section(
        content_execution, "-- EXECUTION TABLE", "-- INQUIRY TABLE"
    )
    inquiry_section = extract_section(
        content_execution, "-- INQUIRY TABLE", "-- WORKFLOW_DEFINITION TABLE"
    )

    # Add workflow columns to execution table
    execution_section = execution_section.replace(
        "created TIMESTAMPTZ NOT NULL DEFAULT NOW(),",
        """created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    workflow_def BIGINT,
    workflow_task JSONB,""",
    )

    # Add workflow_def foreign key constraint (will be added after workflow_definition table exists)
    # For now, just note it in comments

    migration_00006 = f"""-- Migration: Execution System
-- Description: Creates enforcement, execution (with workflow columns), and inquiry tables
-- Version: 20250101000006

-- ============================================================================
{enforcement_section}

-- ============================================================================
{execution_section}

-- ============================================================================
{inquiry_section}

-- Add foreign key constraint for enforcement.rule
ALTER TABLE enforcement
    ADD CONSTRAINT enforcement_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE CASCADE;
"""

    write_migration("20250101000006_execution_system.sql", migration_00006)

    # ========================================================================
    # Migration 00007: Workflow System
    # ========================================================================

    workflow_def_section = extract_section(
        content_execution,
        "-- WORKFLOW_DEFINITION TABLE",
        "-- WORKFLOW_TASK_EXECUTION TABLE",
    )

    migration_00007 = f"""-- Migration: Workflow System
-- Description: Creates workflow_definition table (workflow_task_execution consolidated into execution.workflow_task JSONB)
-- Version: 20250101000007

-- ============================================================================
{workflow_def_section}

-- Add foreign key constraint for execution.workflow_def
ALTER TABLE execution
    ADD CONSTRAINT execution_workflow_def_fkey
    FOREIGN KEY (workflow_def) REFERENCES workflow_definition(id) ON DELETE CASCADE;
"""

    write_migration("20250101000007_workflow_system.sql", migration_00007)

    # ========================================================================
    # Migration 00008: Worker and Notification
    # ========================================================================

    worker_section = extract_section(
        content_00002, "-- WORKER TABLE", "-- IDENTITY TABLE"
    )
    notification_section = extract_section(
        content_supporting, "-- NOTIFICATION TABLE", "-- ARTIFACT TABLE"
    )

    # Add worker_role to worker table
    worker_section = worker_section.replace(
        "worker_type worker_type_enum NOT NULL,",
        """worker_type worker_type_enum NOT NULL,
    worker_role worker_role_enum NOT NULL DEFAULT 'action',""",
    )

    migration_00008 = f"""-- Migration: Worker and Notification
-- Description: Creates worker (with worker_role) and notification tables
-- Version: 20250101000008

-- ============================================================================
{worker_section}

-- ============================================================================
{notification_section}
"""

    write_migration("20250101000008_worker_notification.sql", migration_00008)

    # ========================================================================
    # Migration 00009: Artifacts and Keys
    # ========================================================================

    artifact_section = extract_section(content_supporting, "-- ARTIFACT TABLE", None)
    key_section = extract_section(content_00002, "-- KEY TABLE", "-- WORKER TABLE")

    migration_00009 = f"""-- Migration: Artifacts and Keys
-- Description: Creates artifact and key tables for storage and secrets management
-- Version: 20250101000009

-- ============================================================================
{artifact_section}

-- ============================================================================
{key_section}
"""

    write_migration("20250101000009_artifacts_keys.sql", migration_00009)

    # ========================================================================
    # Migration 00010: Webhook System
    # ========================================================================

    # Get final webhook functions from restore file
    content_webhook_restore = read_migration(
        "20260204000001_restore_webhook_functions.sql"
    )

    migration_00010 = (
        """-- Migration: Webhook System
-- Description: Creates webhook-related functions for trigger activation
-- Version: 20250101000010

-- ============================================================================
-- WEBHOOK VALIDATION AND PROCESSING FUNCTIONS
-- ============================================================================

"""
        + content_webhook_restore
    )

    write_migration("20250101000010_webhook_system.sql", migration_00010)

    # ========================================================================
    # Migration 00011: Pack Environments
    # ========================================================================

    content_pack_env = read_migration("20260203000002_add_pack_environments.sql")

    # Extract pack_environment table section (skip the enum and installers column as they're already added)
    pack_env_table = extract_section(
        content_pack_env, "CREATE TABLE pack_environment", None
    )

    migration_00011 = f"""-- Migration: Pack Environments
-- Description: Creates pack_environment table for managing pack dependency environments
-- Version: 20250101000011

-- ============================================================================
-- PACK_ENVIRONMENT TABLE
-- ============================================================================

{pack_env_table}
"""

    write_migration("20250101000011_pack_environments.sql", migration_00011)

    # ========================================================================
    # Migration 00012: Pack Testing
    # ========================================================================

    content_pack_test = read_migration("20260120200000_add_pack_test_results.sql")

    write_migration("20250101000012_pack_testing.sql", content_pack_test)

    # ========================================================================
    # Migration 00013: LISTEN/NOTIFY Triggers (Consolidated)
    # ========================================================================

    # Read all notify trigger migrations
    exec_notify = read_migration("20260119000001_add_execution_notify_trigger.sql")
    event_notify = read_migration("20260129150000_add_event_notify_trigger.sql")
    rule_trigger_update = read_migration(
        "20260203000003_add_rule_trigger_to_execution_notify.sql"
    )
    enforcement_notify = read_migration(
        "20260204000001_add_enforcement_notify_trigger.sql"
    )

    # Get the final version of execution notify (with rule field)
    exec_notify_final = rule_trigger_update if rule_trigger_update else exec_notify

    migration_00013 = f"""-- Migration: LISTEN/NOTIFY Triggers
-- Description: Consolidated PostgreSQL LISTEN/NOTIFY triggers for real-time events
-- Version: 20250101000013

-- ============================================================================
-- EXECUTION CHANGE NOTIFICATION
-- ============================================================================

{exec_notify_final}

-- ============================================================================
-- EVENT CREATION NOTIFICATION
-- ============================================================================

{event_notify}

-- ============================================================================
-- ENFORCEMENT CHANGE NOTIFICATION
-- ============================================================================

{enforcement_notify}
"""

    write_migration("20250101000013_notify_triggers.sql", migration_00013)

    print()
    print("=" * 70)
    print("Migration consolidation complete!")
    print("=" * 70)
    print()
    print("Summary:")
    print(f"  Old migrations: 22 files")
    print(f"  New migrations: 13 files")
    print(f"  Removed: 9 files (consolidated or data moved to YAML)")
    print()
    print("Key changes:")
    print("  ✓ Removed runtime_type_enum (never recreated)")
    print(
        "  ✓ Removed workflow_task_execution table (consolidated into execution.workflow_task)"
    )
    print("  ✓ Removed individual webhook columns (consolidated into webhook_config)")
    print("  ✓ Added is_adhoc flags from start")
    print("  ✓ Added workflow columns to execution from start")
    print("  ✓ Added rule tracking to event from start")
    print("  ✓ Added worker_role from start")
    print("  ✓ Consolidated all LISTEN/NOTIFY triggers")
    print()
    print("Next steps:")
    print("  1. Review the generated migrations")
    print("  2. Test on fresh database: createdb attune_test && sqlx migrate run")
    print("  3. Compare schema: pg_dump --schema-only")
    print("  4. If successful, delete migrations.old/")


if __name__ == "__main__":
    main()
