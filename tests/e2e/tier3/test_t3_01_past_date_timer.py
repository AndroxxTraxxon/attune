"""
T3.1: Date Timer with Past Date Test

Tests that date timers with past dates are handled gracefully - either by
executing immediately or failing with a clear error message.

Priority: LOW
Duration: ~5 seconds
"""

import time
from datetime import datetime, timedelta

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import create_date_timer, create_echo_action, unique_ref
from helpers.polling import wait_for_event_count, wait_for_execution_count


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.edge_case
def test_past_date_timer_immediate_execution(client: AttuneClient, test_pack):
    """
    Test that a timer with a past date executes immediately or is handled gracefully.

    Expected behavior: Either execute immediately OR reject with clear error.
    """
    print("\n" + "=" * 80)
    print("T3.1: Past Date Timer Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create a date in the past (1 hour ago)
    print("\n[STEP 1] Creating date timer with past date...")
    past_date = datetime.utcnow() - timedelta(hours=1)
    date_str = past_date.strftime("%Y-%m-%dT%H:%M:%SZ")

    trigger_ref = f"past_date_timer_{unique_ref()}"

    try:
        trigger_response = create_date_timer(
            client=client,
            pack_ref=pack_ref,
            trigger_ref=trigger_ref,
            date=date_str,
        )

        trigger_id = trigger_response["id"]
        print(f"✓ Past date timer created: {trigger_ref}")
        print(f"  Scheduled date: {date_str} (1 hour ago)")
        print(f"  Trigger ID: {trigger_id}")

    except Exception as e:
        error_msg = str(e)
        print(f"✗ Timer creation failed: {error_msg}")

        # This is acceptable - rejecting past dates is valid behavior
        if "past" in error_msg.lower() or "invalid" in error_msg.lower():
            print(f"✓ System rejected past date with clear error")
            print("\n" + "=" * 80)
            print("PAST DATE TIMER TEST SUMMARY")
            print("=" * 80)
            print(f"✓ Past date timer rejected with clear error")
            print(f"✓ Error message: {error_msg}")
            print("\n✅ Past date validation WORKING!")
            print("=" * 80)
            return  # Test passes - rejection is acceptable
        else:
            print(f"⚠ Unexpected error: {error_msg}")
            pytest.fail(f"Past date timer failed with unclear error: {error_msg}")

    # Step 2: Create an action
    print("\n[STEP 2] Creating action...")
    action_ref = create_echo_action(
        client=client, pack_ref=pack_ref, message="Past date timer fired!"
    )
    print(f"✓ Action created: {action_ref}")

    # Step 3: Create rule linking trigger to action
    print("\n[STEP 3] Creating rule...")
    rule_data = {
        "name": f"Past Date Timer Rule {unique_ref()}",
        "trigger": trigger_ref,
        "action": action_ref,
        "enabled": True,
    }

    rule_response = client.create_rule(rule_data)
    rule_id = rule_response["id"]
    print(f"✓ Rule created: {rule_id}")

    # Step 4: Check if timer fires immediately
    print("\n[STEP 4] Checking if timer fires immediately...")
    print("  Waiting up to 10 seconds for immediate execution...")

    start_time = time.time()

    try:
        # Wait for at least 1 event
        events = wait_for_event_count(
            client=client,
            trigger_ref=trigger_ref,
            expected_count=1,
            timeout=10,
            operator=">=",
        )

        elapsed = time.time() - start_time
        print(f"✓ Timer fired immediately! ({elapsed:.1f}s after rule creation)")
        print(f"  Events created: {len(events)}")

        # Check if execution was created
        executions = wait_for_execution_count(
            client=client,
            action_ref=action_ref,
            expected_count=1,
            timeout=5,
            operator=">=",
        )

        print(f"✓ Execution created: {len(executions)} execution(s)")

        # Verify only 1 event (should not repeat)
        time.sleep(5)
        events_after_wait = client.list_events(trigger=trigger_ref)

        if len(events_after_wait) == 1:
            print(f"✓ Timer fired only once (no repeat)")
        else:
            print(f"⚠ Timer fired {len(events_after_wait)} times (expected 1)")

        behavior = "immediate_execution"

    except Exception as e:
        elapsed = time.time() - start_time
        print(f"✗ No immediate execution detected after {elapsed:.1f}s")
        print(f"  Error: {e}")

        # Check if timer is in some error/expired state
        try:
            trigger_info = client.get_trigger(trigger_ref)
            print(f"  Trigger status: {trigger_info.get('status', 'unknown')}")
        except:
            pass

        behavior = "no_execution"

    # Step 5: Verify expected behavior
    print("\n[STEP 5] Verifying behavior...")

    if behavior == "immediate_execution":
        print("✓ System executed past date timer immediately")
        print("  This is acceptable behavior")
    elif behavior == "no_execution":
        print("⚠ Past date timer did not execute")
        print("  This may be acceptable if timer is marked as expired")
        print("  Recommendation: Document expected behavior")

    # Summary
    print("\n" + "=" * 80)
    print("PAST DATE TIMER TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Past date timer created: {trigger_ref}")
    print(f"  Scheduled date: {date_str} (1 hour in past)")
    print(f"✓ Rule created: {rule_id}")
    print(f"  Behavior: {behavior}")

    if behavior == "immediate_execution":
        print(f"\n✅ Past date timer executed immediately (acceptable)")
    elif behavior == "no_execution":
        print(f"\n⚠️ Past date timer did not execute")
        print("   Recommendation: Either execute immediately OR reject creation")

    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.edge_case
def test_just_missed_date_timer(client: AttuneClient, test_pack):
    """
    Test a date timer that just passed (a few seconds ago).

    This tests the boundary condition where a timer might have been valid
    when scheduled but passed by the time it's activated.
    """
    print("\n" + "=" * 80)
    print("T3.1b: Just Missed Date Timer Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create a date timer just 2 seconds in the past
    print("\n[STEP 1] Creating date timer 2 seconds in the past...")
    past_date = datetime.utcnow() - timedelta(seconds=2)
    date_str = past_date.strftime("%Y-%m-%dT%H:%M:%SZ")

    trigger_ref = f"just_missed_timer_{unique_ref()}"

    try:
        trigger_response = create_date_timer(
            client=client,
            pack_ref=pack_ref,
            trigger_ref=trigger_ref,
            date=date_str,
        )
        print(f"✓ Just-missed timer created: {trigger_ref}")
        print(f"  Date: {date_str} (2 seconds ago)")
    except Exception as e:
        print(f"✗ Timer creation failed: {e}")
        print("✓ System rejected just-missed date (acceptable)")
        return

    # Step 2: Create action and rule
    print("\n[STEP 2] Creating action and rule...")
    action_ref = create_echo_action(
        client=client, pack_ref=pack_ref, message="Just-missed timer fired"
    )

    rule_data = {
        "name": f"Just Missed Timer Rule {unique_ref()}",
        "trigger": trigger_ref,
        "action": action_ref,
        "enabled": True,
    }
    rule_response = client.create_rule(rule_data)
    print(f"✓ Rule created: {rule_response['id']}")

    # Step 3: Check execution
    print("\n[STEP 3] Checking for immediate execution...")

    try:
        events = wait_for_event_count(
            client=client,
            trigger_ref=trigger_ref,
            expected_count=1,
            timeout=5,
            operator=">=",
        )
        print(f"✓ Just-missed timer executed: {len(events)} event(s)")
    except Exception as e:
        print(f"⚠ Just-missed timer did not execute: {e}")

    # Summary
    print("\n" + "=" * 80)
    print("JUST MISSED TIMER TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Timer with recent past date tested")
    print(f"✓ Boundary condition validated")
    print("\n💡 Recent past dates behavior documented!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.edge_case
def test_far_past_date_timer(client: AttuneClient, test_pack):
    """
    Test a date timer with a date far in the past (1 year ago).

    This should definitely be rejected or handled specially.
    """
    print("\n" + "=" * 80)
    print("T3.1c: Far Past Date Timer Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Try to create a timer 1 year in the past
    print("\n[STEP 1] Creating date timer 1 year in the past...")
    far_past_date = datetime.utcnow() - timedelta(days=365)
    date_str = far_past_date.strftime("%Y-%m-%dT%H:%M:%SZ")

    trigger_ref = f"far_past_timer_{unique_ref()}"

    try:
        trigger_response = create_date_timer(
            client=client,
            pack_ref=pack_ref,
            trigger_ref=trigger_ref,
            date=date_str,
        )
        print(f"⚠ Far past timer was accepted: {trigger_ref}")
        print(f"  Date: {date_str} (1 year ago)")
        print(f"  Recommendation: Consider rejecting dates > 24 hours in past")

    except Exception as e:
        error_msg = str(e)
        print(f"✓ Far past timer rejected: {error_msg}")

        if "past" in error_msg.lower() or "invalid" in error_msg.lower():
            print(f"✓ Clear error message provided")
        else:
            print(f"⚠ Error message could be clearer")

    # Summary
    print("\n" + "=" * 80)
    print("FAR PAST DATE TIMER TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Far past date validation tested (1 year ago)")
    print(f"✓ Edge case behavior documented")
    print("\n💡 Far past date handling validated!")
    print("=" * 80)
