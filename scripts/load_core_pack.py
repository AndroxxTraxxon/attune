#!/usr/bin/env python3
"""
Core Pack Loader for Attune

This script loads the core pack from the filesystem into the database.
It reads pack.yaml, action definitions, trigger definitions, and sensor definitions
and creates all necessary database entries.

Usage:
    python3 scripts/load_core_pack.py [--database-url URL] [--pack-dir DIR]

Environment Variables:
    DATABASE_URL: PostgreSQL connection string (default: from config or localhost)
    ATTUNE_PACKS_DIR: Base directory for packs (default: ./packs)
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

import psycopg2
import psycopg2.extras
import yaml

# Default configuration
DEFAULT_DATABASE_URL = "postgresql://postgres:postgres@localhost:5432/attune"
DEFAULT_PACKS_DIR = "./packs"
CORE_PACK_REF = "core"


def generate_label(name: str) -> str:
    """Generate a human-readable label from a name.

    Examples:
        'crontimer' -> 'Crontimer'
        'http_request' -> 'Http Request'
        'datetime_timer' -> 'Datetime Timer'
    """
    # Replace underscores with spaces and capitalize each word
    return " ".join(word.capitalize() for word in name.replace("_", " ").split())


class CorePackLoader:
    """Loads the core pack into the database"""

    def __init__(self, database_url: str, packs_dir: Path, schema: str = "public"):
        self.database_url = database_url
        self.packs_dir = packs_dir
        self.core_pack_dir = packs_dir / CORE_PACK_REF
        self.schema = schema
        self.conn = None
        self.pack_id = None

    def connect(self):
        """Connect to the database"""
        print(f"Connecting to database...")
        self.conn = psycopg2.connect(self.database_url)
        self.conn.autocommit = False

        # Set search_path to use the correct schema
        cursor = self.conn.cursor()
        cursor.execute(f"SET search_path TO {self.schema}, public")
        cursor.close()
        self.conn.commit()

        print(f"✓ Connected to database (schema: {self.schema})")

    def close(self):
        """Close database connection"""
        if self.conn:
            self.conn.close()

    def load_yaml(self, file_path: Path) -> Dict[str, Any]:
        """Load and parse YAML file"""
        with open(file_path, "r") as f:
            return yaml.safe_load(f)

    def upsert_pack(self) -> int:
        """Create or update the core pack"""
        print("\n→ Loading pack metadata...")

        pack_yaml_path = self.core_pack_dir / "pack.yaml"
        if not pack_yaml_path.exists():
            raise FileNotFoundError(f"pack.yaml not found at {pack_yaml_path}")

        pack_data = self.load_yaml(pack_yaml_path)

        cursor = self.conn.cursor()

        # Prepare pack data
        ref = pack_data["ref"]
        label = pack_data["label"]
        description = pack_data.get("description", "")
        version = pack_data["version"]
        conf_schema = json.dumps(pack_data.get("conf_schema", {}))
        config = json.dumps(pack_data.get("config", {}))
        meta = json.dumps(pack_data.get("meta", {}))
        tags = pack_data.get("tags", [])
        runtime_deps = pack_data.get("runtime_deps", [])
        is_standard = pack_data.get("system", False)

        # Upsert pack
        cursor.execute(
            """
            INSERT INTO pack (
                ref, label, description, version,
                conf_schema, config, meta, tags, runtime_deps, is_standard
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (ref) DO UPDATE SET
                label = EXCLUDED.label,
                description = EXCLUDED.description,
                version = EXCLUDED.version,
                conf_schema = EXCLUDED.conf_schema,
                config = EXCLUDED.config,
                meta = EXCLUDED.meta,
                tags = EXCLUDED.tags,
                runtime_deps = EXCLUDED.runtime_deps,
                is_standard = EXCLUDED.is_standard,
                updated = NOW()
            RETURNING id
        """,
            (
                ref,
                label,
                description,
                version,
                conf_schema,
                config,
                meta,
                tags,
                runtime_deps,
                is_standard,
            ),
        )

        self.pack_id = cursor.fetchone()[0]
        cursor.close()

        print(f"✓ Pack '{ref}' loaded (ID: {self.pack_id})")
        return self.pack_id

    def upsert_triggers(self) -> Dict[str, int]:
        """Load trigger definitions"""
        print("\n→ Loading triggers...")

        triggers_dir = self.core_pack_dir / "triggers"
        if not triggers_dir.exists():
            print("  No triggers directory found")
            return {}

        trigger_ids = {}
        cursor = self.conn.cursor()

        for yaml_file in sorted(triggers_dir.glob("*.yaml")):
            trigger_data = self.load_yaml(yaml_file)

            ref = f"{CORE_PACK_REF}.{trigger_data['name']}"
            label = trigger_data.get("label") or generate_label(trigger_data["name"])
            description = trigger_data.get("description", "")
            enabled = trigger_data.get("enabled", True)
            param_schema = json.dumps(trigger_data.get("parameters", {}))
            out_schema = json.dumps(trigger_data.get("output", {}))

            cursor.execute(
                """
                INSERT INTO trigger (
                    ref, pack, pack_ref, label, description,
                    enabled, param_schema, out_schema, is_adhoc
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (ref) DO UPDATE SET
                    label = EXCLUDED.label,
                    description = EXCLUDED.description,
                    enabled = EXCLUDED.enabled,
                    param_schema = EXCLUDED.param_schema,
                    out_schema = EXCLUDED.out_schema,
                    updated = NOW()
                RETURNING id
            """,
                (
                    ref,
                    self.pack_id,
                    CORE_PACK_REF,
                    label,
                    description,
                    enabled,
                    param_schema,
                    out_schema,
                    False,  # Pack-installed triggers are not ad-hoc
                ),
            )

            trigger_id = cursor.fetchone()[0]
            trigger_ids[ref] = trigger_id
            print(f"  ✓ Trigger '{ref}' (ID: {trigger_id})")

        cursor.close()
        return trigger_ids

    def upsert_actions(self) -> Dict[str, int]:
        """Load action definitions"""
        print("\n→ Loading actions...")

        actions_dir = self.core_pack_dir / "actions"
        if not actions_dir.exists():
            print("  No actions directory found")
            return {}

        action_ids = {}
        cursor = self.conn.cursor()

        # First, ensure we have a runtime for actions
        runtime_id = self.ensure_shell_runtime(cursor)

        for yaml_file in sorted(actions_dir.glob("*.yaml")):
            action_data = self.load_yaml(yaml_file)

            ref = f"{CORE_PACK_REF}.{action_data['name']}"
            label = action_data.get("label") or generate_label(action_data["name"])
            description = action_data.get("description", "")

            # Determine entrypoint
            entrypoint = action_data.get("entry_point", "")
            if not entrypoint:
                # Try to find corresponding script file
                action_name = action_data["name"]
                for ext in [".sh", ".py"]:
                    script_path = actions_dir / f"{action_name}{ext}"
                    if script_path.exists():
                        entrypoint = str(script_path.relative_to(self.packs_dir))
                        break

            param_schema = json.dumps(action_data.get("parameters", {}))
            out_schema = json.dumps(action_data.get("output", {}))

            # Parameter delivery and format (defaults: stdin + json for security)
            parameter_delivery = action_data.get("parameter_delivery", "stdin").lower()
            parameter_format = action_data.get("parameter_format", "json").lower()

            # Validate parameter delivery method (only stdin and file allowed)
            if parameter_delivery not in ["stdin", "file"]:
                print(
                    f"  ⚠ Invalid parameter_delivery '{parameter_delivery}' for '{ref}', defaulting to 'stdin'"
                )
                parameter_delivery = "stdin"

            # Validate parameter format
            if parameter_format not in ["dotenv", "json", "yaml"]:
                print(
                    f"  ⚠ Invalid parameter_format '{parameter_format}' for '{ref}', defaulting to 'json'"
                )
                parameter_format = "json"

            cursor.execute(
                """
                INSERT INTO action (
                    ref, pack, pack_ref, label, description,
                    entrypoint, runtime, param_schema, out_schema, is_adhoc,
                    parameter_delivery, parameter_format
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (ref) DO UPDATE SET
                    label = EXCLUDED.label,
                    description = EXCLUDED.description,
                    entrypoint = EXCLUDED.entrypoint,
                    param_schema = EXCLUDED.param_schema,
                    out_schema = EXCLUDED.out_schema,
                    parameter_delivery = EXCLUDED.parameter_delivery,
                    parameter_format = EXCLUDED.parameter_format,
                    updated = NOW()
                RETURNING id
            """,
                (
                    ref,
                    self.pack_id,
                    CORE_PACK_REF,
                    label,
                    description,
                    entrypoint,
                    runtime_id,
                    param_schema,
                    out_schema,
                    False,  # Pack-installed actions are not ad-hoc
                    parameter_delivery,
                    parameter_format,
                ),
            )

            action_id = cursor.fetchone()[0]
            action_ids[ref] = action_id
            print(f"  ✓ Action '{ref}' (ID: {action_id})")

        cursor.close()
        return action_ids

    def ensure_shell_runtime(self, cursor) -> int:
        """Ensure shell runtime exists"""
        cursor.execute(
            """
            INSERT INTO runtime (
                ref, pack, pack_ref, name, description, distributions
            )
            VALUES (%s, %s, %s, %s, %s, %s)
            ON CONFLICT (ref) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                updated = NOW()
            RETURNING id
        """,
            (
                "core.action.shell",
                self.pack_id,
                CORE_PACK_REF,
                "Shell",
                "Shell script runtime",
                json.dumps({"shell": {"command": "sh"}}),
            ),
        )
        return cursor.fetchone()[0]

    def upsert_sensors(self, trigger_ids: Dict[str, int]) -> Dict[str, int]:
        """Load sensor definitions"""
        print("\n→ Loading sensors...")

        sensors_dir = self.core_pack_dir / "sensors"
        if not sensors_dir.exists():
            print("  No sensors directory found")
            return {}

        sensor_ids = {}
        cursor = self.conn.cursor()

        # Ensure sensor runtime exists
        sensor_runtime_id = self.ensure_sensor_runtime(cursor)

        for yaml_file in sorted(sensors_dir.glob("*.yaml")):
            sensor_data = self.load_yaml(yaml_file)

            ref = f"{CORE_PACK_REF}.{sensor_data['name']}"
            label = sensor_data.get("label") or generate_label(sensor_data["name"])
            description = sensor_data.get("description", "")
            enabled = sensor_data.get("enabled", True)

            # Get trigger reference (handle both trigger_type and trigger_types)
            trigger_types = sensor_data.get("trigger_types", [])
            if not trigger_types:
                # Fallback to singular trigger_type
                trigger_type = sensor_data.get("trigger_type", "")
                trigger_types = [trigger_type] if trigger_type else []

            # Use the first trigger type (sensors currently support one trigger)
            trigger_ref = None
            trigger_id = None
            if trigger_types:
                # Check if it's already a full ref or just the type name
                first_trigger = trigger_types[0]
                if "." in first_trigger:
                    trigger_ref = first_trigger
                else:
                    trigger_ref = f"{CORE_PACK_REF}.{first_trigger}"
                trigger_id = trigger_ids.get(trigger_ref)

            # Determine entrypoint
            entry_point = sensor_data.get("entry_point", "")
            if not entry_point:
                sensor_name = sensor_data["name"]
                for ext in [".py", ".sh"]:
                    script_path = sensors_dir / f"{sensor_name}{ext}"
                    if script_path.exists():
                        entry_point = str(script_path.relative_to(self.packs_dir))
                        break

            config = json.dumps(sensor_data.get("config", {}))

            cursor.execute(
                """
                INSERT INTO sensor (
                    ref, pack, pack_ref, label, description,
                    entrypoint, runtime, runtime_ref, trigger, trigger_ref,
                    enabled, config
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (ref) DO UPDATE SET
                    label = EXCLUDED.label,
                    description = EXCLUDED.description,
                    entrypoint = EXCLUDED.entrypoint,
                    trigger = EXCLUDED.trigger,
                    trigger_ref = EXCLUDED.trigger_ref,
                    enabled = EXCLUDED.enabled,
                    config = EXCLUDED.config,
                    updated = NOW()
                RETURNING id
            """,
                (
                    ref,
                    self.pack_id,
                    CORE_PACK_REF,
                    label,
                    description,
                    entry_point,
                    sensor_runtime_id,
                    "core.sensor.builtin",
                    trigger_id,
                    trigger_ref,
                    enabled,
                    config,
                ),
            )

            sensor_id = cursor.fetchone()[0]
            sensor_ids[ref] = sensor_id
            print(f"  ✓ Sensor '{ref}' (ID: {sensor_id})")

        cursor.close()
        return sensor_ids

    def ensure_sensor_runtime(self, cursor) -> int:
        """Ensure sensor runtime exists"""
        cursor.execute(
            """
            INSERT INTO runtime (
                ref, pack, pack_ref, name, description, distributions
            )
            VALUES (%s, %s, %s, %s, %s, %s)
            ON CONFLICT (ref) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                updated = NOW()
            RETURNING id
        """,
            (
                "core.sensor.builtin",
                self.pack_id,
                CORE_PACK_REF,
                "Built-in Sensor",
                "Built-in sensor runtime",
                json.dumps([]),
            ),
        )
        return cursor.fetchone()[0]

    def load_pack(self):
        """Main loading process"""
        print("=" * 60)
        print("Core Pack Loader")
        print("=" * 60)

        if not self.core_pack_dir.exists():
            raise FileNotFoundError(
                f"Core pack directory not found: {self.core_pack_dir}"
            )

        try:
            self.connect()

            # Load pack metadata
            self.upsert_pack()

            # Load triggers
            trigger_ids = self.upsert_triggers()

            # Load actions
            action_ids = self.upsert_actions()

            # Load sensors
            sensor_ids = self.upsert_sensors(trigger_ids)

            # Commit all changes
            self.conn.commit()

            print("\n" + "=" * 60)
            print("✓ Core pack loaded successfully!")
            print("=" * 60)
            print(f"  Pack ID: {self.pack_id}")
            print(f"  Triggers: {len(trigger_ids)}")
            print(f"  Actions: {len(action_ids)}")
            print(f"  Sensors: {len(sensor_ids)}")
            print()

        except Exception as e:
            if self.conn:
                self.conn.rollback()
            print(f"\n✗ Error loading core pack: {e}")
            import traceback

            traceback.print_exc()
            sys.exit(1)
        finally:
            self.close()


def main():
    parser = argparse.ArgumentParser(
        description="Load the core pack into the Attune database"
    )
    parser.add_argument(
        "--database-url",
        default=os.getenv("DATABASE_URL", DEFAULT_DATABASE_URL),
        help=f"PostgreSQL connection string (default: {DEFAULT_DATABASE_URL})",
    )
    parser.add_argument(
        "--pack-dir",
        type=Path,
        default=Path(os.getenv("ATTUNE_PACKS_DIR", DEFAULT_PACKS_DIR)),
        help=f"Base directory for packs (default: {DEFAULT_PACKS_DIR})",
    )
    parser.add_argument(
        "--schema",
        default=os.getenv("DB_SCHEMA", "public"),
        help="Database schema to use (default: public)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be done without making changes",
    )

    args = parser.parse_args()

    if args.dry_run:
        print("DRY RUN MODE: No changes will be made")
        print()

    loader = CorePackLoader(args.database_url, args.pack_dir, args.schema)
    loader.load_pack()


if __name__ == "__main__":
    main()
