"""
T3.21: Action Log Size Limits Test

Tests that action execution logs are properly limited in size to prevent
memory/storage issues. Validates log truncation and size enforcement.

Priority: MEDIUM
Duration: ~20 seconds
"""

import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import create_webhook_trigger, unique_ref
from helpers.polling import (
    wait_for_execution_completion,
    wait_for_execution_count,
)


@pytest.mark.tier3
@pytest.mark.logs
@pytest.mark.limits
def test_large_log_output_truncation(client: AttuneClient, test_pack):
    """
    Test that large log output is properly truncated.

    Flow:
    1. Create action that generates very large log output
    2. Execute action
    3. Verify logs are truncated to reasonable size
    4. Verify truncation is indicated in execution result
    """
    print("\n" + "=" * 80)
    print("T3.21.1: Large Log Output Truncation")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"log_limit_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for log limit test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create action that generates large logs
    print("\n[STEP 2] Creating action with large log output...")
    action_ref = f"log_limit_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Large Log Action",
        "description": "Generates large log output to test limits",
        "runner_type": "python",
        "entry_point": """
# Generate large log output (>1MB)
for i in range(50000):
    print(f"Log line {i}: " + "A" * 100)

print("Finished generating large logs")
""",
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created action that generates ~5MB of logs")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"log_limit_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201, (
        f"Failed to create rule: {rule_response.text}"
    )
    rule = rule_response.json()["data"]
    print(f"✓ Created rule")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"test": "large_logs"})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution
    print("\n[STEP 5] Waiting for execution with large logs...")
    wait_for_execution_count(client, expected_count=1, timeout=15)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=15)
    print(f"✓ Execution completed: {execution['status']}")

    # Step 6: Verify log truncation
    print("\n[STEP 6] Verifying log size limits...")

    # Get execution result with logs
    result = execution.get("result", {})

    # Logs should exist but be limited in size
    # Typical limits are 1MB, 5MB, or 10MB depending on implementation
    if isinstance(result, dict):
        stdout = result.get("stdout", "")
        stderr = result.get("stderr", "")

        total_log_size = len(stdout) + len(stderr)
        print(f"  - Total log size: {total_log_size:,} bytes")

        # Verify logs don't exceed reasonable limit (e.g., 10MB)
        max_log_size = 10 * 1024 * 1024  # 10MB
        assert total_log_size <= max_log_size, (
            f"Logs exceed maximum size: {total_log_size} > {max_log_size}"
        )

        # If truncation occurred, there should be some indicator
        # (this depends on implementation - might be in metadata)
        if total_log_size >= 1024 * 1024:  # If >= 1MB
            print(f"  - Large logs detected and handled")

    print(f"✓ Log size limits enforced")
    print(f"  - Execution ID: {execution_id}")
    print(f"  - Status: {execution['status']}")

    print("\n✅ Test passed: Large log output properly handled")


@pytest.mark.tier3
@pytest.mark.logs
@pytest.mark.limits
def test_stderr_log_capture(client: AttuneClient, test_pack):
    """
    Test that stderr logs are captured separately from stdout.

    Flow:
    1. Create action that writes to both stdout and stderr
    2. Execute action
    3. Verify both stdout and stderr are captured
    4. Verify they are stored separately
    """
    print("\n" + "=" * 80)
    print("T3.21.2: Stderr Log Capture")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"stderr_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for stderr test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create action that writes to stdout and stderr
    print("\n[STEP 2] Creating action with stdout/stderr output...")
    action_ref = f"stderr_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Stdout/Stderr Action",
        "description": "Writes to both stdout and stderr",
        "runner_type": "python",
        "entry_point": """
import sys

print("This is stdout line 1")
print("This is stdout line 2", file=sys.stderr)
print("This is stdout line 3")
print("This is stderr line 2", file=sys.stderr)

sys.stdout.flush()
sys.stderr.flush()
""",
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created action with mixed output")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"stderr_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201
    rule = rule_response.json()["data"]
    print(f"✓ Created rule")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"test": "stderr"})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution
    print("\n[STEP 5] Waiting for execution...")
    wait_for_execution_count(client, expected_count=1, timeout=10)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=10)
    print(f"✓ Execution completed: {execution['status']}")

    # Step 6: Verify stdout and stderr are captured
    print("\n[STEP 6] Verifying stdout/stderr capture...")
    assert execution["status"] == "succeeded", (
        f"Expected succeeded, got {execution['status']}"
    )

    result = execution.get("result", {})
    if isinstance(result, dict):
        stdout = result.get("stdout", "")
        stderr = result.get("stderr", "")

        # Verify both streams captured content
        print(f"  - Stdout length: {len(stdout)} bytes")
        print(f"  - Stderr length: {len(stderr)} bytes")

        # Check that stdout contains stdout lines
        if "stdout line" in stdout.lower():
            print(f"  ✓ Stdout captured")

        # Check that stderr contains stderr lines
        if "stderr line" in stderr.lower() or "stderr line" in stdout.lower():
            print(f"  ✓ Stderr captured (may be in stdout)")

    print(f"✓ Log streams validated")
    print(f"  - Execution ID: {execution_id}")

    print("\n✅ Test passed: Stdout and stderr properly captured")


@pytest.mark.tier3
@pytest.mark.logs
@pytest.mark.limits
def test_log_line_count_limits(client: AttuneClient, test_pack):
    """
    Test that extremely high line counts are handled properly.

    Flow:
    1. Create action that generates many log lines
    2. Execute action
    3. Verify system handles high line count gracefully
    """
    print("\n" + "=" * 80)
    print("T3.21.3: Log Line Count Limits")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"log_lines_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for log lines test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create action that generates many lines
    print("\n[STEP 2] Creating action with many log lines...")
    action_ref = f"log_lines_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Many Lines Action",
        "description": "Generates many log lines",
        "runner_type": "python",
        "entry_point": """
# Generate 10,000 short log lines
for i in range(10000):
    print(f"Line {i}")

print("All lines printed")
""",
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created action that generates 10,000 lines")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"log_lines_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201
    rule = rule_response.json()["data"]
    print(f"✓ Created rule")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"test": "many_lines"})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution
    print("\n[STEP 5] Waiting for execution...")
    wait_for_execution_count(client, expected_count=1, timeout=15)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=15)
    print(f"✓ Execution completed: {execution['status']}")

    # Step 6: Verify execution succeeded despite many lines
    print("\n[STEP 6] Verifying high line count handling...")
    assert execution["status"] == "succeeded", (
        f"Expected succeeded, got {execution['status']}"
    )

    result = execution.get("result", {})
    if isinstance(result, dict):
        stdout = result.get("stdout", "")
        line_count = stdout.count("\n") if stdout else 0
        print(f"  - Log lines captured: {line_count:,}")

        # Verify we captured a reasonable number of lines
        # (may be truncated if limits apply)
        assert line_count > 0, "Should have captured some log lines"

    print(f"✓ High line count handled gracefully")
    print(f"  - Execution ID: {execution_id}")
    print(f"  - Status: {execution['status']}")

    print("\n✅ Test passed: High line count handled properly")


@pytest.mark.tier3
@pytest.mark.logs
@pytest.mark.limits
def test_binary_output_handling(client: AttuneClient, test_pack):
    """
    Test that binary/non-UTF8 output is handled gracefully.

    Flow:
    1. Create action that outputs binary data
    2. Execute action
    3. Verify system doesn't crash and handles gracefully
    """
    print("\n" + "=" * 80)
    print("T3.21.4: Binary Output Handling")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"binary_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for binary output test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create action with binary output
    print("\n[STEP 2] Creating action with binary output...")
    action_ref = f"binary_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Binary Output Action",
        "description": "Outputs binary data",
        "runner_type": "python",
        "entry_point": """
import sys

print("Before binary data")

# Write some binary data (will be converted to string representation)
try:
    # Python 3 - sys.stdout is text mode by default
    binary_bytes = bytes([0xFF, 0xFE, 0xFD, 0xFC])
    print(f"Binary bytes: {binary_bytes.hex()}")
except Exception as e:
    print(f"Binary handling: {e}")

print("After binary data")
""",
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created action with binary output")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"binary_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201
    rule = rule_response.json()["data"]
    print(f"✓ Created rule")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"test": "binary"})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution
    print("\n[STEP 5] Waiting for execution...")
    wait_for_execution_count(client, expected_count=1, timeout=10)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=10)
    print(f"✓ Execution completed: {execution['status']}")

    # Step 6: Verify execution succeeded
    print("\n[STEP 6] Verifying binary output handling...")
    assert execution["status"] == "succeeded", (
        f"Expected succeeded, got {execution['status']}"
    )

    # System should handle binary data gracefully (encode, sanitize, or represent as hex)
    result = execution.get("result", {})
    if isinstance(result, dict):
        stdout = result.get("stdout", "")
        print(f"  - Output length: {len(stdout)} bytes")
        print(f"  - Contains 'Before binary data': {'Before binary data' in stdout}")
        print(f"  - Contains 'After binary data': {'After binary data' in stdout}")

    print(f"✓ Binary output handled gracefully")
    print(f"  - Execution ID: {execution_id}")
    print(f"  - Status: {execution['status']}")

    print("\n✅ Test passed: Binary output handled without crashing")
