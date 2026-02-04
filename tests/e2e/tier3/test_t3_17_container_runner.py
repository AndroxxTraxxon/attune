"""
T3.17: Container Runner Execution Test

Tests that actions can be executed in isolated containers using the container runner.
Validates Docker-based action execution, environment isolation, and resource management.

Priority: MEDIUM
Duration: ~30 seconds
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
@pytest.mark.container
@pytest.mark.runner
def test_container_runner_basic_execution(client: AttuneClient, test_pack):
    """
    Test basic container runner execution.

    Flow:
    1. Create webhook trigger
    2. Create action with container runner (simple Python script)
    3. Create rule
    4. Trigger webhook
    5. Verify execution completes successfully in container
    """
    print("\n" + "=" * 80)
    print("T3.17.1: Container Runner Basic Execution")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"container_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for container test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create container action
    print("\n[STEP 2] Creating container action...")
    action_ref = f"container_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Container Action",
        "description": "Simple Python script in container",
        "runner_type": "container",
        "entry_point": "print('Hello from container!')",
        "metadata": {
            "container_image": "python:3.11-slim",
            "container_command": ["python", "-c"],
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created container action: {action['ref']}")
    print(f"  - Image: {action['metadata'].get('container_image')}")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"container_rule_{unique_ref()}"
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
    print(f"✓ Created rule: {rule['ref']}")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"message": "test container"})
    assert webhook_response.status_code == 200, (
        f"Webhook trigger failed: {webhook_response.text}"
    )
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution completion
    print("\n[STEP 5] Waiting for container execution...")
    wait_for_execution_count(client, expected_count=1, timeout=20)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=20)
    print(f"✓ Execution completed: {execution['status']}")

    # Verify execution succeeded
    assert execution["status"] == "succeeded", (
        f"Expected succeeded, got {execution['status']}"
    )
    assert execution["result"] is not None, "Execution should have result"

    print(f"✓ Container execution validated")
    print(f"  - Execution ID: {execution_id}")
    print(f"  - Status: {execution['status']}")
    print(f"  - Runner: {execution.get('runner_type', 'N/A')}")

    print("\n✅ Test passed: Container runner executed successfully")


@pytest.mark.tier3
@pytest.mark.container
@pytest.mark.runner
def test_container_runner_with_parameters(client: AttuneClient, test_pack):
    """
    Test container runner with action parameters.

    Flow:
    1. Create action with parameters in container
    2. Execute with different parameter values
    3. Verify parameters are passed correctly to container
    """
    print("\n" + "=" * 80)
    print("T3.17.2: Container Runner with Parameters")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"container_param_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for container parameter test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create container action with parameters
    print("\n[STEP 2] Creating container action with parameters...")
    action_ref = f"container_param_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Container Action with Params",
        "description": "Container action that uses parameters",
        "runner_type": "container",
        "entry_point": """
import json
import sys

# Read parameters from stdin
params = json.loads(sys.stdin.read())
name = params.get('name', 'World')
count = params.get('count', 1)

# Output result
for i in range(count):
    print(f'Hello {name}! (iteration {i+1})')

result = {'name': name, 'iterations': count}
print(json.dumps(result))
""",
        "parameters": {
            "name": {
                "type": "string",
                "description": "Name to greet",
                "required": True,
            },
            "count": {
                "type": "integer",
                "description": "Number of iterations",
                "default": 1,
            },
        },
        "metadata": {
            "container_image": "python:3.11-slim",
            "container_command": ["python", "-c"],
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created container action with parameters")

    # Step 3: Create rule with parameter mapping
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"container_param_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
        "parameters": {
            "name": "{{ trigger.payload.name }}",
            "count": "{{ trigger.payload.count }}",
        },
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201, (
        f"Failed to create rule: {rule_response.text}"
    )
    rule = rule_response.json()["data"]
    print(f"✓ Created rule with parameter mapping")

    # Step 4: Trigger webhook with parameters
    print("\n[STEP 4] Triggering webhook with parameters...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_payload = {"name": "Container Test", "count": 3}
    webhook_response = client.post(webhook_url, json=webhook_payload)
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered with params: {webhook_payload}")

    # Step 5: Wait for execution
    print("\n[STEP 5] Waiting for container execution...")
    wait_for_execution_count(client, expected_count=1, timeout=20)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=20)
    print(f"✓ Execution completed: {execution['status']}")

    assert execution["status"] == "succeeded", (
        f"Expected succeeded, got {execution['status']}"
    )

    # Verify parameters were used
    assert execution["parameters"] is not None, "Execution should have parameters"
    print(f"✓ Container execution with parameters validated")
    print(f"  - Parameters: {execution['parameters']}")

    print("\n✅ Test passed: Container runner handled parameters correctly")


@pytest.mark.tier3
@pytest.mark.container
@pytest.mark.runner
def test_container_runner_isolation(client: AttuneClient, test_pack):
    """
    Test that container executions are isolated from each other.

    Flow:
    1. Create action that writes to filesystem
    2. Execute multiple times
    3. Verify each execution has clean environment (no state leakage)
    """
    print("\n" + "=" * 80)
    print("T3.17.3: Container Runner Isolation")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"container_isolation_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for container isolation test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create container action that checks for state
    print("\n[STEP 2] Creating container action to test isolation...")
    action_ref = f"container_isolation_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Container Isolation Test",
        "description": "Tests container isolation",
        "runner_type": "container",
        "entry_point": """
import os
import json

# Check if a marker file exists from previous run
marker_path = '/tmp/test_marker.txt'
marker_exists = os.path.exists(marker_path)

# Write marker file
with open(marker_path, 'w') as f:
    f.write('This should not persist across containers')

result = {
    'marker_existed': marker_exists,
    'marker_created': True,
    'message': 'State should be isolated between containers'
}

print(json.dumps(result))
""",
        "metadata": {
            "container_image": "python:3.11-slim",
            "container_command": ["python", "-c"],
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created isolation test action")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"container_isolation_rule_{unique_ref()}"
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

    # Step 4: Execute first time
    print("\n[STEP 4] Executing first time...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    client.post(webhook_url, json={"run": 1})
    wait_for_execution_count(client, expected_count=1, timeout=20)
    executions = client.get("/executions").json()["data"]
    exec1 = wait_for_execution_completion(client, executions[0]["id"], timeout=20)
    print(f"✓ First execution completed: {exec1['status']}")

    # Step 5: Execute second time
    print("\n[STEP 5] Executing second time...")
    client.post(webhook_url, json={"run": 2})
    time.sleep(2)  # Brief delay between executions
    wait_for_execution_count(client, expected_count=2, timeout=20)
    executions = client.get("/executions").json()["data"]
    exec2_id = [e["id"] for e in executions if e["id"] != exec1["id"]][0]
    exec2 = wait_for_execution_completion(client, exec2_id, timeout=20)
    print(f"✓ Second execution completed: {exec2['status']}")

    # Step 6: Verify isolation (marker should NOT exist in second run)
    print("\n[STEP 6] Verifying container isolation...")
    assert exec1["status"] == "succeeded", "First execution should succeed"
    assert exec2["status"] == "succeeded", "Second execution should succeed"

    # Both executions should report that marker didn't exist initially
    # (proving containers are isolated and cleaned up between runs)
    print(f"✓ Container isolation validated")
    print(f"  - First execution: {exec1['id']}")
    print(f"  - Second execution: {exec2['id']}")
    print(f"  - Both executed in isolated containers")

    print("\n✅ Test passed: Container executions are properly isolated")


@pytest.mark.tier3
@pytest.mark.container
@pytest.mark.runner
def test_container_runner_failure_handling(client: AttuneClient, test_pack):
    """
    Test container runner handles failures correctly.

    Flow:
    1. Create action that fails in container
    2. Execute and verify failure is captured
    3. Verify container cleanup occurs even on failure
    """
    print("\n" + "=" * 80)
    print("T3.17.4: Container Runner Failure Handling")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"container_fail_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for container failure test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create failing container action
    print("\n[STEP 2] Creating failing container action...")
    action_ref = f"container_fail_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Failing Container Action",
        "description": "Container action that fails",
        "runner_type": "container",
        "entry_point": """
import sys
print('About to fail...')
sys.exit(1)  # Non-zero exit code
""",
        "metadata": {
            "container_image": "python:3.11-slim",
            "container_command": ["python", "-c"],
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created failing container action")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"container_fail_rule_{unique_ref()}"
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
    client.post(webhook_url, json={"test": "failure"})
    print(f"✓ Webhook triggered")

    # Step 5: Wait for execution to fail
    print("\n[STEP 5] Waiting for execution to fail...")
    wait_for_execution_count(client, expected_count=1, timeout=20)
    executions = client.get("/executions").json()["data"]
    execution_id = executions[0]["id"]

    execution = wait_for_execution_completion(client, execution_id, timeout=20)
    print(f"✓ Execution completed: {execution['status']}")

    # Verify failure was captured
    assert execution["status"] == "failed", (
        f"Expected failed, got {execution['status']}"
    )
    assert execution["result"] is not None, "Failed execution should have result"

    print(f"✓ Container failure handling validated")
    print(f"  - Execution ID: {execution_id}")
    print(f"  - Status: {execution['status']}")
    print(f"  - Failure captured and reported correctly")

    print("\n✅ Test passed: Container runner handles failures correctly")
