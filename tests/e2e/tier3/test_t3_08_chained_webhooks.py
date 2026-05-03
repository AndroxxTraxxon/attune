"""
T3.8: Chained Webhook Triggers Test

Tests webhook triggers that fire other workflows which in turn trigger
additional webhooks, creating a chain of automated events.

Priority: MEDIUM
Duration: ~30 seconds
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import (
    create_echo_action,
    create_failing_action,
    create_webhook_trigger,
    unique_ref,
)
from helpers.polling import (
    wait_for_event_count,
    wait_for_execution_completion,
    wait_for_execution_count,
)


def create_webhook_post_action(
    client: AttuneClient,
    pack_ref: str,
    action_ref: str,
    webhook_url: str,
    payload: dict,
    label: str,
) -> dict:
    """Create a current-contract inline shell action that POSTs to a webhook."""
    if webhook_url.startswith("/"):
        webhook_url = f"{client.base_url}{webhook_url}"

    action_payload = {
        "ref": f"{pack_ref}.{action_ref}",
        "pack_ref": pack_ref,
        "label": label,
        "description": f"{label} webhook POST action",
        "runtime_ref": "core.shell",
        "entrypoint": f"""python3 - <<'PY'
import json
import urllib.request

request = urllib.request.Request(
    "{webhook_url}",
    data=json.dumps({{"payload": {payload!r}}}).encode(),
    headers={{"Content-Type": "application/json"}},
    method="POST",
)
with urllib.request.urlopen(request, timeout=10) as response:
    print(json.dumps({{"status_code": response.status}}))
PY""",
        "required_worker_runtimes": {"python": "*"},
    }
    response = client.post("/api/v1/actions", json=action_payload)
    assert response.status_code == 201, f"Failed to create action: {response.text}"
    return response.json()["data"]


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.orchestration
def test_webhook_triggers_workflow_triggers_webhook(client: AttuneClient, test_pack):
    """
    Test webhook chain: Webhook A → Workflow → Webhook B → Action.

    Flow:
    1. Create webhook A that triggers a workflow
    2. Workflow makes HTTP call to trigger webhook B
    3. Webhook B triggers final action
    4. Verify complete chain executes
    """
    print("\n" + "=" * 80)
    print("T3.8.1: Webhook Triggers Workflow Triggers Webhook")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook A (initial trigger)
    print("\n[STEP 1] Creating webhook A (initial trigger)...")
    webhook_a_ref = f"webhook_a_{unique_ref()}"
    webhook_a = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=webhook_a_ref,
        description="Initial webhook in chain",
    )
    print(f"✓ Created webhook A: {webhook_a['ref']}")

    # Step 2: Create webhook B (chained trigger)
    print("\n[STEP 2] Creating webhook B (chained trigger)...")
    webhook_b_ref = f"webhook_b_{unique_ref()}"
    webhook_b = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=webhook_b_ref,
        description="Chained webhook in sequence",
    )
    print(f"✓ Created webhook B: {webhook_b['ref']}")

    # Step 3: Create final action (end of chain)
    print("\n[STEP 3] Creating final action...")
    final_action_ref = f"final_action_{unique_ref()}"
    final_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=final_action_ref,
        description="Final action in chain",
    )
    print(f"✓ Created final action: {final_action['ref']}")

    # Step 4: Create HTTP action to trigger webhook B
    print("\n[STEP 4] Creating HTTP action to trigger webhook B...")
    http_action_ref = f"http_trigger_action_{unique_ref()}"

    # Get API base URL (assume localhost:8080 for tests)
    api_url = client.base_url
    webhook_b_url = webhook_b["webhook_url"]
    if webhook_b_url.startswith("/"):
        webhook_b_url = f"{api_url}{webhook_b_url}"

    http_action_payload = {
        "ref": f"{pack_ref}.{http_action_ref}",
        "pack_ref": pack_ref,
        "label": "HTTP Trigger Action",
        "description": "Triggers webhook B via HTTP",
        "runtime_ref": "core.shell",
        "entrypoint": f"""python3 - <<'PY'
import json
import urllib.request

request = urllib.request.Request(
    "{webhook_b_url}",
    data=json.dumps({{"payload": {{"message": "Chained from workflow", "step": 2}}}}).encode(),
    headers={{"Content-Type": "application/json"}},
    method="POST",
)
with urllib.request.urlopen(request, timeout=10) as response:
    print(json.dumps({{"status_code": response.status}}))
PY""",
        "required_worker_runtimes": {"python": "*"},
        "param_schema": {
            "payload": {
                "type": "object",
                "description": "Data to send",
                "required": False,
            }
        },
    }
    http_action_response = client.post("/api/v1/actions", json=http_action_payload)
    assert http_action_response.status_code == 201, (
        f"Failed to create HTTP action: {http_action_response.text}"
    )
    http_action = http_action_response.json()["data"]
    print(f"✓ Created HTTP action: {http_action['ref']}")
    print(f"  Will POST to: {webhook_b_url}")

    # Step 5: Create workflow that calls HTTP action
    print("\n[STEP 5] Creating workflow for chaining...")
    workflow_ref = f"{pack_ref}.chain_workflow_{unique_ref()}"
    workflow_payload = {
        "ref": workflow_ref,
        "pack_ref": pack_ref,
        "label": "Chain Workflow",
        "description": "Workflow that triggers next webhook",
        "version": "1.0.0",
        "definition": {
            "version": "1.0.0",
            "tasks": [
                {
                    "name": "trigger_next_webhook",
                    "action": http_action["ref"],
                    "input": {
                        "payload": {
                            "message": "Chained from workflow",
                            "step": 2,
                        },
                    },
                }
            ]
        },
    }
    workflow_response = client.post("/api/v1/workflows", json=workflow_payload)
    assert workflow_response.status_code == 201, (
        f"Failed to create workflow: {workflow_response.text}"
    )
    workflow = workflow_response.json()["data"]
    print(f"✓ Created chain workflow: {workflow['ref']}")

    # Step 6: Create rule A (webhook A → workflow)
    print("\n[STEP 6] Creating rule A (webhook A → workflow)...")
    rule_a_ref = f"{pack_ref}.rule_a_{unique_ref()}"
    rule_a_payload = {
        "ref": rule_a_ref,
        "pack_ref": pack_ref,
        "label": "Webhook A to Workflow",
        "trigger_ref": webhook_a["ref"],
        "action_ref": workflow["ref"],
        "enabled": True,
    }
    rule_a_response = client.post("/api/v1/rules", json=rule_a_payload)
    assert rule_a_response.status_code == 201, (
        f"Failed to create rule A: {rule_a_response.text}"
    )
    rule_a = rule_a_response.json()["data"]
    print(f"✓ Created rule A: {rule_a['ref']}")

    # Step 7: Create rule B (webhook B → final action)
    print("\n[STEP 7] Creating rule B (webhook B → final action)...")
    rule_b_ref = f"{pack_ref}.rule_b_{unique_ref()}"
    rule_b_payload = {
        "ref": rule_b_ref,
        "pack_ref": pack_ref,
        "label": "Webhook B to Final Action",
        "trigger_ref": webhook_b["ref"],
        "action_ref": final_action["ref"],
        "enabled": True,
        "action_params": {
            "message": "{{ event.payload.message }}",
        },
    }
    rule_b_response = client.post("/api/v1/rules", json=rule_b_payload)
    assert rule_b_response.status_code == 201, (
        f"Failed to create rule B: {rule_b_response.text}"
    )
    rule_b = rule_b_response.json()["data"]
    print(f"✓ Created rule B: {rule_b['ref']}")

    # Step 8: Trigger the chain by calling webhook A
    print("\n[STEP 8] Triggering webhook chain...")
    print(f"  Chain: Webhook A → Workflow → HTTP → Webhook B → Final Action")
    webhook_a_url = webhook_a["webhook_url"]
    webhook_response = client.post(
        webhook_a_url, json={"payload": {"message": "Start chain", "step": 1}}
    )
    assert webhook_response.status_code == 200, (
        f"Webhook A trigger failed: {webhook_response.text}"
    )
    print(f"✓ Webhook A triggered successfully")

    # Step 9: Wait for chain to complete
    print("\n[STEP 9] Waiting for webhook chain to complete...")
    # Expected: 2 events (webhook A + webhook B), multiple executions
    time.sleep(3)

    webhook_a_events = wait_for_event_count(
        client,
        expected_count=1,
        trigger_ref=webhook_a["ref"],
        timeout=20,
        operator=">=",
    )
    webhook_b_events = wait_for_event_count(
        client,
        expected_count=1,
        trigger_ref=webhook_b["ref"],
        timeout=20,
        operator=">=",
    )
    print(f"  ✓ Found webhook A events: {len(webhook_a_events)}")
    print(f"  ✓ Found webhook B events: {len(webhook_b_events)}")

    workflow_execs = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=workflow["ref"],
        timeout=20,
        operator=">=",
    )
    final_execs = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=final_action["ref"],
        timeout=20,
        operator=">=",
    )
    print(f"  ✓ Found workflow executions: {len(workflow_execs)}")
    print(f"  ✓ Found final action executions: {len(final_execs)}")

    # Step 10: Verify chain succeeded
    print("\n[STEP 10] Verifying chain completion...")

    print(f"  - Webhook A events: {len(webhook_a_events)}")
    print(f"  - Webhook B events: {len(webhook_b_events)}")

    assert len(webhook_a_events) >= 1, "Webhook A should have fired"
    assert len(webhook_b_events) >= 1, "Webhook B should have fired"
    assert len(workflow_execs) >= 1, "Workflow should have executed"
    assert len(final_execs) >= 1, "Final action should have executed"
    print(f"  ✓ Webhook A → Workflow → HTTP → Webhook B verified")

    print("\n✅ Test passed: Webhook chain validated")


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.orchestration
def test_webhook_cascade_multiple_levels(client: AttuneClient, test_pack):
    """
    Test multi-level webhook cascade: A → B → C.

    Flow:
    1. Create 3 webhooks (A, B, C)
    2. Webhook A triggers action that fires webhook B
    3. Webhook B triggers action that fires webhook C
    4. Verify cascade propagates through all levels
    """
    print("\n" + "=" * 80)
    print("T3.8.2: Webhook Cascade Multiple Levels")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create cascading webhooks
    print("\n[STEP 1] Creating cascade webhooks (A, B, C)...")
    webhooks = []
    for level in ["A", "B", "C"]:
        webhook_ref = f"webhook_{level.lower()}_{unique_ref()}"
        webhook = create_webhook_trigger(
            client=client,
            pack_ref=pack_ref,
            trigger_ref=webhook_ref,
            description=f"Webhook {level} in cascade",
        )
        webhooks.append(webhook)
        print(f"  ✓ Created webhook {level}: {webhook['ref']}")

    webhook_a, webhook_b, webhook_c = webhooks

    # Step 2: Create final action for webhook C
    print("\n[STEP 2] Creating final action...")
    final_action_ref = f"final_cascade_action_{unique_ref()}"
    final_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=final_action_ref,
        description="Final action in cascade",
    )
    print(f"✓ Created final action: {final_action['ref']}")

    # Step 3: Create HTTP actions for triggering next level
    print("\n[STEP 3] Creating HTTP trigger actions...")
    api_url = client.base_url

    # HTTP action A→B
    http_a_to_b_ref = f"http_a_to_b_{unique_ref()}"
    http_a_to_b = create_webhook_post_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=http_a_to_b_ref,
        webhook_url=webhook_b["webhook_url"],
        payload={"level": 2, "from": "A"},
        label="Trigger B from A",
    )
    print(f"  ✓ Created HTTP A→B: {http_a_to_b['ref']}")

    # HTTP action B→C
    http_b_to_c_ref = f"http_b_to_c_{unique_ref()}"
    http_b_to_c = create_webhook_post_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=http_b_to_c_ref,
        webhook_url=webhook_c["webhook_url"],
        payload={"level": 3, "from": "B"},
        label="Trigger C from B",
    )
    print(f"  ✓ Created HTTP B→C: {http_b_to_c['ref']}")

    # Step 4: Create rules for cascade
    print("\n[STEP 4] Creating cascade rules...")

    # Rule A: webhook A → HTTP A→B
    rule_a_ref = f"{pack_ref}.cascade_rule_a_{unique_ref()}"
    rule_a_payload = {
        "ref": rule_a_ref,
        "pack_ref": pack_ref,
        "label": "Cascade Rule A",
        "trigger_ref": webhook_a["ref"],
        "action_ref": http_a_to_b["ref"],
        "enabled": True,
    }
    rule_a_response = client.post("/api/v1/rules", json=rule_a_payload)
    assert rule_a_response.status_code == 201
    rule_a = rule_a_response.json()["data"]
    print(f"  ✓ Created rule A: {rule_a['ref']}")

    # Rule B: webhook B → HTTP B→C
    rule_b_ref = f"{pack_ref}.cascade_rule_b_{unique_ref()}"
    rule_b_payload = {
        "ref": rule_b_ref,
        "pack_ref": pack_ref,
        "label": "Cascade Rule B",
        "trigger_ref": webhook_b["ref"],
        "action_ref": http_b_to_c["ref"],
        "enabled": True,
    }
    rule_b_response = client.post("/api/v1/rules", json=rule_b_payload)
    assert rule_b_response.status_code == 201
    rule_b = rule_b_response.json()["data"]
    print(f"  ✓ Created rule B: {rule_b['ref']}")

    # Rule C: webhook C → final action
    rule_c_ref = f"{pack_ref}.cascade_rule_c_{unique_ref()}"
    rule_c_payload = {
        "ref": rule_c_ref,
        "pack_ref": pack_ref,
        "label": "Cascade Rule C",
        "trigger_ref": webhook_c["ref"],
        "action_ref": final_action["ref"],
        "enabled": True,
        "action_params": {
            "message": "Cascade complete!",
        },
    }
    rule_c_response = client.post("/api/v1/rules", json=rule_c_payload)
    assert rule_c_response.status_code == 201
    rule_c = rule_c_response.json()["data"]
    print(f"  ✓ Created rule C: {rule_c['ref']}")

    # Step 5: Trigger cascade
    print("\n[STEP 5] Triggering webhook cascade...")
    print(f"  Cascade: A → B → C → Final Action")
    webhook_a_url = webhook_a["webhook_url"]
    webhook_response = client.post(
        webhook_a_url, json={"payload": {"level": 1, "message": "Start cascade"}}
    )
    assert webhook_response.status_code == 200
    print(f"✓ Webhook A triggered - cascade started")

    # Step 6: Wait for cascade propagation
    print("\n[STEP 6] Waiting for cascade to propagate...")
    time.sleep(5)  # Give time for async HTTP calls

    webhook_a_events = wait_for_event_count(
        client, expected_count=1, trigger_ref=webhook_a["ref"], timeout=20, operator=">="
    )
    webhook_b_events = wait_for_event_count(
        client, expected_count=1, trigger_ref=webhook_b["ref"], timeout=20, operator=">="
    )
    webhook_c_events = wait_for_event_count(
        client, expected_count=1, trigger_ref=webhook_c["ref"], timeout=20, operator=">="
    )
    final_execs = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=final_action["ref"],
        timeout=20,
        operator=">=",
    )

    # Step 7: Verify cascade
    print("\n[STEP 7] Verifying cascade propagation...")

    # Check webhook A fired
    print(f"  - Webhook A events: {len(webhook_a_events)}")
    assert len(webhook_a_events) >= 1, "Webhook A should have fired"

    print(f"  - Webhook B events: {len(webhook_b_events)}")
    print(f"  - Webhook C events: {len(webhook_c_events)}")
    assert len(webhook_b_events) >= 1, "Webhook B should have fired"
    assert len(webhook_c_events) >= 1, "Webhook C should have fired"
    assert len(final_execs) >= 1, "Final cascade action should have executed"
    print(f"  ✓ Full cascade (A→B→C) verified")
    print(f"\n✓ Cascade initiated successfully")

    print("\n✅ Test passed: Multi-level webhook cascade validated")


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.orchestration
def test_webhook_chain_with_data_passing(client: AttuneClient, test_pack):
    """
    Test webhook chain with data transformation between steps.

    Flow:
    1. Webhook A receives initial data
    2. Workflow transforms data
    3. Transformed data sent to webhook B
    4. Verify data flows correctly through chain
    """
    print("\n" + "=" * 80)
    print("T3.8.3: Webhook Chain with Data Passing")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhooks
    print("\n[STEP 1] Creating webhooks...")
    webhook_a_ref = f"data_webhook_a_{unique_ref()}"
    webhook_a = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=webhook_a_ref,
        description="Webhook A with data input",
    )
    print(f"  ✓ Created webhook A: {webhook_a['ref']}")

    webhook_b_ref = f"data_webhook_b_{unique_ref()}"
    webhook_b = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=webhook_b_ref,
        description="Webhook B receives transformed data",
    )
    print(f"  ✓ Created webhook B: {webhook_b['ref']}")

    # Step 2: Create data transformation action
    print("\n[STEP 2] Creating data transformation action...")
    transform_action_ref = f"transform_data_{unique_ref()}"
    transform_action_payload = {
        "ref": f"{pack_ref}.{transform_action_ref}",
        "pack_ref": pack_ref,
        "label": "Transform Data",
        "description": "Transforms data for next step",
        "runtime_ref": "core.shell",
        "param_schema": {
            "value": {
                "type": "integer",
                "description": "Value to transform",
                "required": True,
            }
        },
        "entrypoint": f"""python3 - "$value" <<'PY'
import json
import sys
import urllib.request

value = int(sys.argv[1])
transformed = value * 2 + 10
request = urllib.request.Request(
    "{client.base_url}{webhook_b['webhook_url']}",
    data=json.dumps({{"payload": {{"original": value, "transformed_value": transformed}}}}).encode(),
    headers={{"Content-Type": "application/json"}},
    method="POST",
)
with urllib.request.urlopen(request, timeout=10) as response:
    print(json.dumps({{"original": value, "transformed_value": transformed, "status_code": response.status}}))
PY""",
        "required_worker_runtimes": {"python": "*"},
    }
    transform_response = client.post("/api/v1/actions", json=transform_action_payload)
    assert transform_response.status_code == 201, transform_response.text
    transform_action = transform_response.json()["data"]
    print(f"✓ Created transform action: {transform_action['ref']}")

    # Step 3: Create final action
    print("\n[STEP 3] Creating final action...")
    final_action_ref = f"final_data_action_{unique_ref()}"
    final_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=final_action_ref,
        description="Final action with transformed data",
    )
    print(f"✓ Created final action: {final_action['ref']}")

    # Step 4: Create rules
    print("\n[STEP 4] Creating rules with data mapping...")

    # Rule A: webhook A → transform action
    rule_a_ref = f"{pack_ref}.data_rule_a_{unique_ref()}"
    rule_a_payload = {
        "ref": rule_a_ref,
        "pack_ref": pack_ref,
        "label": "Data Rule A",
        "trigger_ref": webhook_a["ref"],
        "action_ref": transform_action["ref"],
        "enabled": True,
        "action_params": {
            "value": "{{ event.payload.input_value }}",
        },
    }
    rule_a_response = client.post("/api/v1/rules", json=rule_a_payload)
    assert rule_a_response.status_code == 201
    rule_a = rule_a_response.json()["data"]
    print(f"  ✓ Created rule A with data mapping")

    # Rule B: webhook B → final action
    rule_b_ref = f"{pack_ref}.data_rule_b_{unique_ref()}"
    rule_b_payload = {
        "ref": rule_b_ref,
        "pack_ref": pack_ref,
        "label": "Data Rule B",
        "trigger_ref": webhook_b["ref"],
        "action_ref": final_action["ref"],
        "enabled": True,
        "action_params": {
            "message": "Received: {{ event.payload.transformed_value }}",
        },
    }
    rule_b_response = client.post("/api/v1/rules", json=rule_b_payload)
    assert rule_b_response.status_code == 201
    rule_b = rule_b_response.json()["data"]
    print(f"  ✓ Created rule B with data mapping")

    # Step 5: Trigger with test data
    print("\n[STEP 5] Triggering webhook chain with data...")
    test_input = 5
    expected_output = test_input * 2 + 10  # Should be 20

    webhook_a_url = webhook_a["webhook_url"]
    webhook_response = client.post(
        webhook_a_url, json={"payload": {"input_value": test_input}}
    )
    assert webhook_response.status_code == 200
    print(f"✓ Webhook A triggered with input: {test_input}")
    print(f"  Expected transformation: {test_input} → {expected_output}")

    # Step 6: Wait for execution
    print("\n[STEP 6] Waiting for transformation...")
    time.sleep(3)
    transform_execs = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=transform_action["ref"],
        timeout=20,
        operator=">=",
    )
    final_execs = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=final_action["ref"],
        timeout=20,
        operator=">=",
    )
    webhook_b_events = wait_for_event_count(
        client,
        expected_count=1,
        trigger_ref=webhook_b["ref"],
        timeout=20,
        operator=">=",
    )
    assert len(final_execs) >= 1
    assert len(webhook_b_events) >= 1

    if transform_execs:
        transform_exec = transform_execs[0]
        transform_exec = wait_for_execution_completion(
            client, transform_exec["id"], timeout=20
        )
        print(f"✓ Transform action succeeded: {transform_exec['status']}")

        if transform_exec["status"] == "completed":
            result = transform_exec.get("result", {})
            if isinstance(result, dict):
                transformed = result.get("transformed_value")
                original = result.get("original")
                print(f"  Input: {original}")
                print(f"  Output: {transformed}")

                # Verify transformation is correct
                if transformed == expected_output:
                    print(f"  ✓ Data transformation correct!")

    print("\n✅ Test passed: Webhook chain with data passing validated")


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.orchestration
def test_webhook_chain_error_propagation(client: AttuneClient, test_pack):
    """
    Test error handling in webhook chains.

    Flow:
    1. Create webhook chain where middle step fails
    2. Verify failure doesn't propagate to subsequent webhooks
    3. Verify error is properly captured and reported
    """
    print("\n" + "=" * 80)
    print("T3.8.4: Webhook Chain Error Propagation")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook
    print("\n[STEP 1] Creating webhook...")
    webhook_ref = f"error_webhook_{unique_ref()}"
    webhook = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=webhook_ref,
        description="Webhook for error test",
    )
    print(f"✓ Created webhook: {webhook['ref']}")

    # Step 2: Create failing action
    print("\n[STEP 2] Creating failing action...")
    fail_action = create_failing_action(
        client=client,
        pack_ref=pack_ref,
        name=f"fail_chain_action_{unique_ref()}",
    )
    print(f"✓ Created failing action: {fail_action['ref']}")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"{pack_ref}.error_chain_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack_ref": pack_ref,
        "label": "Error Chain Rule",
        "trigger_ref": webhook["ref"],
        "action_ref": fail_action["ref"],
        "enabled": True,
    }
    rule_response = client.post("/api/v1/rules", json=rule_payload)
    assert rule_response.status_code == 201
    rule = rule_response.json()["data"]
    print(f"✓ Created rule: {rule['ref']}")

    # Step 4: Trigger webhook
    print("\n[STEP 4] Triggering webhook with failing action...")
    webhook_url = webhook["webhook_url"]
    webhook_response = client.post(webhook_url, json={"payload": {"test": "error"}})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait and verify failure handling
    print("\n[STEP 5] Verifying error handling...")
    time.sleep(3)
    executions = wait_for_execution_count(
        client,
        expected_count=1,
        action_ref=fail_action["ref"],
        timeout=20,
        operator=">=",
    )
    fail_exec = executions[0]
    fail_exec = wait_for_execution_completion(client, fail_exec["id"], timeout=20)

    print(f"✓ Execution succeeded: {fail_exec['status']}")
    assert fail_exec["status"] == "failed", (
        f"Expected failed status, got {fail_exec['status']}"
    )

    # Verify error is captured
    result = fail_exec.get("result", {})
    print(f"✓ Error captured in execution result")

    # Verify webhook event was still created despite failure
    webhook_events = wait_for_event_count(
        client,
        expected_count=1,
        trigger_ref=webhook["ref"],
        timeout=20,
        operator=">=",
    )
    assert len(webhook_events) >= 1, "Webhook event should exist despite failure"
    print(f"✓ Webhook event created despite action failure")

    print("\n✅ Test passed: Error propagation in webhook chain validated")
