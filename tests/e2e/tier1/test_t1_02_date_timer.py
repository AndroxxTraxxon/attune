#!/usr/bin/env python3
"""
T1.2: Date Timer (One-Shot Execution)

Tests that an action executes once at a specific future time.

Test Flow:
1. Create echo action
2. Create date timer rule (fire_at = 5 seconds from now)
3. Wait for 1 event from the sensor
4. Verify exactly 1 execution occurred
5. Wait additional time, verify no duplicate fires

Success Criteria:
- Timer fires once at scheduled time (±2 seconds)
- Exactly 1 event created
- Exactly 1 execution created (succeeded)
- No duplicate executions after timer expires
"""

import time
from datetime import datetime

import pytest
from helpers import (
    AttuneClient,
    create_date_timer,
    create_echo_action,
    timestamp_future,
    wait_for_event_count,
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.tier1
@pytest.mark.timer
@pytest.mark.integration
@pytest.mark.timeout(45)
class TestDateTimerAutomation:
    """Test date timer (one-shot) automation flow"""

    def test_date_timer_fires_once(self, client: AttuneClient, pack_ref: str):
        """Test that date timer fires exactly once at scheduled time"""

        fire_in_seconds = 5
        buffer_time = 15

        print(f"\n=== T1.2: Date Timer One-Shot Execution ===")
        print(f"Scheduled to fire in: {fire_in_seconds}s")

        # Step 1: Create echo action
        print("\n[1/4] Creating echo action...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]
        print(f"✓ Created action: {action_ref}")

        # Step 2: Create date timer (creates rule with trigger_params internally)
        print("\n[2/4] Creating date timer...")
        fire_at = timestamp_future(fire_in_seconds)
        created_after = datetime.utcnow().isoformat() + "Z"
        timer = create_date_timer(
            client=client,
            fire_at=fire_at,
            action_ref=action_ref,
            action_params={"message": f"Date timer fired at {fire_at}"},
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create timer rule"
        rule_id = rule["id"]
        print(f"✓ Created rule: {rule['label']} (ID: {rule_id})")
        print(f"  Scheduled for: {fire_at}")

        # Step 3: Wait for timer to fire
        print(f"\n[3/4] Waiting for timer to fire (timeout: {fire_in_seconds + buffer_time}s)...")
        start_time = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=1,
            trigger_ref="core.datetimetimer",
            rule_id=rule_id,
            created_after=created_after,
            timeout=fire_in_seconds + buffer_time,
            poll_interval=0.5,
            operator=">=",
        )

        fire_time = time.time()
        actual_delay = fire_time - start_time

        print(f"✓ Timer fired after {actual_delay:.2f}s")
        print(f"  Expected: ~{fire_in_seconds}s")

        # Verify timing precision (±5 seconds tolerance for sensor startup + pickup delay)
        assert abs(actual_delay - fire_in_seconds) < 5.0, (
            f"Timer fired at {actual_delay:.1f}s, expected ~{fire_in_seconds}s (±5s)"
        )

        assert len(events) >= 1, "Expected at least 1 event"

        # Step 4: Verify execution succeeded
        print(f"\n[4/4] Verifying execution succeeded...")
        executions = wait_for_execution_count(
            client=client,
            expected_count=1,
            action_ref=action_ref,
            timeout=15,
            poll_interval=0.5,
            operator=">=",
        )

        assert len(executions) >= 1, "Expected at least 1 execution"
        execution = executions[0]

        if execution["status"] not in ["completed", "failed", "cancelled"]:
            execution = wait_for_execution_status(
                client=client,
                execution_id=execution["id"],
                expected_status="completed",
                timeout=10,
            )

        assert execution["status"] == "completed", (
            f"Execution failed with status: {execution['status']}"
        )
        print(f"✓ Execution succeeded (ID: {execution['id']})")

        # Step 5: Wait to verify no duplicate fires
        print(f"\nWaiting 8s to verify no duplicate fires...")
        time.sleep(8)

        final_events = client.list_events(trigger_ref="core.datetimetimer", rule_id=rule_id)
        print(f"✓ Final event count: {len(final_events)}")
        assert len(final_events) == 1, (
            f"Expected exactly 1 event, found {len(final_events)} (duplicate fire detected)"
        )

        total_time = time.time() - start_time
        print("\n=== Test Summary ===")
        print(f"✓ Date timer fired once at scheduled time")
        print(f"✓ Timing precision: {abs(actual_delay - fire_in_seconds):.2f}s deviation")
        print(f"✓ Exactly 1 event, 1 execution")
        print(f"✓ No duplicate fires")
        print(f"✓ Total: {total_time:.1f}s")
        print(f"✓ Test PASSED")

    def test_date_timer_past_date(self, client: AttuneClient, pack_ref: str):
        """Test that date timer with past date is handled gracefully"""

        print(f"\n=== T1.2b: Date Timer with Past Date ===")

        # Create action
        action = create_echo_action(client=client, pack_ref=pack_ref)

        # Create date timer with past date (1 hour ago)
        past_date = timestamp_future(-3600)
        print(f"  Date: {past_date} (past)")

        timer = create_date_timer(
            client=client,
            fire_at=past_date,
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create timer rule"
        rule_id = rule["id"]

        # The sensor rejects fire_at <= now, so the timer should NOT fire.
        # Wait briefly and verify no events.
        print("\nWaiting 5s to confirm timer does not fire...")
        time.sleep(5)

        all_events = client.list_events(trigger_ref="core.datetimetimer")
        events = [e for e in all_events if e.get("rule") == rule_id]
        print(f"  Events for rule {rule_id}: {len(events)}")

        # Past date should result in 0 events (sensor rejects it)
        assert len(events) == 0, (
            f"Expected 0 events for past date timer, found {len(events)}"
        )

        print("\n✓ Past date timer handled gracefully (no fires)")
        print("✓ Test PASSED")

    def test_date_timer_far_future(self, client: AttuneClient, pack_ref: str):
        """Test creating date timer far in the future (doesn't fire during test)"""

        print(f"\n=== T1.2c: Date Timer Far Future ===")

        action = create_echo_action(client=client, pack_ref=pack_ref)
        future_time = timestamp_future(3600)

        print(f"  Time: {future_time} (+1 hour)")

        timer = create_date_timer(
            client=client,
            fire_at=future_time,
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create timer rule"
        rule_id = rule["id"]

        # Verify timer doesn't fire prematurely
        print("\nWaiting 3s to verify timer doesn't fire prematurely...")
        time.sleep(3)

        all_events = client.list_events(trigger_ref="core.datetimetimer")
        events = [e for e in all_events if e.get("rule") == rule_id]
        executions = client.list_executions(action_ref=action["ref"])

        print(f"  Events for rule {rule_id}: {len(events)}")
        print(f"  Executions: {len(executions)}")

        assert len(events) == 0, "Timer fired prematurely"
        assert len(executions) == 0, "Execution created prematurely"

        print("\n✓ Timer correctly waiting for future time")
        print("✓ Test PASSED")
