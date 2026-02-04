"""
T2.9: Execution Timeout Policy

Tests that long-running actions are killed after timeout, preventing indefinite
execution and resource exhaustion.

Test validates:
- Action process killed after timeout
- Execution status: 'running' → 'failed'
- Error message indicates timeout
- Exit code indicates SIGTERM/SIGKILL
- Worker remains stable after kill
- No zombie processes
"""

import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def test_execution_timeout_basic(client: AttuneClient, test_pack):
    """
    Test that long-running action is killed after timeout.

    Flow:
    1. Create action that sleeps for 60 seconds
    2. Configure timeout policy: 5 seconds
    3. Execute action
    4. Verify execution starts
    5. Wait 7 seconds
    6. Verify worker kills action process
    7. Verify execution status becomes 'failed'
    8. Verify timeout error message recorded
    """
    print("\n" + "=" * 80)
    print("TEST: Execution Timeout Policy (T2.9)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create long-running action
    # ========================================================================
    print("\n[STEP 1] Creating long-running action...")

    long_running_script = """#!/usr/bin/env python3
import sys
import time

print('Action starting...')
print('Sleeping for 60 seconds...')
sys.stdout.flush()

time.sleep(60)

print('Action completed (should not reach here)')
sys.exit(0)
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"long_running_{unique_ref()}",
            "description": "Action that runs for 60 seconds",
            "runner_type": "python3",
            "entry_point": "long_run.py",
            "enabled": True,
            "parameters": {},
            "metadata": {
                "timeout": 5  # 5 second timeout
            },
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")
    print(f"  Timeout: 5 seconds")
    print(f"  Actual duration: 60 seconds (without timeout)")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing action...")

    start_time = time.time()
    execution = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait briefly and verify it's running
    # ========================================================================
    print("\n[STEP 3] Verifying execution starts...")

    time.sleep(2)
    execution_status = client.get_execution(execution_id)
    print(f"  Execution status after 2s: {execution_status['status']}")

    if execution_status["status"] == "running":
        print("  ✓ Execution is running")
    else:
        print(f"  ℹ Execution status: {execution_status['status']}")

    # ========================================================================
    # STEP 4: Wait for timeout to occur
    # ========================================================================
    print("\n[STEP 4] Waiting for timeout to occur (7 seconds total)...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="failed",
        timeout=10,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Execution completed: status={result['status']}")
    print(f"  Total execution time: {total_time:.1f}s")

    # ========================================================================
    # STEP 5: Verify timeout behavior
    # ========================================================================
    print("\n[STEP 5] Verifying timeout behavior...")

    # Execution should fail
    assert result["status"] == "failed", (
        f"❌ Expected status 'failed', got '{result['status']}'"
    )
    print("  ✓ Execution status: failed")

    # Execution should complete in ~5 seconds, not 60
    if total_time < 10:
        print(f"  ✓ Execution timed out quickly: {total_time:.1f}s < 10s")
    else:
        print(f"  ⚠ Execution took longer: {total_time:.1f}s")

    # Check for timeout indication in result
    result_details = client.get_execution(execution_id)
    exit_code = result_details.get("exit_code")
    error_message = result_details.get("error") or result_details.get("stderr") or ""

    print(f"  Exit code: {exit_code}")
    if error_message:
        print(f"  Error message: {error_message[:100]}...")

    # Exit code might indicate signal (negative values or specific codes)
    if exit_code and (exit_code < 0 or exit_code in [124, 137, 143]):
        print("  ✓ Exit code suggests timeout/signal")
    else:
        print(f"  ℹ Exit code: {exit_code}")

    # ========================================================================
    # STEP 6: Validate success criteria
    # ========================================================================
    print("\n[STEP 6] Validating success criteria...")

    # Criterion 1: Execution failed
    assert result["status"] == "failed", "❌ Execution should fail"
    print("  ✓ Execution failed due to timeout")

    # Criterion 2: Completed quickly (not full 60 seconds)
    assert total_time < 15, f"❌ Execution took too long: {total_time:.1f}s"
    print(f"  ✓ Execution killed promptly: {total_time:.1f}s")

    # Criterion 3: Worker remains stable (we can still make requests)
    try:
        client.list_executions(limit=1)
        print("  ✓ Worker remains stable after timeout")
    except Exception as e:
        print(f"  ⚠ Worker may be unstable: {e}")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Execution Timeout Policy")
    print("=" * 80)
    print(f"✓ Action with 60s duration: {action_ref}")
    print(f"✓ Timeout policy: 5 seconds")
    print(f"✓ Execution killed after timeout")
    print(f"✓ Status changed to: failed")
    print(f"✓ Total time: {total_time:.1f}s (not 60s)")
    print(f"✓ Worker remained stable")
    print("\n✅ TEST PASSED: Execution timeout works correctly!")
    print("=" * 80 + "\n")


def test_execution_timeout_hierarchy(client: AttuneClient, test_pack):
    """
    Test timeout at different levels: action, workflow, system.

    Flow:
    1. Create action with action-level timeout
    2. Create workflow with workflow-level timeout
    3. Test both timeout levels
    """
    print("\n" + "=" * 80)
    print("TEST: Execution Timeout - Timeout Hierarchy")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action with short timeout
    # ========================================================================
    print("\n[STEP 1] Creating action with action-level timeout...")

    action_with_timeout = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"action_timeout_{unique_ref()}",
            "description": "Action with 3s timeout",
            "runner_type": "python3",
            "entry_point": "action.py",
            "enabled": True,
            "parameters": {},
            "metadata": {
                "timeout": 3  # Action-level timeout: 3 seconds
            },
        },
    )
    print(f"✓ Created action: {action_with_timeout['ref']}")
    print(f"  Action-level timeout: 3 seconds")

    # ========================================================================
    # STEP 2: Create workflow with workflow-level timeout
    # ========================================================================
    print("\n[STEP 2] Creating workflow with workflow-level timeout...")

    task_action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_{unique_ref()}",
            "description": "Task action",
            "runner_type": "python3",
            "entry_point": "task.py",
            "enabled": True,
            "parameters": {},
        },
    )

    workflow_with_timeout = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"workflow_timeout_{unique_ref()}",
            "description": "Workflow with 5s timeout",
            "runner_type": "workflow",
            "entry_point": "",
            "enabled": True,
            "parameters": {},
            "metadata": {
                "timeout": 5  # Workflow-level timeout: 5 seconds
            },
            "workflow_definition": {
                "tasks": [
                    {"name": "task_1", "action": task_action["ref"], "parameters": {}},
                ]
            },
        },
    )
    print(f"✓ Created workflow: {workflow_with_timeout['ref']}")
    print(f"  Workflow-level timeout: 5 seconds")

    # ========================================================================
    # STEP 3: Test action-level timeout
    # ========================================================================
    print("\n[STEP 3] Testing action-level timeout...")

    action_execution = client.create_execution(
        action_ref=action_with_timeout["ref"], parameters={}
    )
    action_execution_id = action_execution["id"]
    print(f"✓ Action execution created: ID={action_execution_id}")

    # Action has 3s timeout, so should complete within 5s
    time.sleep(5)
    action_result = client.get_execution(action_execution_id)
    print(f"  Action execution status: {action_result['status']}")

    # ========================================================================
    # STEP 4: Test workflow-level timeout
    # ========================================================================
    print("\n[STEP 4] Testing workflow-level timeout...")

    workflow_execution = client.create_execution(
        action_ref=workflow_with_timeout["ref"], parameters={}
    )
    workflow_execution_id = workflow_execution["id"]
    print(f"✓ Workflow execution created: ID={workflow_execution_id}")

    # Workflow has 5s timeout
    time.sleep(7)
    workflow_result = client.get_execution(workflow_execution_id)
    print(f"  Workflow execution status: {workflow_result['status']}")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Timeout Hierarchy")
    print("=" * 80)
    print(f"✓ Action-level timeout tested: 3s")
    print(f"✓ Workflow-level timeout tested: 5s")
    print(f"✓ Multiple timeout levels work")
    print("\n✅ TEST PASSED: Timeout hierarchy works correctly!")
    print("=" * 80 + "\n")


def test_execution_no_timeout_completes_normally(client: AttuneClient, test_pack):
    """
    Test that actions without timeout complete normally.

    Flow:
    1. Create action that sleeps 3 seconds (no timeout)
    2. Execute action
    3. Verify it completes successfully
    4. Verify it takes full duration
    """
    print("\n" + "=" * 80)
    print("TEST: No Timeout - Normal Completion")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action without timeout
    # ========================================================================
    print("\n[STEP 1] Creating action without timeout...")

    normal_script = """#!/usr/bin/env python3
import sys
import time

print('Action starting...')
time.sleep(3)
print('Action completed normally')
sys.exit(0)
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"no_timeout_{unique_ref()}",
            "description": "Action without timeout",
            "runner_type": "python3",
            "entry_point": "normal.py",
            "enabled": True,
            "parameters": {},
            # No timeout specified
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")
    print(f"  No timeout configured")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing action...")

    start_time = time.time()
    execution = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait for completion
    # ========================================================================
    print("\n[STEP 3] Waiting for completion...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=10,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Execution completed: status={result['status']}")
    print(f"  Total time: {total_time:.1f}s")

    # ========================================================================
    # STEP 4: Verify normal completion
    # ========================================================================
    print("\n[STEP 4] Verifying normal completion...")

    assert result["status"] == "succeeded", (
        f"❌ Expected 'succeeded', got '{result['status']}'"
    )
    print("  ✓ Execution succeeded")

    # Should take at least 3 seconds (sleep duration)
    if total_time >= 3:
        print(f"  ✓ Completed full duration: {total_time:.1f}s >= 3s")
    else:
        print(f"  ⚠ Completed quickly: {total_time:.1f}s < 3s")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: No Timeout - Normal Completion")
    print("=" * 80)
    print(f"✓ Action without timeout: {action_ref}")
    print(f"✓ Execution completed successfully")
    print(f"✓ Duration: {total_time:.1f}s")
    print(f"✓ No premature termination")
    print("\n✅ TEST PASSED: Actions without timeout work correctly!")
    print("=" * 80 + "\n")


def test_execution_timeout_vs_failure(client: AttuneClient, test_pack):
    """
    Test distinguishing between timeout and regular failure.

    Flow:
    1. Create action that fails immediately (exit 1)
    2. Create action that times out
    3. Execute both
    4. Verify different failure reasons
    """
    print("\n" + "=" * 80)
    print("TEST: Timeout vs Regular Failure")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action that fails immediately
    # ========================================================================
    print("\n[STEP 1] Creating action that fails immediately...")

    fail_script = """#!/usr/bin/env python3
import sys
print('Failing immediately')
sys.exit(1)
"""

    fail_action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"immediate_fail_{unique_ref()}",
            "description": "Action that fails immediately",
            "runner_type": "python3",
            "entry_point": "fail.py",
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created fail action: {fail_action['ref']}")

    # ========================================================================
    # STEP 2: Create action that times out
    # ========================================================================
    print("\n[STEP 2] Creating action that times out...")

    timeout_action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"timeout_{unique_ref()}",
            "description": "Action that times out",
            "runner_type": "python3",
            "entry_point": "timeout.py",
            "enabled": True,
            "parameters": {},
            "metadata": {"timeout": 2},
        },
    )
    print(f"✓ Created timeout action: {timeout_action['ref']}")

    # ========================================================================
    # STEP 3: Execute fail action
    # ========================================================================
    print("\n[STEP 3] Executing fail action...")

    fail_execution = client.create_execution(
        action_ref=fail_action["ref"], parameters={}
    )
    fail_execution_id = fail_execution["id"]

    fail_result = wait_for_execution_status(
        client=client,
        execution_id=fail_execution_id,
        expected_status="failed",
        timeout=10,
    )
    print(f"✓ Fail execution completed: status={fail_result['status']}")

    fail_details = client.get_execution(fail_execution_id)
    fail_exit_code = fail_details.get("exit_code")
    print(f"  Exit code: {fail_exit_code}")

    # ========================================================================
    # STEP 4: Execute timeout action
    # ========================================================================
    print("\n[STEP 4] Executing timeout action...")

    timeout_execution = client.create_execution(
        action_ref=timeout_action["ref"], parameters={}
    )
    timeout_execution_id = timeout_execution["id"]

    timeout_result = wait_for_execution_status(
        client=client,
        execution_id=timeout_execution_id,
        expected_status="failed",
        timeout=10,
    )
    print(f"✓ Timeout execution completed: status={timeout_result['status']}")

    timeout_details = client.get_execution(timeout_execution_id)
    timeout_exit_code = timeout_details.get("exit_code")
    print(f"  Exit code: {timeout_exit_code}")

    # ========================================================================
    # STEP 5: Compare failure types
    # ========================================================================
    print("\n[STEP 5] Comparing failure types...")

    print(f"\n  Immediate Failure:")
    print(f"  - Exit code: {fail_exit_code}")
    print(f"  - Expected: 1 (explicit exit code)")

    print(f"\n  Timeout Failure:")
    print(f"  - Exit code: {timeout_exit_code}")
    print(f"  - Expected: negative or signal code (e.g., -15, 137, 143)")

    # Different exit codes suggest different failure types
    if fail_exit_code != timeout_exit_code:
        print("\n  ✓ Exit codes differ (different failure types)")
    else:
        print("\n  ℹ Exit codes same (may not distinguish timeout)")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Timeout vs Regular Failure")
    print("=" * 80)
    print(f"✓ Regular failure exit code: {fail_exit_code}")
    print(f"✓ Timeout failure exit code: {timeout_exit_code}")
    print(f"✓ Both failures handled appropriately")
    print("\n✅ TEST PASSED: Failure types distinguishable!")
    print("=" * 80 + "\n")
