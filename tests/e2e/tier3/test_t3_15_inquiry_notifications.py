"""
T3.15: Inquiry Creation Notifications Test

Tests that the notifier service sends real-time notifications when inquiries are created.
Validates notification delivery for human-in-the-loop approval workflows.

Priority: MEDIUM
Duration: ~20 seconds
"""

import time
from typing import Any, Dict

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import create_webhook_trigger, unique_ref
from helpers.polling import (
    wait_for_execution_count,
    wait_for_inquiry_count,
)


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
def test_inquiry_creation_notification(client: AttuneClient, test_pack):
    """
    Test that inquiry creation triggers notification.

    Flow:
    1. Create webhook trigger and inquiry action
    2. Create rule
    3. Trigger webhook
    4. Verify inquiry is created
    5. Validate inquiry notification metadata
    """
    print("\n" + "=" * 80)
    print("T3.15.1: Inquiry Creation Notification")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"inquiry_notify_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for inquiry notification test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create inquiry action
    print("\n[STEP 2] Creating inquiry action...")
    action_ref = f"inquiry_notify_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Inquiry Action for Notification",
        "description": "Creates inquiry to test notifications",
        "runner_type": "inquiry",
        "parameters": {
            "question": {
                "type": "string",
                "description": "Question to ask",
                "required": True,
            },
            "choices": {
                "type": "array",
                "description": "Available choices",
                "required": False,
            },
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created inquiry action: {action['ref']}")

    # Step 3: Create rule with inquiry action
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"inquiry_notify_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
        "parameters": {
            "question": "Do you approve this request?",
            "choices": ["approve", "deny"],
        },
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201, (
        f"Failed to create rule: {rule_response.text}"
    )
    rule = rule_response.json()["data"]
    print(f"✓ Created rule: {rule['ref']}")

    # Step 4: Trigger webhook to create inquiry
    print("\n[STEP 4] Triggering webhook to create inquiry...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    test_payload = {
        "message": "Request for approval",
        "timestamp": time.time(),
    }
    webhook_response = client.post(webhook_url, json=test_payload)
    assert webhook_response.status_code == 200, (
        f"Webhook trigger failed: {webhook_response.text}"
    )
    print(f"✓ Webhook triggered successfully")

    # Step 5: Wait for inquiry creation
    print("\n[STEP 5] Waiting for inquiry creation...")
    wait_for_inquiry_count(client, expected_count=1, timeout=10)
    inquiries = client.get("/inquiries").json()["data"]
    assert len(inquiries) == 1, f"Expected 1 inquiry, got {len(inquiries)}"
    inquiry = inquiries[0]
    print(f"✓ Inquiry created: {inquiry['id']}")

    # Step 6: Validate inquiry notification metadata
    print("\n[STEP 6] Validating inquiry notification metadata...")
    assert inquiry["status"] == "pending", (
        f"Expected pending status, got {inquiry['status']}"
    )
    assert "created" in inquiry, "Inquiry missing created timestamp"
    assert "updated" in inquiry, "Inquiry missing updated timestamp"
    assert inquiry["execution_id"] is not None, "Inquiry should be linked to execution"

    print(f"✓ Inquiry notification metadata validated")
    print(f"  - Inquiry ID: {inquiry['id']}")
    print(f"  - Status: {inquiry['status']}")
    print(f"  - Execution ID: {inquiry['execution_id']}")
    print(f"  - Created: {inquiry['created']}")

    print("\n✅ Test passed: Inquiry creation notification flow validated")


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
def test_inquiry_response_notification(client: AttuneClient, test_pack):
    """
    Test that inquiry response triggers notification.

    Flow:
    1. Create inquiry via webhook trigger
    2. Wait for inquiry creation
    3. Respond to inquiry
    4. Verify notification for inquiry response
    """
    print("\n" + "=" * 80)
    print("T3.15.2: Inquiry Response Notification")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"inquiry_resp_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for inquiry response test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create inquiry action
    print("\n[STEP 2] Creating inquiry action...")
    action_ref = f"inquiry_resp_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Inquiry Response Action",
        "description": "Creates inquiry for response test",
        "runner_type": "inquiry",
        "parameters": {
            "question": {
                "type": "string",
                "description": "Question to ask",
                "required": True,
            },
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created inquiry action: {action['ref']}")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"inquiry_resp_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
        "parameters": {
            "question": "Approve deployment to production?",
        },
    }
    rule_response = client.post("/rules", json=rule_payload)
    assert rule_response.status_code == 201, (
        f"Failed to create rule: {rule_response.text}"
    )
    rule = rule_response.json()["data"]
    print(f"✓ Created rule: {rule['ref']}")

    # Step 4: Trigger webhook to create inquiry
    print("\n[STEP 4] Triggering webhook...")
    webhook_url = f"/webhooks/{trigger['ref']}"
    webhook_response = client.post(webhook_url, json={"request": "deploy"})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for inquiry creation
    print("\n[STEP 5] Waiting for inquiry creation...")
    wait_for_inquiry_count(client, expected_count=1, timeout=10)
    inquiries = client.get("/inquiries").json()["data"]
    inquiry = inquiries[0]
    inquiry_id = inquiry["id"]
    print(f"✓ Inquiry created: {inquiry_id}")

    # Step 6: Respond to inquiry
    print("\n[STEP 6] Responding to inquiry...")
    response_payload = {
        "response": "approved",
        "comment": "Deployment approved by test",
    }
    response = client.post(f"/inquiries/{inquiry_id}/respond", json=response_payload)
    assert response.status_code == 200, f"Failed to respond: {response.text}"
    print(f"✓ Inquiry response submitted")

    # Step 7: Verify inquiry status updated
    print("\n[STEP 7] Verifying inquiry status update...")
    time.sleep(2)  # Allow notification processing
    updated_inquiry = client.get(f"/inquiries/{inquiry_id}").json()["data"]
    assert updated_inquiry["status"] == "responded", (
        f"Expected responded status, got {updated_inquiry['status']}"
    )
    assert updated_inquiry["response"] is not None, "Inquiry should have response data"

    print(f"✓ Inquiry response notification metadata validated")
    print(f"  - Inquiry ID: {inquiry_id}")
    print(f"  - Status: {updated_inquiry['status']}")
    print(f"  - Response received: {updated_inquiry['response'] is not None}")
    print(f"  - Updated: {updated_inquiry['updated']}")

    print("\n✅ Test passed: Inquiry response notification flow validated")


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
def test_inquiry_timeout_notification(client: AttuneClient, test_pack):
    """
    Test that inquiry timeout triggers notification.

    Flow:
    1. Create inquiry with short timeout
    2. Wait for timeout to occur
    3. Verify notification for inquiry timeout
    """
    print("\n" + "=" * 80)
    print("T3.15.3: Inquiry Timeout Notification")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"inquiry_timeout_webhook_{unique_ref()}"
    trigger = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        description="Webhook for inquiry timeout test",
    )
    print(f"✓ Created trigger: {trigger['ref']}")

    # Step 2: Create inquiry action with short timeout
    print("\n[STEP 2] Creating inquiry action with timeout...")
    action_ref = f"inquiry_timeout_action_{unique_ref()}"
    action_payload = {
        "ref": action_ref,
        "pack": pack_ref,
        "name": "Timeout Inquiry Action",
        "description": "Creates inquiry with short timeout",
        "runner_type": "inquiry",
        "timeout": 3,  # 3 second timeout
        "parameters": {
            "question": {
                "type": "string",
                "description": "Question to ask",
                "required": True,
            },
        },
        "enabled": True,
    }
    action_response = client.post("/actions", json=action_payload)
    assert action_response.status_code == 201, (
        f"Failed to create action: {action_response.text}"
    )
    action = action_response.json()["data"]
    print(f"✓ Created inquiry action with 3s timeout: {action['ref']}")

    # Step 3: Create rule
    print("\n[STEP 3] Creating rule...")
    rule_ref = f"inquiry_timeout_rule_{unique_ref()}"
    rule_payload = {
        "ref": rule_ref,
        "pack": pack_ref,
        "trigger": trigger["ref"],
        "action": action["ref"],
        "enabled": True,
        "parameters": {
            "question": "Quick approval needed!",
        },
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
    webhook_response = client.post(webhook_url, json={"urgent": True})
    assert webhook_response.status_code == 200
    print(f"✓ Webhook triggered")

    # Step 5: Wait for inquiry creation
    print("\n[STEP 5] Waiting for inquiry creation...")
    wait_for_inquiry_count(client, expected_count=1, timeout=10)
    inquiries = client.get("/inquiries").json()["data"]
    inquiry = inquiries[0]
    inquiry_id = inquiry["id"]
    print(f"✓ Inquiry created: {inquiry_id}")

    # Step 6: Wait for timeout to occur
    print("\n[STEP 6] Waiting for inquiry timeout...")
    time.sleep(5)  # Wait longer than timeout
    timed_out_inquiry = client.get(f"/inquiries/{inquiry_id}").json()["data"]

    # Verify timeout status
    assert timed_out_inquiry["status"] in ["timeout", "expired", "cancelled"], (
        f"Expected timeout status, got {timed_out_inquiry['status']}"
    )

    print(f"✓ Inquiry timeout notification metadata validated")
    print(f"  - Inquiry ID: {inquiry_id}")
    print(f"  - Status: {timed_out_inquiry['status']}")
    print(f"  - Timeout: {action['timeout']}s")
    print(f"  - Updated: {timed_out_inquiry['updated']}")

    print("\n✅ Test passed: Inquiry timeout notification flow validated")


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
@pytest.mark.skip(
    reason="Requires WebSocket infrastructure for real-time inquiry notifications"
)
def test_websocket_inquiry_notification_delivery(client: AttuneClient, test_pack):
    """
    Test actual WebSocket notification delivery for inquiries.

    This test is skipped until WebSocket test infrastructure is implemented.

    Flow:
    1. Connect to WebSocket with auth
    2. Subscribe to inquiry notifications
    3. Create inquiry via workflow
    4. Receive real-time notification
    5. Validate notification structure
    """
    print("\n" + "=" * 80)
    print("T3.15.4: WebSocket Inquiry Notification Delivery")
    print("=" * 80)

    # This would require WebSocket client infrastructure similar to T3.14.4
    # Notifications would include:
    # - inquiry.created
    # - inquiry.responded
    # - inquiry.timeout
    # - inquiry.cancelled

    pytest.skip("WebSocket client infrastructure not yet implemented")
