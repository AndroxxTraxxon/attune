"""
Fixture Helpers for E2E Tests

Provides helper functions for creating test resources like packs,
triggers, actions, rules, etc.
"""

import time
import uuid
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

from .client import AttuneClient


def unique_ref(prefix: str = "test") -> str:
    """
    Generate unique reference string

    Args:
        prefix: Prefix for reference

    Returns:
        Unique reference string (e.g., "test_abc123")
    """
    timestamp = int(time.time() * 1000)
    random_part = str(uuid.uuid4())[:8]
    return f"{prefix}_{timestamp}_{random_part}"


def timestamp_now() -> str:
    """Get current timestamp in ISO format"""
    return datetime.utcnow().isoformat() + "Z"


def timestamp_future(seconds: int) -> str:
    """
    Get future timestamp in ISO format

    Args:
        seconds: Seconds in the future

    Returns:
        ISO timestamp string
    """
    future = datetime.utcnow() + timedelta(seconds=seconds)
    return future.isoformat() + "Z"


# ============================================================================
# Pack Creation
# ============================================================================


def create_test_pack(
    client: AttuneClient,
    pack_ref: Optional[str] = None,
    pack_dir: str = "tests/fixtures/packs/test_pack",
) -> Dict[str, Any]:
    """
    Create or get test pack

    Args:
        client: AttuneClient instance
        pack_ref: Optional pack reference (generated if not provided)
        pack_dir: Path to pack directory

    Returns:
        Pack data
    """
    # Extract pack_ref from pack_dir if not provided
    if not pack_ref:
        # Default pack ref is "test_pack" for the standard test pack
        pack_ref = "test_pack"

    # Always try to get existing pack first
    existing_pack = client.get_pack_by_ref(pack_ref)
    if existing_pack:
        return existing_pack

    # Register new pack if it doesn't exist
    return client.register_pack(pack_dir, force=True)


def ensure_core_pack(client: AttuneClient) -> Dict[str, Any]:
    """
    Ensure core pack exists, register it if needed

    Args:
        client: AttuneClient instance

    Returns:
        Core pack data
    """
    # Try to get existing core pack
    try:
        core_pack = client.get_pack_by_ref("core")
        if core_pack:
            return core_pack
    except Exception:
        pass

    # Core pack doesn't exist, register it
    try:
        return client.register_pack("packs/core", force=True, skip_tests=True)
    except Exception as e:
        # If registration fails, try without skip_tests
        try:
            return client.register_pack("packs/core", force=True)
        except Exception as inner_e:
            raise Exception(f"Failed to register core pack: {inner_e}") from e


# ============================================================================
# Trigger Creation
# ============================================================================


def create_interval_timer(
    client: AttuneClient,
    interval_seconds: int = 5,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
) -> Dict[str, Any]:
    """
    Create interval timer sensor for timer to actually fire

    Args:
        client: AttuneClient instance
        interval_seconds: Interval in seconds
        name: Sensor name (generated if not provided)
        pack_ref: Pack reference

    Returns:
        Dict with trigger and sensor info
    """
    sensor_name = name or f"interval_{interval_seconds}s_{unique_ref()}"

    # Ensure core pack exists
    ensure_core_pack(client)

    # Get or ensure core.intervaltimer trigger exists
    triggers = client.list_triggers()
    core_trigger = None
    for t in triggers:
        if t.get("ref") == "core.intervaltimer":
            core_trigger = t
            break

    if not core_trigger:
        # Create core.intervaltimer trigger if it doesn't exist
        core_trigger = client.create_trigger(
            ref="core.intervaltimer",
            label="Interval Timer",
            pack_ref="core",
            description="Fires at regular intervals",
        )

    # Create sensor to make timer actually fire events
    sensor_ref = f"{pack_ref}.{sensor_name}_sensor"
    sensor_config = {"unit": "seconds", "interval": interval_seconds}

    sensor = client.create_sensor(
        ref=sensor_ref,
        trigger_id=core_trigger["id"],
        trigger_ref=core_trigger["ref"],
        label=f"{sensor_name} Sensor",
        description=f"Sensor for interval timer (every {interval_seconds}s)",
        entrypoint="internal://timer",
        runtime_ref="core.sensor.python3",
        pack_ref=pack_ref,
        enabled=True,
        config=sensor_config,
    )

    # Restart sensor service to load the new sensor
    restart_sensor_service(wait_seconds=2)

    # Return dict with both trigger and sensor info
    return {
        "id": core_trigger["id"],
        "ref": core_trigger["ref"],
        "label": sensor["label"],
        "trigger": core_trigger,
        "sensor": sensor,
        "sensor_id": sensor["id"],
    }


def create_date_timer(
    client: AttuneClient,
    fire_at: Optional[str] = None,
    seconds_from_now: int = 5,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
) -> Dict[str, Any]:
    """
    Create date timer sensor for timer to actually fire

    Args:
        client: AttuneClient instance
        fire_at: ISO timestamp when to fire (optional)
        seconds_from_now: Seconds from now to fire (used if fire_at not provided)
        name: Sensor name (generated if not provided)
        pack_ref: Pack reference

    Returns:
        Dict with trigger and sensor info
    """
    if not fire_at:
        fire_at = timestamp_future(seconds_from_now)

    sensor_name = name or f"date_{unique_ref()}"

    # Ensure core pack exists
    ensure_core_pack(client)

    # Get or ensure core.datetimetimer trigger exists
    triggers = client.list_triggers()
    core_trigger = None
    for t in triggers:
        if t.get("ref") == "core.datetimetimer":
            core_trigger = t
            break

    if not core_trigger:
        # Create core.datetimetimer trigger if it doesn't exist
        core_trigger = client.create_trigger(
            ref="core.datetimetimer",
            label="Date/Time Timer",
            pack_ref="core",
            description="Fires at a specific date/time",
        )

    # Create sensor to make timer actually fire events
    sensor_ref = f"{pack_ref}.{sensor_name}_sensor"
    sensor_config = {"date": fire_at}

    sensor = client.create_sensor(
        ref=sensor_ref,
        trigger_id=core_trigger["id"],
        trigger_ref=core_trigger["ref"],
        label=f"{sensor_name} Sensor",
        description=f"Sensor for date timer (fires at {fire_at})",
        entrypoint="internal://timer",
        runtime_ref="core.sensor.python3",
        pack_ref=pack_ref,
        enabled=True,
        config=sensor_config,
    )

    # Restart sensor service to load the new sensor
    restart_sensor_service(wait_seconds=2)

    # Return dict with both trigger and sensor info
    return {
        "id": core_trigger["id"],
        "ref": core_trigger["ref"],
        "label": sensor["label"],
        "trigger": core_trigger,
        "sensor": sensor,
        "sensor_id": sensor["id"],
        "fire_at": fire_at,
    }


def create_cron_timer(
    client: AttuneClient,
    cron_expression: str = "*/5 * * * * *",
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
    timezone: str = "UTC",
) -> Dict[str, Any]:
    """
    Create cron timer sensor for timer to actually fire

    Args:
        client: AttuneClient instance
        cron_expression: Cron expression (6-field with seconds)
        name: Sensor name (generated if not provided)
        pack_ref: Pack reference
        timezone: Timezone for cron evaluation

    Returns:
        Dict with trigger and sensor info
    """
    sensor_name = name or f"cron_{unique_ref()}"

    # Ensure core pack exists
    ensure_core_pack(client)

    # Get or ensure core.crontimer trigger exists
    triggers = client.list_triggers()
    core_trigger = None
    for t in triggers:
        if t.get("ref") == "core.crontimer":
            core_trigger = t
            break

    if not core_trigger:
        # Create core.crontimer trigger if it doesn't exist
        core_trigger = client.create_trigger(
            ref="core.crontimer",
            label="Cron Timer",
            pack_ref="core",
            description="Fires based on cron schedule",
        )

    # Create sensor to make timer actually fire events
    sensor_ref = f"{pack_ref}.{sensor_name}_sensor"
    sensor_config = {"cron": cron_expression, "timezone": timezone}

    sensor = client.create_sensor(
        ref=sensor_ref,
        trigger_id=core_trigger["id"],
        trigger_ref=core_trigger["ref"],
        label=f"{sensor_name} Sensor",
        description=f"Sensor for cron timer ({cron_expression})",
        entrypoint="internal://timer",
        runtime_ref="core.sensor.python3",
        pack_ref=pack_ref,
        enabled=True,
        config=sensor_config,
    )

    # Restart sensor service to load the new sensor
    restart_sensor_service(wait_seconds=2)

    # Return dict with both trigger and sensor info
    return {
        "id": core_trigger["id"],
        "ref": core_trigger["ref"],
        "label": sensor["label"],
        "trigger": core_trigger,
        "sensor": sensor,
        "sensor_id": sensor["id"],
        "cron_expression": cron_expression,
        "timezone": timezone,
    }


def create_webhook_trigger(
    client: AttuneClient,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
) -> Dict[str, Any]:
    """
    Create webhook trigger

    Args:
        client: AttuneClient instance
        name: Trigger name (generated if not provided)
        pack_ref: Pack reference

    Returns:
        Created trigger data
    """
    trigger_name = name or f"webhook_{unique_ref()}"

    return client.create_trigger(
        pack_ref=pack_ref,
        name=trigger_name,
        trigger_type="webhook",
        parameters={},
    )


# ============================================================================
# Action Creation
# ============================================================================


def create_simple_action(
    client: AttuneClient,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
    runner_type: str = "python3",
    entrypoint: str = "actions/echo.py",
    param_schema: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """
    Create simple action

    Args:
        client: AttuneClient instance
        name: Action name (generated if not provided)
        pack_ref: Pack reference
        runner_type: Runner type
        entrypoint: Entry point path
        param_schema: JSON Schema for parameters

    Returns:
        Created action data
    """
    action_name = name or f"action_{unique_ref()}"

    if param_schema is None:
        param_schema = {
            "type": "object",
            "properties": {"message": {"type": "string", "default": "Hello, World!"}},
        }

    return client.create_action(
        pack_ref=pack_ref,
        name=action_name,
        runner_type=runner_type,
        entrypoint=entrypoint,
        param_schema=param_schema,
    )


def create_echo_action(
    client: AttuneClient,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
) -> Dict[str, Any]:
    """
    Create echo action (simple action that echoes input)

    Args:
        client: AttuneClient instance
        name: Action name (generated if not provided)
        pack_ref: Pack reference

    Returns:
        Created action data
    """
    return create_simple_action(
        client=client,
        name=name or f"echo_{unique_ref()}",
        pack_ref=pack_ref,
        runner_type="python3",
        entrypoint="actions/echo.py",
        param_schema={
            "type": "object",
            "properties": {
                "message": {"type": "string", "default": "echo"},
                "count": {"type": "integer", "default": 1},
            },
        },
    )


def create_failing_action(
    client: AttuneClient,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
    exit_code: int = 1,
) -> Dict[str, Any]:
    """
    Create action that always fails

    Args:
        client: AttuneClient instance
        name: Action name (generated if not provided)
        pack_ref: Pack reference
        exit_code: Exit code to return

    Returns:
        Created action data
    """
    action_name = name or f"failing_{unique_ref()}"

    return client.create_action(
        pack_ref=pack_ref,
        name=action_name,
        runner_type="python3",
        entrypoint="actions/fail.py",
        param_schema={
            "type": "object",
            "properties": {"exit_code": {"type": "integer", "default": exit_code}},
        },
    )


def create_sleep_action(
    client: AttuneClient,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
    default_duration: int = 5,
) -> Dict[str, Any]:
    """
    Create action that sleeps for specified duration

    Args:
        client: AttuneClient instance
        name: Action name (generated if not provided)
        pack_ref: Pack reference
        default_duration: Default sleep duration in seconds

    Returns:
        Created action data
    """
    action_name = name or f"sleep_{unique_ref()}"

    return client.create_action(
        pack_ref=pack_ref,
        name=action_name,
        runner_type="python3",
        entrypoint="actions/sleep.py",
        param_schema={
            "type": "object",
            "properties": {
                "duration": {"type": "integer", "default": default_duration}
            },
        },
    )


# ============================================================================
# Rule Creation
# ============================================================================


def create_rule(
    client: AttuneClient,
    trigger_id: int,
    action_ref: str,
    name: Optional[str] = None,
    pack_ref: str = "test.test_pack",
    enabled: bool = True,
    criteria: Optional[str] = None,
    action_parameters: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """
    Create rule

    Args:
        client: AttuneClient instance
        trigger_id: Trigger ID to monitor
        action_ref: Action reference to execute
        name: Rule name (generated if not provided)
        pack_ref: Pack reference
        enabled: Whether rule is enabled
        criteria: Optional Jinja2 criteria expression
        action_parameters: Parameters to pass to action

    Returns:
        Created rule data
    """
    rule_name = name or f"rule_{unique_ref()}"

    return client.create_rule(
        name=rule_name,
        pack_ref=pack_ref,
        trigger_id=trigger_id,
        action_ref=action_ref,
        enabled=enabled,
        criteria=criteria,
        action_parameters=action_parameters or {},
    )


def create_timer_automation(
    client: AttuneClient,
    interval_seconds: int = 5,
    pack_ref: str = "test.test_pack",
    action_parameters: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """
    Create complete timer automation (trigger + action + rule)

    Args:
        client: AttuneClient instance
        interval_seconds: Timer interval in seconds
        pack_ref: Pack reference
        action_parameters: Parameters to pass to action

    Returns:
        Dictionary with trigger, action, and rule data
    """
    # Create timer trigger
    trigger = create_interval_timer(
        client=client, interval_seconds=interval_seconds, pack_ref=pack_ref
    )

    # Create echo action
    action = create_echo_action(client=client, pack_ref=pack_ref)

    # Create rule linking them
    rule = create_rule(
        client=client,
        trigger_id=trigger["id"],
        action_ref=action["ref"],
        pack_ref=pack_ref,
        action_parameters=action_parameters,
    )

    return {"trigger": trigger, "action": action, "rule": rule}


def create_webhook_automation(
    client: AttuneClient,
    pack_ref: str = "test.test_pack",
    action_parameters: Optional[Dict[str, Any]] = None,
    criteria: Optional[str] = None,
) -> Dict[str, Any]:
    """
    Create complete webhook automation (trigger + action + rule)

    Args:
        client: AttuneClient instance
        pack_ref: Pack reference
        action_parameters: Parameters to pass to action
        criteria: Optional rule criteria

    Returns:
        Dictionary with trigger, action, and rule data
    """
    # Create webhook trigger
    trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)

    # Create echo action
    action = create_echo_action(client=client, pack_ref=pack_ref)

    # Create rule linking them
    rule = create_rule(
        client=client,
        trigger_id=trigger["id"],
        action_ref=action["ref"],
        pack_ref=pack_ref,
        action_parameters=action_parameters,
        criteria=criteria,
    )

    return {"trigger": trigger, "action": action, "rule": rule}


# ============================================================================
# Service Management
# ============================================================================


def restart_sensor_service(wait_seconds: int = 3) -> bool:
    """
    Restart sensor service to reload sensors

    This is needed after creating new sensors so they are loaded
    and can start generating events.

    Works with E2E services managed by start-e2e-services.sh script.

    Args:
        wait_seconds: Seconds to wait after restart for service to be ready

    Returns:
        True if restart successful, False otherwise
    """
    import os
    import signal
    import subprocess

    try:
        # Check if we're running in docker-compose environment
        if os.path.exists("/.dockerenv") or os.getenv("DOCKER_ENV"):
            # Try to restart via docker-compose
            result = subprocess.run(
                ["docker-compose", "restart", "sensor"],
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode == 0:
                time.sleep(wait_seconds)
                return True

        # For E2E services, use PID file to restart the sensor service
        # Calculate paths relative to tests directory
        helpers_dir = os.path.dirname(os.path.abspath(__file__))  # tests/helpers
        tests_dir = os.path.dirname(helpers_dir)  # tests
        pid_dir = os.path.join(tests_dir, "pids")
        pid_file = os.path.join(pid_dir, "sensor.pid")
        log_dir = os.path.join(tests_dir, "logs")
        log_file = os.path.join(log_dir, "sensor.log")

        if os.path.exists(pid_file):
            # Read PID and stop the service
            with open(pid_file, "r") as f:
                pid_str = f.read().strip()
                if not pid_str:
                    print(f"Warning: PID file {pid_file} is empty")
                    os.remove(pid_file)
                    return False
                pid = int(pid_str)

            # Stop the existing process
            stopped = False
            try:
                # Send SIGTERM for graceful shutdown
                os.kill(pid, signal.SIGTERM)

                # Wait up to 5 seconds for graceful shutdown
                for _ in range(10):
                    try:
                        os.kill(pid, 0)  # Check if process exists
                        time.sleep(0.5)
                    except ProcessLookupError:
                        stopped = True
                        break

                # Force kill if still running
                if not stopped:
                    try:
                        os.kill(pid, signal.SIGKILL)
                        time.sleep(1)
                        stopped = True
                    except ProcessLookupError:
                        stopped = True
            except ProcessLookupError:
                stopped = True  # Process doesn't exist

            # Remove PID file
            if os.path.exists(pid_file):
                os.remove(pid_file)

            if not stopped:
                print(f"Warning: Failed to stop sensor process {pid}")

            # Restart the sensor service
            # Get project root
            project_root = os.path.dirname(tests_dir)  # project root
            binary_path = os.path.join(project_root, "target", "debug", "attune-sensor")
            config_file = os.path.join(project_root, "config.e2e.yaml")

            # Verify binary exists
            if not os.path.exists(binary_path):
                print(f"Error: Sensor binary not found at {binary_path}")
                return False

            env = os.environ.copy()
            env["ATTUNE__ENVIRONMENT"] = "e2e"
            env["ATTUNE_CONFIG"] = config_file

            with open(log_file, "a") as log:
                log.write(f"\n\n=== Sensor service restarted at {time.time()} ===\n\n")
                log.flush()
                process = subprocess.Popen(
                    [binary_path],
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    env=env,
                    start_new_session=True,
                    cwd=project_root,  # Run from project root
                )

            # Write new PID file
            with open(pid_file, "w") as f:
                f.write(str(process.pid))
                f.flush()

            # Wait for service to initialize
            time.sleep(wait_seconds)

            # Verify process is still running
            try:
                os.kill(process.pid, 0)

                # Additional verification: check if log file is being written to
                if os.path.exists(log_file):
                    # Get file size before and after a short wait
                    size_before = os.path.getsize(log_file)
                    time.sleep(1)
                    size_after = os.path.getsize(log_file)

                    if size_after > size_before:
                        print(
                            f"✓ Sensor service restarted successfully (PID: {process.pid})"
                        )
                        return True
                    else:
                        print(
                            f"Warning: Sensor service started but not logging (PID: {process.pid})"
                        )
                        return True  # Still return True as process is running
                else:
                    print(f"✓ Sensor service restarted (PID: {process.pid})")
                    return True
            except ProcessLookupError:
                print(f"✗ Sensor service failed to start (process died immediately)")
                if os.path.exists(pid_file):
                    os.remove(pid_file)
                return False
        else:
            print(
                f"Warning: No PID file found at {pid_file}. Sensor service may not be running."
            )
            time.sleep(wait_seconds)
            return False

    except Exception as e:
        print(f"Warning: Error restarting sensor service: {e}")
        import traceback

        traceback.print_exc()
        time.sleep(wait_seconds)
        return False
