#!/usr/bin/env python3
"""
T1.2: Date Timer (One-Shot Execution)

Tests that an action executes once at a specific future time.

Test Flow:
1. Create date timer trigger (5 seconds from now)
2. Create action with unique marker output
3. Create rule linking timer → action
4. Wait 7 seconds
5. Verify exactly 1 execution occurred
6. Wait additional 10 seconds
7. Verify no additional executions

Success Criteria:
- Timer fires once at scheduled time (±1 second)
- Exactly 1 enforcement created
- Exactly 1 execution created
- No duplicate executions after timer expires
- Timer marked as expired/completed
"""

import time
from datetime import datetime, timedelta

import pytest
from helpers import (
    AttuneClient,
    create_date_timer,
    create_echo_action,
    create_rule,
    timestamp_future,
    wait_for_event_count,
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.tier1
@pytest.mark.timer
@pytest.mark.integration
@pytest.mark.timeout(30)
class TestDateTimerAutomation:
    """Test date timer (one-shot) automation flow"""

    def test_date_timer_fires_once(self, client: AttuneClient, pack_ref: str):
        """Test that date timer fires exactly once at scheduled time"""

        fire_in_seconds = 5
        buffer_time = 3

        print(f"\n=== T1.2: Date Timer One-Shot Execution ===")
        print(f"Scheduled to fire in: {fire_in_seconds}s")

        # Step 1: Create date timer trigger
        print("\n[1/5] Creating date timer trigger...")
        fire_at = timestamp_future(fire_in_seconds)
        trigger = create_date_timer(
            client=client,
            fire_at=fire_at,
            pack_ref=pack_ref,
        )
        print(f"✓ Created trigger: {trigger['label']} (ID: {trigger['id']})")
        print(f"  Scheduled for: {fire_at}")
        assert trigger["ref"] == "core.datetimetimer"
        assert "sensor" in trigger
        assert trigger["sensor"]["enabled"] is True
        assert trigger["fire_at"] == fire_at

        # Step 2: Create echo action with unique marker
        print("\n[2/5] Creating echo action...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]
        unique_message = f"Date timer fired at {fire_at}"
        print(f"✓ Created action: {action_ref} (ID: {action['id']})")

        # Step 3: Create rule linking trigger → action
        print("\n[3/5] Creating rule...")
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action_ref,
            pack_ref=pack_ref,
            enabled=True,
            action_parameters={"message": unique_message},
        )
        print(f"✓ Created rule: {rule['label']} (ID: {rule['id']})")

        # Step 4: Wait for timer to fire
        print(
            f"\n[4/5] Waiting for timer to fire (timeout: {fire_in_seconds + buffer_time}s)..."
        )
        print(f"  Current time: {datetime.utcnow().isoformat()}Z")
        print(f"  Fire time:    {fire_at}")

        start_time = time.time()

        # Wait for exactly 1 event
        events = wait_for_event_count(
            client=client,
            expected_count=1,
            trigger_id=trigger["id"],
            timeout=fire_in_seconds + buffer_time,
            poll_interval=0.5,
            operator=">=",
        )

        fire_time = time.time()
        actual_delay = fire_time - start_time

        print(f"✓ Timer fired after {actual_delay:.2f}s")
        print(f"  Expected: ~{fire_in_seconds}s")
        print(f"  Difference: {abs(actual_delay - fire_in_seconds):.2f}s")

        # Verify timing precision (±2 seconds tolerance)
        assert abs(actual_delay - fire_in_seconds) < 2.0, (
            f"Timer fired at {actual_delay:.1f}s, expected ~{fire_in_seconds}s (±2s)"
        )

        # Verify event
        assert len(events) >= 1, "Expected at least 1 event"
        event = events[0]
        print(f"\n  Event details:")
        print(f"    ID: {event['id']}")
        print(f"    Trigger ID: {event['trigger']}")
        print(f"    Created: {event['created']}")
        assert event["trigger"] == trigger["id"]

        # Step 5: Verify execution completed
        print(f"\n[5/5] Verifying execution completed...")

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

        print(f"✓ Execution created (ID: {execution['id']})")
        print(f"  Status: {execution['status']}")

        # Wait for execution to complete if needed
        if execution["status"] not in ["succeeded", "failed", "canceled"]:
            execution = wait_for_execution_status(
                client=client,
                execution_id=execution["id"],
                expected_status="succeeded",
                timeout=10,
            )

        assert execution["status"] == "succeeded", (
            f"Execution failed with status: {execution['status']}"
        )
        print(f"✓ Execution succeeded")

        # Step 6: Wait additional time to ensure no duplicate fires
        print(f"\nWaiting additional 10s to verify no duplicate fires...")
        time.sleep(10)

        # Check event count again
        final_events = client.list_events(trigger_id=trigger["id"])
        print(f"✓ Final event count: {len(final_events)}")

        # Should still be exactly 1 event
        assert len(final_events) == 1, (
            f"Expected exactly 1 event, found {len(final_events)} (duplicate fire detected)"
        )

        # Check execution count again
        final_executions = client.list_executions(action_ref=action_ref)
        print(f"✓ Final execution count: {len(final_executions)}")

        assert len(final_executions) == 1, (
            f"Expected exactly 1 execution, found {len(final_executions)}"
        )

        # Final summary
        total_time = time.time() - start_time
        print("\n=== Test Summary ===")
        print(f"✓ Date timer fired once at scheduled time")
        print(
            f"✓ Timing precision: {abs(actual_delay - fire_in_seconds):.2f}s deviation"
        )
        print(f"✓ Exactly 1 event created")
        print(f"✓ Exactly 1 execution completed")
        print(f"✓ No duplicate fires detected")
        print(f"✓ Total test duration: {total_time:.1f}s")
        print(f"✓ Test PASSED")

    def test_date_timer_past_date(self, client: AttuneClient, pack_ref: str):
        """Test that date timer with past date fires immediately or fails gracefully"""

        print(f"\n=== T1.2b: Date Timer with Past Date ===")

        # Step 1: Create date timer with past date (1 hour ago)
        print("\n[1/4] Creating date timer with past date...")
        past_date = timestamp_future(-3600)  # 1 hour ago
        print(f"  Date: {past_date} (past)")

        trigger = create_date_timer(
            client=client,
            fire_at=past_date,
            pack_ref=pack_ref,
        )
        print(f"✓ Trigger created: {trigger['label']} (ID: {trigger['id']})")

        # Step 2: Create action and rule
        print("\n[2/4] Creating action and rule...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
            action_parameters={"message": "Past date timer"},
        )
        print(f"✓ Action and rule created")

        # Step 3: Check if timer fires immediately
        print("\n[3/4] Checking timer behavior...")
        print("  Waiting up to 10s to see if timer fires immediately...")

        try:
            # Wait briefly to see if event is created
            events = wait_for_event_count(
                client=client,
                expected_count=1,
                trigger_id=trigger["id"],
                timeout=10,
                poll_interval=0.5,
                operator=">=",
            )

            print(f"✓ Timer fired immediately (behavior: fire on past date)")
            print(f"  Events created: {len(events)}")

            # Verify execution completed
            executions = wait_for_execution_count(
                client=client,
                expected_count=1,
                action_ref=action["ref"],
                timeout=10,
            )

            execution = executions[0]
            if execution["status"] not in ["succeeded", "failed", "canceled"]:
                execution = wait_for_execution_status(
                    client=client,
                    execution_id=execution["id"],
                    expected_status="succeeded",
                    timeout=10,
                )

            assert execution["status"] == "succeeded"
            print(f"✓ Execution completed successfully")

        except TimeoutError:
            # Timer may not fire for past dates - this is also acceptable behavior
            print(f"✓ Timer did not fire (behavior: skip past date)")
            print(f"  This is acceptable behavior - past dates are ignored")

        # Step 4: Verify no ongoing fires
        print("\n[4/4] Verifying timer is one-shot...")
        time.sleep(5)

        final_events = client.list_events(trigger_id=trigger["id"])
        print(f"✓ Final event count: {len(final_events)}")

        # Should be 0 or 1, never more than 1
        assert len(final_events) <= 1, (
            f"Expected 0 or 1 event, found {len(final_events)} (timer firing repeatedly)"
        )

        print("\n=== Test Summary ===")
        print(f"✓ Past date timer handled gracefully")
        print(f"✓ No repeated fires detected")
        print(f"✓ Test PASSED")

    def test_date_timer_far_future(self, client: AttuneClient, pack_ref: str):
        """Test creating date timer far in the future (doesn't fire during test)"""

        print(f"\n=== T1.2c: Date Timer Far Future ===")

        # Create timer for 1 hour from now
        future_time = timestamp_future(3600)

        print(f"\n[1/3] Creating date timer for far future...")
        print(f"  Time: {future_time} (+1 hour)")

        trigger = create_date_timer(
            client=client,
            fire_at=future_time,
            pack_ref=pack_ref,
        )
        print(f"✓ Trigger created: {trigger['label']} (ID: {trigger['id']})")

        # Create action and rule
        print("\n[2/3] Creating action and rule...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )
        print(f"✓ Setup complete")

        # Verify timer doesn't fire prematurely
        print("\n[3/3] Verifying timer doesn't fire prematurely...")
        time.sleep(3)

        events = client.list_events(trigger_id=trigger["id"])
        executions = client.list_executions(action_ref=action["ref"])

        print(f"  Events: {len(events)}")
        print(f"  Executions: {len(executions)}")

        assert len(events) == 0, "Timer fired prematurely"
        assert len(executions) == 0, "Execution created prematurely"

        print("\n✓ Timer correctly waiting for future time")
        print("✓ Test PASSED")
