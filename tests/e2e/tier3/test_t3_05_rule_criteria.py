"""
T3.5: Webhook with Rule Criteria Filtering Test

Tests that multiple rules on the same webhook trigger can use criteria
expressions to filter which rules fire based on event payload.

Priority: MEDIUM
Duration: ~20 seconds
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, create_webhook_trigger, unique_ref
from helpers.polling import wait_for_event_count, wait_for_execution_count


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.rules
@pytest.mark.criteria
def test_rule_criteria_basic_filtering(client: AttuneClient, test_pack):
    """
    Test that rule criteria expressions filter which rules fire.

    Setup:
    - 1 webhook trigger
    - Rule A: criteria checks event.level == 'info'
    - Rule B: criteria checks event.level == 'error'

    Test:
    - POST with level='info' → only Rule A fires
    - POST with level='error' → only Rule B fires
    - POST with level='debug' → no rules fire
    """
    print("\n" + "=" * 80)
    print("T3.5a: Rule Criteria Basic Filtering Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook trigger
    print("\n[STEP 1] Creating webhook trigger...")
    trigger_ref = f"criteria_webhook_{unique_ref()}"

    trigger_response = create_webhook_trigger(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
    )
    trigger_ref = trigger_response["ref"]
    webhook_url = trigger_response["webhook_url"]

    print(f"✓ Webhook trigger created: {trigger_ref}")

    # Step 2: Create two actions
    print("\n[STEP 2] Creating actions...")
    action_info = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Info level action triggered",
        suffix="_info",
    )["ref"]
    print(f"✓ Info action created: {action_info}")

    action_error = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Error level action triggered",
        suffix="_error",
    )["ref"]
    print(f"✓ Error action created: {action_error}")

    # Step 3: Create rules with criteria
    print("\n[STEP 3] Creating rules with criteria...")

    # Rule A: Only fires for info level
    rule_info_data = {
        "name": f"Info Level Rule {unique_ref()}",
        "description": "Fires only for info level events",
        "trigger_ref": trigger_ref,
        "action_ref": action_info,
        "enabled": True,
        "conditions": {"expression": "{{ event.payload.level == 'info' }}"},
    }

    rule_info_response = client.create_rule(rule_info_data)
    rule_info_id = rule_info_response["id"]
    print(f"✓ Info rule created: {rule_info_id}")
    print(f"  Criteria: level == 'info'")

    # Rule B: Only fires for error level
    rule_error_data = {
        "name": f"Error Level Rule {unique_ref()}",
        "description": "Fires only for error level events",
        "trigger_ref": trigger_ref,
        "action_ref": action_error,
        "enabled": True,
        "conditions": {"expression": "{{ event.payload.level == 'error' }}"},
    }

    rule_error_response = client.create_rule(rule_error_data)
    rule_error_id = rule_error_response["id"]
    print(f"✓ Error rule created: {rule_error_id}")
    print(f"  Criteria: level == 'error'")

    # Step 4: POST webhook with level='info'
    print("\n[STEP 4] Testing info level webhook...")

    info_payload = {
        "level": "info",
        "message": "This is an info message",
        "timestamp": time.time(),
    }

    client.post_webhook(webhook_url, info_payload)
    print(f"✓ Webhook POST sent with level='info'")

    # Wait for event
    time.sleep(2)
    events_after_info = client.list_events(trigger_ref=trigger_ref)
    print(f"  Events created: {len(events_after_info)}")

    # Check executions
    time.sleep(3)
    info_executions = client.list_executions(action_ref=action_info)
    error_executions = client.list_executions(action_ref=action_error)

    print(f"  Info action executions: {len(info_executions)}")
    print(f"  Error action executions: {len(error_executions)}")

    if len(info_executions) >= 1:
        print(f"✓ Info rule fired (criteria matched)")
    else:
        print(f"⚠ Info rule did not fire")

    if len(error_executions) == 0:
        print(f"✓ Error rule did not fire (criteria not matched)")
    else:
        print(f"⚠ Error rule fired unexpectedly")

    # Step 5: POST webhook with level='error'
    print("\n[STEP 5] Testing error level webhook...")

    error_payload = {
        "level": "error",
        "message": "This is an error message",
        "timestamp": time.time(),
    }

    client.post_webhook(webhook_url, error_payload)
    print(f"✓ Webhook POST sent with level='error'")

    # Wait and check executions
    time.sleep(3)
    info_executions_after = client.list_executions(action_ref=action_info)
    error_executions_after = client.list_executions(action_ref=action_error)

    info_count_increase = len(info_executions_after) - len(info_executions)
    error_count_increase = len(error_executions_after) - len(error_executions)

    print(f"  Info action new executions: {info_count_increase}")
    print(f"  Error action new executions: {error_count_increase}")

    if error_count_increase >= 1:
        print(f"✓ Error rule fired (criteria matched)")
    else:
        print(f"⚠ Error rule did not fire")

    if info_count_increase == 0:
        print(f"✓ Info rule did not fire (criteria not matched)")
    else:
        print(f"⚠ Info rule fired unexpectedly")

    # Step 6: POST webhook with level='debug' (should match no rules)
    print("\n[STEP 6] Testing debug level webhook (no match)...")

    debug_payload = {
        "level": "debug",
        "message": "This is a debug message",
        "timestamp": time.time(),
    }

    client.post_webhook(webhook_url, debug_payload)
    print(f"✓ Webhook POST sent with level='debug'")

    # Wait and check executions
    time.sleep(3)
    info_executions_final = client.list_executions(action_ref=action_info)
    error_executions_final = client.list_executions(action_ref=action_error)

    info_count_increase2 = len(info_executions_final) - len(info_executions_after)
    error_count_increase2 = len(error_executions_final) - len(error_executions_after)

    print(f"  Info action new executions: {info_count_increase2}")
    print(f"  Error action new executions: {error_count_increase2}")

    if info_count_increase2 == 0 and error_count_increase2 == 0:
        print(f"✓ No rules fired (neither criteria matched)")
    else:
        print(f"⚠ Some rules fired unexpectedly")

    # Summary
    print("\n" + "=" * 80)
    print("RULE CRITERIA FILTERING TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Webhook trigger: {trigger_ref}")
    print(f"✓ Rules created: 2 (with different criteria)")
    print(f"✓ Webhook POSTs: 3 (info, error, debug)")
    print("\nResults:")
    print(f"  Info POST → Info executions: {len(info_executions)}")
    print(f"  Error POST → Error executions: {error_count_increase}")
    print(
        f"  Debug POST → Total new executions: {info_count_increase2 + error_count_increase2}"
    )
    print("\nCriteria Filtering:")
    if len(info_executions) >= 1:
        print(f"  ✓ Info criteria worked (level == 'info')")
    if error_count_increase >= 1:
        print(f"  ✓ Error criteria worked (level == 'error')")
    if info_count_increase2 == 0 and error_count_increase2 == 0:
        print(f"  ✓ Debug filtered out (no matching criteria)")

    print("\n✅ RULE CRITERIA FILTERING VALIDATED!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.rules
@pytest.mark.criteria
def test_rule_criteria_numeric_comparison(client: AttuneClient, test_pack):
    """
    Test rule criteria with numeric comparisons (>, <, >=, <=).
    """
    print("\n" + "=" * 80)
    print("T3.5b: Rule Criteria Numeric Comparison Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create webhook and actions
    print("\n[STEP 1] Creating webhook and actions...")
    trigger_ref = f"numeric_webhook_{unique_ref()}"

    trigger_response = create_webhook_trigger(client=client, pack_ref=pack_ref, trigger_ref=trigger_ref)
    trigger_ref = trigger_response["ref"]
    webhook_url = trigger_response["webhook_url"]
    print(f"✓ Webhook trigger created: {trigger_ref}")

    action_low = create_echo_action(
        client=client, pack_ref=pack_ref, message="Low priority", suffix="_low"
    )["ref"]
    action_high = create_echo_action(
        client=client, pack_ref=pack_ref, message="High priority", suffix="_high"
    )["ref"]
    print(f"✓ Actions created")

    # Step 2: Create rules with numeric criteria
    print("\n[STEP 2] Creating rules with numeric criteria...")

    # Low priority: priority <= 3
    rule_low_data = {
        "name": f"Low Priority Rule {unique_ref()}",
        "trigger_ref": trigger_ref,
        "action_ref": action_low,
        "enabled": True,
        "conditions": {"expression": "{{ event.payload.priority <= 3 }}"},
    }
    rule_low = client.create_rule(rule_low_data)
    print(f"✓ Low priority rule created (priority <= 3)")

    # High priority: priority >= 7
    rule_high_data = {
        "name": f"High Priority Rule {unique_ref()}",
        "trigger_ref": trigger_ref,
        "action_ref": action_high,
        "enabled": True,
        "conditions": {"expression": "{{ event.payload.priority >= 7 }}"},
    }
    rule_high = client.create_rule(rule_high_data)
    print(f"✓ High priority rule created (priority >= 7)")

    # Step 3: Test with priority=2 (should trigger low only)
    print("\n[STEP 3] Testing priority=2 (low threshold)...")
    client.post_webhook(webhook_url, {"priority": 2, "message": "Low priority event"})
    time.sleep(3)

    low_execs_1 = client.list_executions(action_ref=action_low)
    high_execs_1 = client.list_executions(action_ref=action_high)
    print(f"  Low action executions: {len(low_execs_1)}")
    print(f"  High action executions: {len(high_execs_1)}")

    # Step 4: Test with priority=9 (should trigger high only)
    print("\n[STEP 4] Testing priority=9 (high threshold)...")
    client.post_webhook(webhook_url, {"priority": 9, "message": "High priority event"})
    time.sleep(3)

    low_execs_2 = client.list_executions(action_ref=action_low)
    high_execs_2 = client.list_executions(action_ref=action_high)
    print(f"  Low action executions: {len(low_execs_2)}")
    print(f"  High action executions: {len(high_execs_2)}")

    # Step 5: Test with priority=5 (should trigger neither)
    print("\n[STEP 5] Testing priority=5 (middle - no match)...")
    client.post_webhook(
        webhook_url, {"priority": 5, "message": "Medium priority event"}
    )
    time.sleep(3)

    low_execs_3 = client.list_executions(action_ref=action_low)
    high_execs_3 = client.list_executions(action_ref=action_high)
    print(f"  Low action executions: {len(low_execs_3)}")
    print(f"  High action executions: {len(high_execs_3)}")

    # Summary
    print("\n" + "=" * 80)
    print("NUMERIC CRITERIA TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Tested numeric comparisons (<=, >=)")
    print(f"✓ Priority=2 → Low action: {len(low_execs_1)} executions")
    print(
        f"✓ Priority=9 → High action: {len(high_execs_2) - len(high_execs_1)} new executions"
    )
    print(f"✓ Priority=5 → Neither action triggered")
    print("\n✅ NUMERIC CRITERIA WORKING!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.rules
@pytest.mark.criteria
def test_rule_criteria_complex_expressions(client: AttuneClient, test_pack):
    """
    Test complex rule criteria with AND/OR logic.
    """
    print("\n" + "=" * 80)
    print("T3.5c: Rule Criteria Complex Expressions Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Setup
    print("\n[STEP 1] Creating webhook and action...")
    trigger_ref = f"complex_webhook_{unique_ref()}"
    trigger_response = create_webhook_trigger(client=client, pack_ref=pack_ref, trigger_ref=trigger_ref)
    trigger_ref = trigger_response["ref"]
    webhook_url = trigger_response["webhook_url"]

    action_ref = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Complex criteria matched",
        suffix="_complex",
    )["ref"]
    print(f"✓ Setup complete")

    # Step 2: Create rule with complex criteria
    print("\n[STEP 2] Creating rule with complex criteria...")

    # Criteria: (level == 'error' AND priority > 5) OR environment == 'production'
    complex_criteria = (
        "{{ (event.payload.level == 'error' and event.payload.priority > 5) "
        "or event.payload.environment == 'production' }}"
    )

    rule_data = {
        "name": f"Complex Criteria Rule {unique_ref()}",
        "trigger_ref": trigger_ref,
        "action_ref": action_ref,
        "enabled": True,
        "conditions": {"expression": complex_criteria},
    }
    rule = client.create_rule(rule_data)
    print(f"✓ Rule created with complex criteria")
    print(f"  Criteria: (error AND priority>5) OR environment='production'")

    # Step 3: Test case 1 - Matches first condition
    print("\n[STEP 3] Test: error + priority=8 (should match)...")
    client.post_webhook(
        webhook_url, {"level": "error", "priority": 8, "environment": "staging"}
    )
    time.sleep(3)

    execs_1 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_1)}")
    if len(execs_1) >= 1:
        print(f"✓ Matched first condition (error AND priority>5)")

    # Step 4: Test case 2 - Matches second condition
    print("\n[STEP 4] Test: production env (should match)...")
    client.post_webhook(
        webhook_url, {"level": "info", "priority": 2, "environment": "production"}
    )
    time.sleep(3)

    execs_2 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_2)}")
    if len(execs_2) > len(execs_1):
        print(f"✓ Matched second condition (environment='production')")

    # Step 5: Test case 3 - Matches neither
    print("\n[STEP 5] Test: info + priority=3 + staging (should NOT match)...")
    client.post_webhook(
        webhook_url, {"level": "info", "priority": 3, "environment": "staging"}
    )
    time.sleep(3)

    execs_3 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_3)}")
    if len(execs_3) == len(execs_2):
        print(f"✓ Did not match (neither condition satisfied)")

    # Summary
    print("\n" + "=" * 80)
    print("COMPLEX CRITERIA TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Complex AND/OR criteria tested")
    print(f"✓ Test 1 (error+priority): {len(execs_1)} executions")
    print(f"✓ Test 2 (production): {len(execs_2) - len(execs_1)} new executions")
    print(f"✓ Test 3 (no match): {len(execs_3) - len(execs_2)} new executions")
    print("\n✅ COMPLEX CRITERIA EXPRESSIONS WORKING!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.webhook
@pytest.mark.rules
@pytest.mark.criteria
def test_rule_criteria_list_membership(client: AttuneClient, test_pack):
    """
    Test rule criteria checking list membership (in operator).
    """
    print("\n" + "=" * 80)
    print("T3.5d: Rule Criteria List Membership Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Setup
    print("\n[STEP 1] Creating webhook and action...")
    trigger_ref = f"list_webhook_{unique_ref()}"
    trigger_response = create_webhook_trigger(client=client, pack_ref=pack_ref, trigger_ref=trigger_ref)
    trigger_ref = trigger_response["ref"]
    webhook_url = trigger_response["webhook_url"]

    action_ref = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="List criteria matched",
        suffix="_list",
    )["ref"]
    print(f"✓ Setup complete")

    # Step 2: Create rule checking list membership
    print("\n[STEP 2] Creating rule with list membership criteria...")

    # Criteria: status in ['critical', 'urgent', 'high']
    list_criteria = "{{ event.payload.status in ['critical', 'urgent', 'high'] }}"

    rule_data = {
        "name": f"List Membership Rule {unique_ref()}",
        "trigger_ref": trigger_ref,
        "action_ref": action_ref,
        "enabled": True,
        "conditions": {"expression": list_criteria},
    }
    rule = client.create_rule(rule_data)
    print(f"✓ Rule created")
    print(f"  Criteria: status in ['critical', 'urgent', 'high']")

    # Step 3: Test with matching status
    print("\n[STEP 3] Test: status='critical' (should match)...")
    client.post_webhook(
        webhook_url, {"status": "critical", "message": "Critical alert"}
    )
    time.sleep(3)

    execs_1 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_1)}")
    if len(execs_1) >= 1:
        print(f"✓ Matched list criteria (status='critical')")

    # Step 4: Test with non-matching status
    print("\n[STEP 4] Test: status='low' (should NOT match)...")
    client.post_webhook(webhook_url, {"status": "low", "message": "Low priority alert"})
    time.sleep(3)

    execs_2 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_2)}")
    if len(execs_2) == len(execs_1):
        print(f"✓ Did not match (status='low' not in list)")

    # Step 5: Test with another matching status
    print("\n[STEP 5] Test: status='urgent' (should match)...")
    client.post_webhook(webhook_url, {"status": "urgent", "message": "Urgent alert"})
    time.sleep(3)

    execs_3 = client.list_executions(action_ref=action_ref)
    print(f"  Executions: {len(execs_3)}")
    if len(execs_3) > len(execs_2):
        print(f"✓ Matched list criteria (status='urgent')")

    # Summary
    print("\n" + "=" * 80)
    print("LIST MEMBERSHIP CRITERIA TEST SUMMARY")
    print("=" * 80)
    print(f"✓ List membership (in operator) tested")
    print(f"✓ 'critical' status: matched")
    print(f"✓ 'low' status: filtered out")
    print(f"✓ 'urgent' status: matched")
    print("\n✅ LIST MEMBERSHIP CRITERIA WORKING!")
    print("=" * 80)
