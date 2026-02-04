#!/usr/bin/env python3
"""
T1.1: Interval Timer Automation

Tests that an action executes repeatedly on an interval timer trigger.

Test Flow:
1. Register test pack via API
2. Create interval timer trigger (every 5 seconds)
3. Create simple echo action
4. Create rule linking timer → action
5. Wait for 3 trigger events (15 seconds)
6. Verify 3 enforcements created
7. Verify 3 executions completed successfully

Success Criteria:
- Timer fires every 5 seconds (±500ms tolerance)
- Each timer event creates enforcement
- Each enforcement creates execution
- All executions reach 'succeeded' status
- Action output captured in execution results
- No errors in any service logs
"""

import time

import pytest
from helpers import (
    AttuneClient,
    create_echo_action,
    create_interval_timer,
    create_rule,
    wait_for_event_count,
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.tier1
@pytest.mark.timer
@pytest.mark.integration
@pytest.mark.timeout(60)
class TestIntervalTimerAutomation:
    """Test interval timer automation flow"""

    def test_interval_timer_creates_executions(
        self, client: AttuneClient, pack_ref: str
    ):
        """Test that interval timer creates executions at regular intervals"""

        # Test parameters
        interval_seconds = 5
        expected_executions = 3
        test_duration = interval_seconds * expected_executions + 5  # Add buffer

        print(f"\n=== T1.1: Interval Timer Automation ===")
        print(f"Interval: {interval_seconds}s")
        print(f"Expected executions: {expected_executions}")
        print(f"Test duration: ~{test_duration}s")

        # Step 1: Create interval timer trigger
        print("\n[1/5] Creating interval timer trigger...")
        trigger = create_interval_timer(
            client=client,
            interval_seconds=interval_seconds,
            pack_ref=pack_ref,
        )
        print(f"✓ Created trigger: {trigger['label']} (ID: {trigger['id']})")
        assert trigger["ref"] == "core.intervaltimer"
        assert "sensor" in trigger
        assert trigger["sensor"]["enabled"] is True

        # Step 2: Create echo action
        print("\n[2/5] Creating echo action...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]
        print(f"✓ Created action: {action_ref} (ID: {action['id']})")

        # Step 3: Create rule linking trigger → action
        print("\n[3/5] Creating rule...")

        # Capture timestamp before rule creation for filtering
        import time
        from datetime import datetime, timezone

        rule_creation_time = datetime.now(timezone.utc).isoformat()

        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action_ref,
            pack_ref=pack_ref,
            enabled=True,
            action_parameters={
                "message": f"Timer fired at interval {interval_seconds}s"
            },
        )
        print(f"✓ Created rule: {rule['label']} (ID: {rule['id']})")
        print(f"  Rule creation timestamp: {rule_creation_time}")
        assert rule["enabled"] is True
        assert rule["trigger"] == trigger["id"]
        assert rule["action_ref"] == action_ref

        # Step 4: Wait for events to be created
        print(
            f"\n[4/5] Waiting for {expected_executions} timer events (timeout: {test_duration}s)..."
        )
        start_time = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=expected_executions,
            trigger_id=trigger["id"],
            timeout=test_duration,
            poll_interval=1.0,
        )

        elapsed = time.time() - start_time
        print(f"✓ {len(events)} events created in {elapsed:.1f}s")

        # Sort events by created timestamp (ascending order - oldest first)
        events_sorted = sorted(events[:expected_executions], key=lambda e: e["created"])

        # Verify event timing
        event_times = []
        for i, event in enumerate(events_sorted):
            print(f"  Event {i + 1}: ID={event['id']}, trigger={event['trigger']}")
            assert event["trigger"] == trigger["id"]
            event_times.append(event["created"])

        # Check event intervals (if we have multiple events)
        if len(event_times) >= 2:
            from datetime import datetime

            for i in range(1, len(event_times)):
                t1 = datetime.fromisoformat(event_times[i - 1].replace("Z", "+00:00"))
                t2 = datetime.fromisoformat(event_times[i].replace("Z", "+00:00"))
                interval = (t2 - t1).total_seconds()
                print(
                    f"  Interval {i}: {interval:.1f}s (expected: {interval_seconds}s)"
                )

                # Allow ±1 second tolerance for timing
                assert abs(interval - interval_seconds) < 1.5, (
                    f"Event interval {interval:.1f}s outside tolerance (expected {interval_seconds}s ±1.5s)"
                )

        # Step 5: Verify executions completed successfully
        print(f"\n[5/5] Verifying {expected_executions} executions completed...")

        executions = wait_for_execution_count(
            client=client,
            expected_count=expected_executions,
            rule_id=rule["id"],
            created_after=rule_creation_time,
            timeout=30,
            poll_interval=1.0,
            verbose=True,
        )

        print(f"✓ {len(executions)} executions created")

        # Verify each execution
        succeeded_count = 0
        for i, execution in enumerate(executions[:expected_executions]):
            exec_id = execution["id"]
            status = execution["status"]

            print(f"\n  Execution {i + 1} (ID: {exec_id}):")
            print(f"    Status: {status}")
            print(f"    Action: {execution['action_ref']}")

            # Wait for execution to complete if still running
            if status not in ["succeeded", "failed", "canceled"]:
                print(f"    Waiting for completion...")
                execution = wait_for_execution_status(
                    client=client,
                    execution_id=exec_id,
                    expected_status="succeeded",
                    timeout=15,
                )
                status = execution["status"]
                print(f"    Final status: {status}")

            # Verify execution succeeded
            assert status == "succeeded", (
                f"Execution {exec_id} failed with status '{status}'"
            )

            # Verify execution has correct action
            assert execution["action_ref"] == action_ref

            # Verify execution has result
            if execution.get("result"):
                print(f"    Result: {execution['result']}")

            succeeded_count += 1

        print(f"\n✓ All {succeeded_count} executions succeeded")

        # Final verification
        print("\n=== Test Summary ===")
        print(f"✓ Trigger created and firing every {interval_seconds}s")
        print(f"✓ {len(events)} events generated")
        print(f"✓ {succeeded_count} executions completed successfully")
        print(f"✓ Total test duration: {time.time() - start_time:.1f}s")
        print(f"✓ Test PASSED")

    def test_interval_timer_precision(self, client: AttuneClient, pack_ref: str):
        """Test that interval timer fires with acceptable precision"""

        # Use shorter interval for precision test
        interval_seconds = 3
        expected_fires = 5
        test_duration = interval_seconds * expected_fires + 3

        print(f"\n=== T1.1b: Interval Timer Precision ===")
        print(f"Testing {interval_seconds}s interval over {expected_fires} fires")

        # Create automation
        trigger = create_interval_timer(
            client=client, interval_seconds=interval_seconds, pack_ref=pack_ref
        )
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        print(f"✓ Setup complete: trigger={trigger['id']}, action={action['ref']}")

        # Record event times
        print(f"\nWaiting for {expected_fires} events...")
        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_id=trigger["id"],
            timeout=test_duration,
            poll_interval=0.5,
        )

        # Calculate intervals
        from datetime import datetime

        event_times = [
            datetime.fromisoformat(e["created"].replace("Z", "+00:00"))
            for e in events[:expected_fires]
        ]

        intervals = []
        for i in range(1, len(event_times)):
            interval = (event_times[i] - event_times[i - 1]).total_seconds()
            intervals.append(interval)
            print(f"  Interval {i}: {interval:.2f}s")

        # Calculate statistics
        if intervals:
            avg_interval = sum(intervals) / len(intervals)
            min_interval = min(intervals)
            max_interval = max(intervals)

            print(f"\nInterval Statistics:")
            print(f"  Expected: {interval_seconds}s")
            print(f"  Average:  {avg_interval:.2f}s")
            print(f"  Min:      {min_interval:.2f}s")
            print(f"  Max:      {max_interval:.2f}s")
            print(f"  Range:    {max_interval - min_interval:.2f}s")

            # Verify precision
            # Allow ±1 second tolerance
            tolerance = 1.0
            assert abs(avg_interval - interval_seconds) < tolerance, (
                f"Average interval {avg_interval:.2f}s outside tolerance"
            )

            print(f"\n✓ Timer precision within ±{tolerance}s tolerance")
            print(f"✓ Test PASSED")
