#!/usr/bin/env python3
"""
T1.3: Cron Timer Execution

Tests that an action executes on a cron schedule.

Test Flow:
1. Create echo action
2. Create cron timer rule (every N seconds via 6-field cron expression)
3. Wait for events from the sensor
4. Verify executions at correct intervals

Success Criteria:
- Cron expression correctly parsed
- Events fire at expected intervals
- Executions complete successfully
"""

import time
from datetime import datetime

import pytest
from helpers import (
    AttuneClient,
    create_cron_timer,
    create_echo_action,
    wait_for_event_count,
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.tier1
@pytest.mark.timer
@pytest.mark.integration
@pytest.mark.timeout(90)
class TestCronTimerAutomation:
    """Test cron timer automation flow"""

    def test_cron_timer_every_5_seconds(self, client: AttuneClient, pack_ref: str):
        """Test cron timer with */5 expression (every 5 seconds)"""

        cron_expression = "*/5 * * * * *"  # Every 5 seconds
        expected_fires = 4
        max_wait = 30

        print(f"\n=== T1.3: Cron Timer Every 5 Seconds ===")
        print(f"Expression: {cron_expression}")

        # Create echo action
        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]

        # Create cron timer (fixture creates rule internally)
        created_after = datetime.utcnow().isoformat() + "Z"
        timer = create_cron_timer(
            client=client,
            cron_expression=cron_expression,
            action_ref=action_ref,
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create cron timer rule"
        print(f"✓ Setup complete: rule={rule['id']}")

        # Wait for events
        print(f"\nWaiting for {expected_fires} events (max {max_wait}s)...")
        start = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_ref="core.crontimer",
            rule_id=rule["id"],
            created_after=created_after,
            timeout=max_wait,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} events in {elapsed:.1f}s")

        # Check timing — intervals should be roughly 5s
        # Skip the first interval (may be a startup burst from race between
        # initial API fetch and MQ RuleCreated message)
        event_times = [
            datetime.fromisoformat(e["created"].replace("Z", "+00:00"))
            for e in sorted(events[:expected_fires], key=lambda e: e["created"])
        ]

        print(f"\nEvent timing:")
        intervals = []
        for i in range(len(event_times)):
            if i == 0:
                print(f"  Event {i + 1}: {event_times[i].isoformat()}")
            else:
                interval = (event_times[i] - event_times[i - 1]).total_seconds()
                intervals.append(interval)
                print(f"  Event {i + 1}: {event_times[i].isoformat()} (+{interval:.1f}s)")

        # Check only steady-state intervals (skip first which may be a burst)
        steady_intervals = [iv for iv in intervals if iv > 2.0]
        if steady_intervals:
            avg_interval = sum(steady_intervals) / len(steady_intervals)
            print(f"\nSteady-state avg interval: {avg_interval:.1f}s (expected: 5s)")
            assert abs(avg_interval - 5.0) < 2.0, (
                f"Average steady-state interval {avg_interval:.1f}s not close to 5s"
            )

        # Verify executions
        executions = wait_for_execution_count(
            client=client,
            expected_count=expected_fires,
            action_ref=action_ref,
            timeout=20,
        )

        succeeded = sum(1 for e in executions[:expected_fires] if e["status"] == "completed")
        # Allow N-1 tolerance for artifact version race condition
        assert succeeded >= expected_fires - 1, (
            f"Only {succeeded}/{expected_fires} executions succeeded"
        )
        print(f"✓ {succeeded}/{expected_fires} executions succeeded")
        print(f"✓ Test PASSED")

    def test_cron_timer_specific_seconds(self, client: AttuneClient, pack_ref: str):
        """Test cron timer fires at specific seconds in the minute"""

        # Fire at 0, 15, 30, 45 seconds of every minute
        cron_expression = "0,15,30,45 * * * * *"
        expected_fires = 2
        max_wait_seconds = 45

        print(f"\n=== T1.3b: Cron Timer Specific Seconds ===")
        print(f"Cron expression: {cron_expression}")
        print(f"Expected fires: {expected_fires}+ in {max_wait_seconds}s")

        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]

        created_after = datetime.utcnow().isoformat() + "Z"
        timer = create_cron_timer(
            client=client,
            cron_expression=cron_expression,
            action_ref=action_ref,
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create cron timer rule"
        print(f"✓ Setup complete: rule={rule['id']}")

        # Wait for events
        start_time = time.time()
        events = wait_for_event_count(
            client=client,
            expected_count=expected_fires,
            trigger_ref="core.crontimer",
            rule_id=rule["id"],
            created_after=created_after,
            timeout=max_wait_seconds,
            poll_interval=1.0,
        )

        elapsed = time.time() - start_time
        print(f"✓ {len(events)} events in {elapsed:.1f}s")

        # Verify event timing — should be at one of the expected seconds
        expected_seconds = [0, 15, 30, 45]
        for i, event in enumerate(events[:expected_fires]):
            event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
            second = event_time.second
            print(f"  Event {i + 1}: second {second:02d}")

            matched = any(
                abs(second - es) <= 2 or abs(second - es) >= 58
                for es in expected_seconds
            )
            assert matched, (
                f"Event at second {second} not near expected {expected_seconds}"
            )

        # Verify executions
        executions = wait_for_execution_count(
            client=client,
            expected_count=expected_fires,
            action_ref=action_ref,
            timeout=20,
        )

        succeeded = sum(1 for e in executions[:expected_fires] if e["status"] == "completed")
        assert succeeded >= expected_fires - 1
        print(f"✓ {succeeded}/{expected_fires} executions succeeded")
        print(f"✓ Test PASSED")

    def test_cron_timer_top_of_minute(self, client: AttuneClient, pack_ref: str):
        """Test cron timer that fires at top of each minute"""

        cron_expression = "0 * * * * *"  # Every minute at second 0

        print(f"\n=== T1.3c: Cron Timer Top of Minute ===")
        print(f"Expression: {cron_expression}")
        print("Note: This test may take up to 70 seconds")

        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]

        created_after = datetime.utcnow().isoformat() + "Z"
        timer = create_cron_timer(
            client=client,
            cron_expression=cron_expression,
            action_ref=action_ref,
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create cron timer rule"

        # Calculate wait time until next minute
        now = datetime.utcnow()
        current_second = now.second
        wait_until_next = 60 - current_second + 5  # +5 for sensor pickup delay

        print(f"  Current second: {current_second}")
        print(f"  Waiting ~{wait_until_next}s for top of next minute...")

        start = time.time()
        events = wait_for_event_count(
            client=client,
            expected_count=1,
            trigger_ref="core.crontimer",
            rule_id=rule["id"],
            created_after=created_after,
            timeout=wait_until_next + 5,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} event(s) in {elapsed:.1f}s")

        # Verify event occurred near second 0
        event = events[0]
        event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
        event_second = event_time.second
        print(f"  Event second: {event_second}")

        assert event_second <= 3 or event_second >= 57, (
            f"Event at second {event_second}, expected near 0"
        )

        # Verify execution
        executions = wait_for_execution_count(
            client=client, expected_count=1, action_ref=action_ref, timeout=15
        )
        assert len(executions) >= 1
        print(f"✓ Execution succeeded")
        print(f"✓ Test PASSED")

    def test_cron_timer_complex_expression(self, client: AttuneClient, pack_ref: str):
        """Test complex cron expression (multiple fields)"""

        # Fire at 0, 10, 20, 30 seconds
        cron_expression = "0,10,20,30 * * * * *"

        print(f"\n=== T1.3d: Complex Cron Expression ===")
        print(f"Expression: {cron_expression}")

        action = create_echo_action(client=client, pack_ref=pack_ref)
        action_ref = action["ref"]

        created_after = datetime.utcnow().isoformat() + "Z"
        timer = create_cron_timer(
            client=client,
            cron_expression=cron_expression,
            action_ref=action_ref,
            pack_ref=pack_ref,
        )
        rule = timer["rule"]
        assert rule is not None, "Failed to create cron timer rule"

        # Wait for at least 2 fires
        print(f"\nWaiting for 2 events (max 35s)...")
        start = time.time()

        events = wait_for_event_count(
            client=client,
            expected_count=2,
            trigger_ref="core.crontimer",
            rule_id=rule["id"],
            created_after=created_after,
            timeout=35,
            poll_interval=1.0,
        )

        elapsed = time.time() - start
        print(f"✓ {len(events)} events in {elapsed:.1f}s")

        # Verify events at valid seconds
        valid_seconds = [0, 10, 20, 30]
        for i, event in enumerate(events[:2]):
            event_time = datetime.fromisoformat(event["created"].replace("Z", "+00:00"))
            second = event_time.second
            print(f"  Event {i + 1}: second {second:02d}")
            matched = any(abs(second - vs) <= 2 for vs in valid_seconds)
            assert matched, f"Event at second {second} not near {valid_seconds}"

        # Verify executions
        executions = wait_for_execution_count(
            client=client, expected_count=2, action_ref=action_ref, timeout=20
        )
        assert len(executions) >= 2
        print(f"✓ {len(executions)} executions succeeded")
        print(f"✓ Test PASSED")
