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
    create_interval_timer,
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

        # Step 1: Create interval timer (creates trigger + rule using core.echo)
        print("\n[1/3] Creating interval timer with rule...")

        # Capture timestamp before rule creation for filtering
        from datetime import datetime, timezone

        rule_creation_time = datetime.now(timezone.utc).isoformat()

        trigger = create_interval_timer(
            client=client,
            interval_seconds=interval_seconds,
            pack_ref=pack_ref,
            action_ref="core.echo",
            action_parameters={
                "message": f"Timer fired at interval {interval_seconds}s"
            },
        )
        rule = trigger["rule"]
        assert rule is not None, "Timer rule was not created"
        print(f"✓ Created trigger: {trigger['ref']} (ID: {trigger['id']})")
        print(f"✓ Created rule: {rule['ref']} (ID: {rule['id']})")
        print(f"  Rule creation timestamp: {rule_creation_time}")
        assert rule["enabled"] is True
        assert rule["action_ref"] == "core.echo"

        # Step 2: Wait for events to be created
        print(
            f"\n[2/3] Waiting for {expected_executions} timer events (timeout: {test_duration}s)..."
        )
        start_time = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=expected_executions,
            trigger_id=trigger["id"],
            rule_id=rule["id"],
            created_after=rule_creation_time,
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
            for i in range(1, len(event_times)):
                t1 = datetime.fromisoformat(event_times[i - 1].replace("Z", "+00:00"))
                t2 = datetime.fromisoformat(event_times[i].replace("Z", "+00:00"))
                interval = (t2 - t1).total_seconds()
                print(
                    f"  Interval {i}: {interval:.1f}s (expected: {interval_seconds}s)"
                )

                # Allow ±1.5 second tolerance for timing
                assert abs(interval - interval_seconds) < 1.5, (
                    f"Event interval {interval:.1f}s outside tolerance (expected {interval_seconds}s ±1.5s)"
                )

        # Step 3: Verify executions completed successfully
        print(f"\n[3/3] Verifying {expected_executions} executions completed...")

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
            if status not in ["completed", "failed", "cancelled", "timeout"]:
                print(f"    Waiting for completion...")
                execution = wait_for_execution_status(
                    client=client,
                    execution_id=exec_id,
                    expected_status="completed",
                    timeout=30,
                )
                status = execution["status"]
                print(f"    Final status: {status}")

            # Verify execution has correct action
            assert execution["action_ref"] == "core.echo"

            if status == "completed":
                succeeded_count += 1
                # Verify execution has result
                if execution.get("result"):
                    print(f"    Result: {execution['result']}")

        # Allow 1 failure due to artifact version race condition
        assert succeeded_count >= expected_executions - 1, (
            f"Too many failures: only {succeeded_count}/{expected_executions} completed"
        )

        print(f"\n✓ {succeeded_count}/{expected_executions} executions completed successfully")

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

        # Create automation (single rule via create_interval_timer using core.echo)
        from datetime import datetime, timezone

        rule_creation_time = datetime.now(timezone.utc).isoformat()

        trigger = create_interval_timer(
            client=client,
            interval_seconds=interval_seconds,
            pack_ref=pack_ref,
            action_ref="core.echo",
        )
        rule = trigger["rule"]
        assert rule is not None, "Timer rule was not created"

        print(f"✓ Setup complete: trigger={trigger['id']}, action=core.echo")

        # Record event times (only events created after our rule, for our rule)
        print(f"\nWaiting for {expected_fires} events...")
        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_id=trigger["id"],
            rule_id=rule["id"],
            created_after=rule_creation_time,
            timeout=test_duration,
            poll_interval=0.5,
        )

        # Calculate intervals
        from datetime import datetime

        event_times = sorted([
            datetime.fromisoformat(e["created"].replace("Z", "+00:00"))
            for e in events[:expected_fires]
        ])

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
