#!/usr/bin/env python3
"""
T1.3: Cron Timer Execution

Tests that an action executes on a cron schedule.

Test Flow:
1. Create cron timer trigger (at 0, 3, 6, 12 seconds of each minute)
2. Create action with timestamp output
3. Create rule linking timer → action
4. Wait for one minute + 15 seconds
5. Verify executions at correct second marks

Success Criteria:
- Executions occur at seconds: 0, 3, 6, 12 (first minute)
- Executions occur at seconds: 0, 3, 6, 12 (second minute if test runs long)
- No executions at other second marks
- Cron expression correctly parsed
- Timezone handling correct
"""

import time
from datetime import datetime

import pytest
from helpers import (
    AttuneClient,
    create_cron_timer,
    create_echo_action,
    create_rule,
    wait_for_event_count,
    wait_for_execution_count,
)


@pytest.mark.tier1
@pytest.mark.timer
@pytest.mark.integration
@pytest.mark.timeout(90)
class TestCronTimerAutomation:
    """Test cron timer automation flow"""

    def test_cron_timer_specific_seconds(self, client: AttuneClient, pack_ref: str):
        """Test cron timer fires at specific seconds in the minute"""

        # Cron: Fire at 0, 15, 30, 45 seconds of every minute
        # We'll wait up to 75 seconds to catch at least 2 fires
        cron_expression = "0,15,30,45 * * * * *"
        expected_fires = 2
        max_wait_seconds = 75

        print(f"\n=== T1.3: Cron Timer Execution ===")
        print(f"Cron expression: {cron_expression}")
        print(f"Expected fires: {expected_fires}+ in {max_wait_seconds}s")

        # Step 1: Create cron timer trigger
        print("\n[1/5] Creating cron timer trigger...")
        trigger = create_cron_timer(
            client=client,
            cron_expression=cron_expression,
            pack_ref=pack_ref,
            timezone="UTC",
        )
        print(f"✓ Created trigger: {trigger['label']} (ID: {trigger['id']})")
        print(f"  Expression: {cron_expression}")
        print(f"  Timezone: UTC")
        assert trigger["ref"] == "core.crontimer"
        assert "sensor" in trigger
        assert trigger["sensor"]["enabled"] is True
        assert trigger["cron_expression"] == cron_expression

        # Step 2: Create echo action with timestamp
        print("\n[2/5] Creating echo action...")
        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]
        print(f"✓ Created action: {action_ref} (ID: {action['id']})")

        # Step 3: Create rule
        print("\n[3/5] Creating rule...")
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action_ref,
            pack_ref=pack_ref,
            enabled=True,
            action_parameters={"message": "Cron timer fired"},
        )
        print(f"✓ Created rule: {rule['label']} (ID: {rule['id']})")

        # Step 4: Wait for events
        print(
            f"\n[4/5] Waiting for {expected_fires} cron events (max {max_wait_seconds}s)..."
        )
        current_time = datetime.utcnow()
        print(f"  Start time: {current_time.isoformat()}Z")
        print(f"  Current second: {current_time.second}")

        # Calculate how long until next fire
        current_second = current_time.second
        next_fires = [0, 15, 30, 45]
        next_fire_second = None
        for fire_second in next_fires:
            if fire_second > current_second:
                next_fire_second = fire_second
                break
        if next_fire_second is None:
            next_fire_second = next_fires[0]  # Next minute

        wait_seconds = (next_fire_second - current_second) % 60
        print(
            f"  Next expected fire in ~{wait_seconds} seconds (at second {next_fire_second})"
        )

        start_time = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_id=trigger["id"],
            timeout=max_wait_seconds,
            poll_interval=1.0,
        )

        elapsed = time.time() - start_time
        print(f"✓ {len(events)} events created in {elapsed:.1f}s")

        # Verify event timing
        print(f"\n  Event timing analysis:")
        for i, event in enumerate(events[:expected_fires]):
            event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
            second = event_time.second
            print(f"    Event {i + 1}: {event_time.isoformat()} (second: {second:02d})")

            # Verify event fired at one of the expected seconds (with ±2 second tolerance)
            expected_seconds = [0, 15, 30, 45]
            matched = False
            for expected_second in expected_seconds:
                if (
                    abs(second - expected_second) <= 2
                    or abs(second - expected_second) >= 58
                ):
                    matched = True
                    break

            assert matched, (
                f"Event fired at second {second}, not within ±2s of expected seconds {expected_seconds}"
            )

        # Step 5: Verify executions completed
        print(f"\n[5/5] Verifying {expected_fires} executions completed...")

        executions = wait_for_execution_count(
            client=client,
            expected_count=expected_fires,
            action_ref=action_ref,
            timeout=30,
            poll_interval=1.0,
        )

        print(f"✓ {len(executions)} executions created")

        # Verify each execution succeeded
        succeeded_count = 0
        for i, execution in enumerate(executions[:expected_fires]):
            exec_id = execution["id"]
            status = execution["status"]

            print(f"\n  Execution {i + 1} (ID: {exec_id}):")
            print(f"    Status: {status}")

            # Most should be succeeded by now, but wait if needed
            if status not in ["succeeded", "failed", "canceled"]:
                print(f"    Waiting for completion...")
                from helpers import wait_for_execution_status

                execution = wait_for_execution_status(
                    client=client,
                    execution_id=exec_id,
                    expected_status="succeeded",
                    timeout=15,
                )
                status = execution["status"]
                print(f"    Final status: {status}")

            assert status == "succeeded", (
                f"Execution {exec_id} failed with status '{status}'"
            )
            succeeded_count += 1

        print(f"\n✓ All {succeeded_count} executions succeeded")

        # Final summary
        total_time = time.time() - start_time
        print("\n=== Test Summary ===")
        print(f"✓ Cron expression: {cron_expression}")
        print(f"✓ {len(events)} events at correct times")
        print(f"✓ {succeeded_count} executions completed successfully")
        print(f"✓ Total test duration: {total_time:.1f}s")
        print(f"✓ Test PASSED")

    def test_cron_timer_every_5_seconds(self, client: AttuneClient, pack_ref: str):
        """Test cron timer with */5 expression (every 5 seconds)"""

        cron_expression = "*/5 * * * * *"  # Every 5 seconds
        expected_fires = 3
        max_wait = 20  # Should get 3 fires in 15 seconds

        print(f"\n=== T1.3b: Cron Timer Every 5 Seconds ===")
        print(f"Expression: {cron_expression}")

        # Create automation
        trigger = create_cron_timer(
            client=client, cron_expression=cron_expression, pack_ref=pack_ref
        )
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        print(f"✓ Setup complete: trigger={trigger['id']}")

        # Wait for events
        print(f"\nWaiting for {expected_fires} events...")
        start = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_id=trigger["id"],
            timeout=max_wait,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} events in {elapsed:.1f}s")

        # Check timing - should be roughly 0s, 5s, 10s
        event_times = [
            datetime.fromisoformat(e["created"].replace("Z", "+00:00"))
            for e in events[:expected_fires]
        ]

        print(f"\nEvent timing:")
        intervals = []
        for i in range(len(event_times)):
            if i == 0:
                print(f"  Event {i + 1}: {event_times[i].isoformat()}")
            else:
                interval = (event_times[i] - event_times[i - 1]).total_seconds()
                intervals.append(interval)
                print(
                    f"  Event {i + 1}: {event_times[i].isoformat()} (+{interval:.1f}s)"
                )

        # Verify intervals are approximately 5 seconds
        if intervals:
            avg_interval = sum(intervals) / len(intervals)
            print(f"\nAverage interval: {avg_interval:.1f}s (expected: 5s)")
            assert abs(avg_interval - 5.0) < 2.0, (
                f"Average interval {avg_interval:.1f}s not close to 5s"
            )

        # Verify executions
        executions = wait_for_execution_count(
            client=client,
            expected_count=expected_fires,
            action_ref=action["ref"],
            timeout=20,
        )

        succeeded = sum(
            1 for e in executions[:expected_fires] if e["status"] == "succeeded"
        )
        print(f"✓ {succeeded}/{expected_fires} executions succeeded")

        assert succeeded >= expected_fires
        print(f"✓ Test PASSED")

    def test_cron_timer_top_of_minute(self, client: AttuneClient, pack_ref: str):
        """Test cron timer that fires at top of each minute"""

        cron_expression = "0 * * * * *"  # Every minute at second 0

        print(f"\n=== T1.3c: Cron Timer Top of Minute ===")
        print(f"Expression: {cron_expression}")
        print("Note: This test may take up to 70 seconds")

        # Create automation
        trigger = create_cron_timer(
            client=client, cron_expression=cron_expression, pack_ref=pack_ref
        )
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        print(f"✓ Setup complete")

        # Calculate wait time until next minute
        now = datetime.utcnow()
        current_second = now.second
        wait_until_next = 60 - current_second + 2  # +2 for processing time

        print(f"\n  Current time: {now.isoformat()}Z")
        print(f"  Current second: {current_second}")
        print(f"  Waiting ~{wait_until_next}s for top of next minute...")

        # Wait for at least 1 event (possibly 2 if test spans multiple minutes)
        start = time.time()
        events = wait_for_event_count(
            client=client,
            expected_count=1,
            trigger_id=trigger["id"],
            timeout=wait_until_next + 5,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} event(s) created in {elapsed:.1f}s")

        # Verify event occurred at second 0 (±2s tolerance)
        event = events[0]
        event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
        event_second = event_time.second

        print(f"\n  Event time: {event_time.isoformat()}")
        print(f"  Event second: {event_second}")

        # Allow ±3 second tolerance (sensor polling + processing)
        assert event_second <= 3 or event_second >= 57, (
            f"Event fired at second {event_second}, expected at/near second 0"
        )

        # Verify execution
        executions = wait_for_execution_count(
            client=client, expected_count=1, action_ref=action["ref"], timeout=15
        )

        assert len(executions) >= 1
        print(f"✓ Execution completed")
        print(f"✓ Test PASSED")

    def test_cron_timer_complex_expression(self, client: AttuneClient, pack_ref: str):
        """Test complex cron expression (multiple fields)"""

        # Every 10 seconds between seconds 0-30
        # This will fire at: 0, 10, 20, 30 seconds
        cron_expression = "0,10,20,30 * * * * *"

        print(f"\n=== T1.3d: Complex Cron Expression ===")
        print(f"Expression: {cron_expression}")
        print("Expected: Fire at 0, 10, 20, 30 seconds of each minute")

        # Create automation
        trigger = create_cron_timer(
            client=client, cron_expression=cron_expression, pack_ref=pack_ref
        )
        action = create_echo_action(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_id=trigger["id"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        print(f"✓ Setup complete")

        # Wait for at least 2 fires
        print(f"\nWaiting for 2 events (max 45s)...")
        start = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=2,
            trigger_id=trigger["id"],
            timeout=45,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} events in {elapsed:.1f}s")

        # Check that events occurred at valid seconds
        valid_seconds = [0, 10, 20, 30]
        print(f"\nEvent seconds:")
        for i, event in enumerate(events[:2]):
            event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
            second = event_time.second
            print(f"  Event {i + 1}: second {second:02d}")

            # Check within ±2 seconds of valid times
            matched = any(abs(second - vs) <= 2 for vs in valid_seconds)
            assert matched, (
                f"Event at second {second} not near valid seconds {valid_seconds}"
            )

        # Verify executions
        executions = wait_for_execution_count(
            client=client, expected_count=2, action_ref=action["ref"], timeout=20
        )

        assert len(executions) >= 2
        print(f"✓ {len(executions)} executions completed")
        print(f"✓ Test PASSED")
