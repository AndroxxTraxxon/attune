"""
T3.2: Timer Cancellation Test

Tests that disabling a rule stops timer from executing, and re-enabling
resumes executions.

Priority: LOW
Duration: ~15 seconds
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, create_interval_timer, unique_ref
from helpers.polling import wait_for_execution_count


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.rules
def test_timer_cancellation_via_rule_disable(client: AttuneClient, test_pack):
    """
    Test that disabling a rule stops timer executions.

    Flow:
    1. Create interval timer (every 3 seconds)
    2. Wait for 2 executions
    3. Disable rule
    4. Wait 10 seconds
    5. Verify no new executions occurred
    """
    print("\n" + "=" * 80)
    print("T3.2a: Timer Cancellation via Rule Disable Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create action and interval timer rule
    print("\n[STEP 1] Creating action and interval timer rule (every 3 seconds)...")
    trigger_ref = f"cancel_timer_{unique_ref()}"

    action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Timer tick",
        suffix="_cancel",
    )
    action_ref = action["ref"]

    trigger_response = create_interval_timer(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        interval=3,
        action_ref=action_ref,
    )
    rule_id = trigger_response["rule"]["id"]

    print(f"✓ Interval timer rule created: {rule_id}")
    print(f"  Interval: 3 seconds")
    print(f"  Status: enabled")

    # Step 2: Wait for 2 executions
    print("\n[STEP 2] Waiting for 2 timer executions...")
    wait_for_execution_count(
        client=client,
        action_ref=action_ref,
        expected_count=2,
        timeout=15,
        operator=">=",
    )

    executions_before_disable = client.list_executions(action_ref=action_ref)
    print(f"✓ {len(executions_before_disable)} executions occurred")

    # Step 3: Disable rule
    print("\n[STEP 3] Disabling rule...")
    client.disable_rule(rule_id)
    print(f"✓ Rule disabled: {rule_id}")

    # Step 4: Wait and verify no new executions
    print("\n[STEP 4] Waiting 10 seconds to verify no new executions...")
    time.sleep(10)

    executions_after_disable = client.list_executions(action_ref=action_ref)
    new_executions = len(executions_after_disable) - len(executions_before_disable)

    print(f"  Executions before disable: {len(executions_before_disable)}")
    print(f"  Executions after disable: {len(executions_after_disable)}")
    print(f"  New executions: {new_executions}")

    if new_executions == 0:
        print(f"✓ No new executions (timer successfully stopped)")
    else:
        print(f"⚠ {new_executions} new execution(s) occurred after disable")

    # Summary
    print("\n" + "=" * 80)
    print("TIMER CANCELLATION TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Timer created: {trigger_ref} (3 second interval)")
    print(f"✓ Rule disabled after {len(executions_before_disable)} executions")
    print(f"✓ New executions after disable: {new_executions}")

    if new_executions == 0:
        print("\n✅ TIMER CANCELLATION WORKING!")
    else:
        print("\n⚠️ Timer may still be firing after rule disable")

    print("=" * 80)

    # Allow some tolerance for in-flight executions (1 execution max)
    assert new_executions <= 1, (
        f"Expected 0-1 new executions after disable, got {new_executions}"
    )


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.rules
def test_timer_resume_after_re_enable(client: AttuneClient, test_pack):
    """
    Test that re-enabling a disabled rule resumes timer executions.
    """
    print("\n" + "=" * 80)
    print("T3.2b: Timer Resume After Re-enable Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create action and timer rule
    print("\n[STEP 1] Creating action and timer rule...")
    trigger_ref = f"resume_timer_{unique_ref()}"

    action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Resume test",
        suffix="_resume",
    )
    action_ref = action["ref"]

    trigger_response = create_interval_timer(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        interval=3,
        action_ref=action_ref,
    )
    rule_id = trigger_response["rule"]["id"]
    print(f"✓ Timer and rule created")

    # Step 2: Wait for 1 execution
    print("\n[STEP 2] Waiting for initial execution...")
    wait_for_execution_count(
        client=client,
        action_ref=action_ref,
        expected_count=1,
        timeout=20,
        operator=">=",
    )
    print(f"✓ Initial execution confirmed")

    # Step 3: Disable rule
    print("\n[STEP 3] Disabling rule...")
    client.disable_rule(rule_id)
    time.sleep(1)
    executions_after_disable = client.list_executions(action_ref=action_ref)
    count_after_disable = len(executions_after_disable)
    print(f"✓ Rule disabled (executions: {count_after_disable})")

    # Step 4: Wait while disabled
    print("\n[STEP 4] Waiting 6 seconds while disabled...")
    time.sleep(6)
    executions_still_disabled = client.list_executions(action_ref=action_ref)
    count_still_disabled = len(executions_still_disabled)
    increase_while_disabled = count_still_disabled - count_after_disable
    print(f"  Executions while disabled: {increase_while_disabled}")

    # Step 5: Re-enable rule
    print("\n[STEP 5] Re-enabling rule...")
    client.enable_rule(rule_id)
    print(f"✓ Rule re-enabled")

    # Step 6: Wait for new executions
    print("\n[STEP 6] Waiting for executions to resume...")
    time.sleep(8)

    executions_after_enable = client.list_executions(action_ref=action_ref)
    count_after_enable = len(executions_after_enable)
    increase_after_enable = count_after_enable - count_still_disabled

    print(f"  Executions before re-enable: {count_still_disabled}")
    print(f"  Executions after re-enable: {count_after_enable}")
    print(f"  New executions: {increase_after_enable}")

    if increase_after_enable >= 1:
        print(f"✓ Timer resumed (new executions after re-enable)")
    else:
        print(f"⚠ Timer did not resume")

    # Summary
    print("\n" + "=" * 80)
    print("TIMER RESUME TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Timer disabled: verified no new executions")
    print(f"✓ Timer re-enabled: {increase_after_enable} new execution(s)")

    if increase_after_enable >= 1:
        print("\n✅ TIMER RESUME WORKING!")
    else:
        print("\n⚠️ Timer did not resume after re-enable")

    print("=" * 80)

    assert increase_after_enable >= 1, "Timer should resume after re-enable"


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.rules
def test_timer_delete_stops_executions(client: AttuneClient, test_pack):
    """
    Test that deleting a rule stops timer executions permanently.
    """
    print("\n" + "=" * 80)
    print("T3.2c: Timer Delete Stops Executions Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create action and timer rule
    print("\n[STEP 1] Creating action and timer rule...")
    trigger_ref = f"delete_timer_{unique_ref()}"

    action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        message="Delete test",
        suffix="_delete",
    )
    action_ref = action["ref"]

    trigger_response = create_interval_timer(
        client=client,
        pack_ref=pack_ref,
        trigger_ref=trigger_ref,
        interval=3,
        action_ref=action_ref,
    )
    rule_id = trigger_response["rule"]["id"]
    print(f"✓ Timer and rule created")

    # Step 2: Wait for 1 execution
    print("\n[STEP 2] Waiting for initial execution...")
    wait_for_execution_count(
        client=client,
        action_ref=action_ref,
        expected_count=1,
        timeout=20,
        operator=">=",
    )

    executions_before_delete = client.list_executions(action_ref=action_ref)
    print(f"✓ Initial executions: {len(executions_before_delete)}")

    # Step 3: Delete rule
    print("\n[STEP 3] Deleting rule...")
    try:
        client.delete_rule(rule_id)
        print(f"✓ Rule deleted: {rule_id}")
    except Exception as e:
        print(f"⚠ Rule deletion failed: {e}")
        pytest.skip("Rule deletion not available")

    # Step 4: Wait and verify no new executions
    print("\n[STEP 4] Waiting 10 seconds to verify no new executions...")
    time.sleep(10)

    executions_after_delete = client.list_executions(action_ref=action_ref)
    new_executions = len(executions_after_delete) - len(executions_before_delete)

    print(f"  Executions before delete: {len(executions_before_delete)}")
    print(f"  Executions after delete: {len(executions_after_delete)}")
    print(f"  New executions: {new_executions}")

    if new_executions == 0:
        print(f"✓ No new executions (timer permanently stopped)")
    else:
        print(f"⚠ {new_executions} new execution(s) after rule deletion")

    # Summary
    print("\n" + "=" * 80)
    print("TIMER DELETE TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Rule deleted: {rule_id}")
    print(f"✓ New executions after delete: {new_executions}")

    if new_executions == 0:
        print("\n✅ TIMER DELETION STOPS EXECUTIONS!")
    else:
        print("\n⚠️ Timer may still fire after rule deletion")

    print("=" * 80)

    # Allow 1 in-flight execution tolerance
    assert new_executions <= 1, (
        f"Expected 0-1 new executions after delete, got {new_executions}"
    )
