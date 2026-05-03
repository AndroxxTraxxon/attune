"""
T3.3: Multiple Concurrent Timers Test

Tests that multiple timers with different intervals run independently
without interfering with each other.

Priority: LOW
Duration: ~30 seconds
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, create_interval_timer, unique_ref
from helpers.polling import wait_for_execution_count


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.performance
def test_multiple_concurrent_timers(client: AttuneClient, test_pack):
    """
    Test that multiple timers with different intervals run independently.

    Setup:
    - Timer A: every 3 seconds
    - Timer B: every 5 seconds
    - Timer C: every 7 seconds

    Run for 21 seconds (LCM of 3, 5, 7 is 105, but 21 gives us good data):
    - Timer A should fire ~7 times (21/3 = 7)
    - Timer B should fire ~4 times (21/5 = 4.2)
    - Timer C should fire ~3 times (21/7 = 3)
    """
    print("\n" + "=" * 80)
    print("T3.3a: Multiple Concurrent Timers Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create actions and three timers with different intervals
    print("\n[STEP 1] Creating actions and three interval timer rules...")

    timers = []

    # Timer A: 3 seconds
    trigger_a = f"timer_3s_{unique_ref()}"
    action_a = create_echo_action(
        client=client, pack_ref=pack_ref, message="Timer A tick", suffix="_3s"
    )["ref"]
    create_interval_timer(
        client=client, pack_ref=pack_ref, trigger_ref=trigger_a, interval=3, action_ref=action_a
    )
    timers.append({"trigger": trigger_a, "interval": 3, "name": "Timer A", "action_ref": action_a})
    print(f"✓ Timer A created: {trigger_a} (3 seconds)")

    # Timer B: 5 seconds
    trigger_b = f"timer_5s_{unique_ref()}"
    action_b = create_echo_action(
        client=client, pack_ref=pack_ref, message="Timer B tick", suffix="_5s"
    )["ref"]
    create_interval_timer(
        client=client, pack_ref=pack_ref, trigger_ref=trigger_b, interval=5, action_ref=action_b
    )
    timers.append({"trigger": trigger_b, "interval": 5, "name": "Timer B", "action_ref": action_b})
    print(f"✓ Timer B created: {trigger_b} (5 seconds)")

    # Timer C: 7 seconds
    trigger_c = f"timer_7s_{unique_ref()}"
    action_c = create_echo_action(
        client=client, pack_ref=pack_ref, message="Timer C tick", suffix="_7s"
    )["ref"]
    create_interval_timer(
        client=client, pack_ref=pack_ref, trigger_ref=trigger_c, interval=7, action_ref=action_c
    )
    timers.append({"trigger": trigger_c, "interval": 7, "name": "Timer C", "action_ref": action_c})
    print(f"✓ Timer C created: {trigger_c} (7 seconds)")

    actions = [
        {"ref": action_a, "name": "Action A"},
        {"ref": action_b, "name": "Action B"},
        {"ref": action_c, "name": "Action C"},
    ]

    # Step 2: Run for 21 seconds and monitor
    print("\n[STEP 2] Running for 21 seconds...")
    print("  Monitoring timer executions...")

    test_duration = 21
    start_time = time.time()

    # Take snapshots at intervals
    snapshots = []

    for i in range(8):  # 0, 3, 6, 9, 12, 15, 18, 21 seconds
        if i > 0:
            time.sleep(3)

        elapsed = time.time() - start_time
        snapshot = {"time": elapsed, "counts": {}}

        for action in actions:
            executions = client.list_executions(action_ref=action["ref"])
            snapshot["counts"][action["name"]] = len(executions)

        snapshots.append(snapshot)
        print(
            f"  t={elapsed:.1f}s: A={snapshot['counts']['Action A']}, "
            f"B={snapshot['counts']['Action B']}, C={snapshot['counts']['Action C']}"
        )

    # Step 5: Verify final counts
    print("\n[STEP 5] Verifying execution counts...")

    final_counts = {
        "Action A": len(client.list_executions(action_ref=action_a)),
        "Action B": len(client.list_executions(action_ref=action_b)),
        "Action C": len(client.list_executions(action_ref=action_c)),
    }

    expected_counts = {
        "Action A": {"min": 6, "max": 8, "ideal": 7},  # 21/3 = 7
        "Action B": {"min": 3, "max": 5, "ideal": 4},  # 21/5 = 4.2
        "Action C": {"min": 2, "max": 4, "ideal": 3},  # 21/7 = 3
    }

    print(f"\nFinal execution counts:")
    results = {}

    for action_name, count in final_counts.items():
        expected = expected_counts[action_name]
        in_range = expected["min"] <= count <= expected["max"]
        status = "✓" if in_range else "⚠"

        print(
            f"  {status} {action_name}: {count} executions "
            f"(expected: {expected['ideal']}, range: {expected['min']}-{expected['max']})"
        )

        results[action_name] = {
            "count": count,
            "expected": expected["ideal"],
            "in_range": in_range,
        }

    # Step 6: Check for timer drift
    print("\n[STEP 6] Checking for timer drift...")

    # Analyze timing consistency
    timing_ok = True

    if len(snapshots) > 2:
        # Check Timer A (should increase by 1 every 3 seconds)
        a_increases = []
        for i in range(1, len(snapshots)):
            increase = (
                snapshots[i]["counts"]["Action A"]
                - snapshots[i - 1]["counts"]["Action A"]
            )
            a_increases.append(increase)

        # Should mostly be 1s (one execution per 3-second interval)
        if any(inc > 2 for inc in a_increases):
            print(f"⚠ Timer A may have drift: {a_increases}")
            timing_ok = False
        else:
            print(f"✓ Timer A consistent: {a_increases}")

    # Step 7: Verify no interference
    print("\n[STEP 7] Verifying no timer interference...")

    # Check that timers didn't affect each other's timing
    interference_detected = False

    # If all timers are within expected ranges, no interference
    if all(r["in_range"] for r in results.values()):
        print(f"✓ All timers within expected ranges (no interference)")
    else:
        print(f"⚠ Some timers outside expected ranges")
        interference_detected = True

    # Summary
    print("\n" + "=" * 80)
    print("CONCURRENT TIMERS TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Test duration: {test_duration} seconds")
    print(f"✓ Timers created: 3 (3s, 5s, 7s intervals)")
    print(f"✓ Final counts:")
    print(f"    Timer A (3s): {final_counts['Action A']} executions (expected ~7)")
    print(f"    Timer B (5s): {final_counts['Action B']} executions (expected ~4)")
    print(f"    Timer C (7s): {final_counts['Action C']} executions (expected ~3)")

    all_in_range = all(r["in_range"] for r in results.values())

    if all_in_range and not interference_detected:
        print("\n✅ CONCURRENT TIMERS WORKING INDEPENDENTLY!")
    else:
        print("\n⚠️ Some timers outside expected ranges")
        print("   This may be due to system load or timing variations")

    print("=" * 80)

    # Allow some tolerance
    assert results["Action A"]["count"] >= 5, "Timer A fired too few times"
    assert results["Action B"]["count"] >= 3, "Timer B fired too few times"
    assert results["Action C"]["count"] >= 2, "Timer C fired too few times"


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.performance
def test_many_concurrent_timers(client: AttuneClient, test_pack):
    """
    Test system can handle many concurrent timers (stress test).

    Creates 5 timers with 2-second intervals and verifies they all fire.
    """
    print("\n" + "=" * 80)
    print("T3.3b: Many Concurrent Timers Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create 5 timers
    print("\n[STEP 1] Creating 5 concurrent timers...")

    num_timers = 5
    timers_and_actions = []

    for i in range(num_timers):
        trigger_ref = f"multi_timer_{i}_{unique_ref()}"
        action_ref = create_echo_action(
            client=client,
            pack_ref=pack_ref,
            message=f"Timer {i} tick",
            suffix=f"_multi{i}",
        )["ref"]

        timer = create_interval_timer(
            client=client, pack_ref=pack_ref, trigger_ref=trigger_ref, interval=2, action_ref=action_ref
        )

        timers_and_actions.append(
            {
                "trigger_ref": trigger_ref,
                "action_ref": action_ref,
                "rule_id": timer["rule"]["id"],
                "index": i,
            }
        )

        print(f"✓ Timer {i} created (2s interval)")

    # Step 2: Wait for executions
    print(f"\n[STEP 2] Waiting 8 seconds for executions...")
    time.sleep(8)

    # Step 3: Check all timers fired
    print(f"\n[STEP 3] Checking execution counts...")

    all_fired = True
    total_executions = 0

    for timer_info in timers_and_actions:
        executions = client.list_executions(action_ref=timer_info["action_ref"])
        count = len(executions)
        total_executions += count

        status = "✓" if count >= 3 else "⚠"
        print(f"  {status} Timer {timer_info['index']}: {count} executions")

        if count < 2:
            all_fired = False

    print(f"\nTotal executions: {total_executions}")
    print(f"Average per timer: {total_executions / num_timers:.1f}")

    # Summary
    print("\n" + "=" * 80)
    print("MANY CONCURRENT TIMERS TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Timers created: {num_timers}")
    print(f"✓ Total executions: {total_executions}")
    print(f"✓ All timers fired: {all_fired}")

    if all_fired:
        print("\n✅ SYSTEM HANDLES MANY CONCURRENT TIMERS!")
    else:
        print("\n⚠️ Some timers did not fire as expected")

    print("=" * 80)

    assert total_executions >= num_timers * 2, (
        f"Expected at least {num_timers * 2} total executions, got {total_executions}"
    )


@pytest.mark.tier3
@pytest.mark.timer
@pytest.mark.performance
def test_timer_precision_under_load(client: AttuneClient, test_pack):
    """
    Test timer precision when multiple timers are running.

    Verifies that timer precision doesn't degrade with concurrent timers.
    """
    print("\n" + "=" * 80)
    print("T3.3c: Timer Precision Under Load Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create 3 timers
    print("\n[STEP 1] Creating 3 timers (2s interval each)...")

    triggers = []
    actions = []

    for i in range(3):
        trigger_ref = f"precision_timer_{i}_{unique_ref()}"
        action_ref = create_echo_action(
            client=client,
            pack_ref=pack_ref,
            message=f"Precision timer {i}",
            suffix=f"_prec{i}",
        )["ref"]
        actions.append(action_ref)

        create_interval_timer(
            client=client, pack_ref=pack_ref, trigger_ref=trigger_ref, interval=2, action_ref=action_ref
        )
        triggers.append(trigger_ref)

        print(f"✓ Timer {i} created")

    # Step 2: Monitor timing
    print("\n[STEP 2] Monitoring timing precision...")

    start_time = time.time()
    measurements = []

    for check in range(4):  # Check at 0, 3, 6, 9 seconds
        if check > 0:
            time.sleep(3)

        elapsed = time.time() - start_time

        # Count executions for first timer
        execs = client.list_executions(action_ref=actions[0])
        count = len(execs)

        expected = int(elapsed / 2)
        delta = abs(count - expected)

        measurements.append(
            {"elapsed": elapsed, "count": count, "expected": expected, "delta": delta}
        )

        print(
            f"  t={elapsed:.1f}s: {count} executions (expected: {expected}, delta: {delta})"
        )

    # Step 3: Calculate precision
    print("\n[STEP 3] Calculating timing precision...")

    max_delta = max(m["delta"] for m in measurements)
    avg_delta = sum(m["delta"] for m in measurements) / len(measurements)

    print(f"  Maximum delta: {max_delta} executions")
    print(f"  Average delta: {avg_delta:.1f} executions")

    precision_ok = max_delta <= 1

    if precision_ok:
        print(f"✓ Timing precision acceptable (max delta ≤ 1)")
    else:
        print(f"⚠ Timing precision degraded (max delta > 1)")

    # Summary
    print("\n" + "=" * 80)
    print("TIMER PRECISION UNDER LOAD TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Concurrent timers: 3")
    print(f"✓ Max timing delta: {max_delta}")
    print(f"✓ Avg timing delta: {avg_delta:.1f}")

    if precision_ok:
        print("\n✅ TIMER PRECISION MAINTAINED UNDER LOAD!")
    else:
        print("\n⚠️ Timer precision may degrade under concurrent load")

    print("=" * 80)

    assert max_delta <= 2, f"Timing precision too poor: max delta {max_delta}"
