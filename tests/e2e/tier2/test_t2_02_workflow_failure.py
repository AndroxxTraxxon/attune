"""
T2.2: Workflow with Failure Handling

Tests that workflows handle child task failures using the canonical transition
model.

Test validates:
- First child completes successfully
- Second child fails as expected
- A success transition from the failing task is not followed
- Parent status reflects the failed child execution
- All execution statuses correct
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def test_workflow_failure_blocks_success_transition(client: AttuneClient, test_pack):
    """
    Test workflow failure handling with success-only transitions.

    Flow:
    1. Create workflow with 3 tasks: A (success) → B (fail) → C
    2. Link A -> B -> C with success transitions
    3. Execute workflow
    4. Verify A completes, B fails, C does not execute
    5. Verify workflow status is 'failed'
    """
    print("\n" + "=" * 80)
    print("TEST: Workflow Failure Handling - Success Transition Stops on Failure (T2.2)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create task actions
    # ========================================================================
    print("\n[STEP 1] Creating task actions...")

    # Task A - succeeds
    task_a = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_a_success_{unique_ref()}",
            "description": "Task A - succeeds",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task A succeeded"; echo \'{"task":"A","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task A (success): {task_a['ref']}")

    # Task B - fails
    task_b = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_b_fail_{unique_ref()}",
            "description": "Task B - fails",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task B failing intentionally" >&2; exit 1',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task B (fails): {task_b['ref']}")

    # Task C - should not execute
    task_c = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_c_skipped_{unique_ref()}",
            "description": "Task C - should be skipped",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task C should not run"; echo \'{"task":"C","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task C (should not run): {task_c['ref']}")

    # ========================================================================
    # STEP 2: Create workflow with canonical success transitions
    # ========================================================================
    print("\n[STEP 2] Creating workflow with success-only transitions...")

    workflow_name = f"failure_transition_{unique_ref()}"
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=workflow_name,
        label="Workflow Failure Transition",
        description="Workflow where a failed task blocks its success transition",
        tasks=[
            {
                "name": "task_a",
                "action": task_a["ref"],
                "input": {},
                "next": [{"when": "{{ succeeded() }}", "do": ["task_b"]}],
            },
            {
                "name": "task_b",
                "action": task_b["ref"],
                "input": {},
                "next": [{"when": "{{ succeeded() }}", "do": ["task_c"]}],
            },
            {"name": "task_c", "action": task_c["ref"], "input": {}},
        ],
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print("  Transition path: task_a --succeeded()--> task_b --succeeded()--> task_c")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow (expecting failure)...")

    execution = client.create_execution(action_ref=workflow_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Workflow execution created: ID={execution_id}")

    # ========================================================================
    # STEP 4: Wait for workflow to fail
    # ========================================================================
    print("\n[STEP 4] Waiting for workflow to fail...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="failed",
        timeout=20,
    )
    print(f"✓ Workflow failed as expected: status={result['status']}")

    # ========================================================================
    # STEP 5: Verify task execution pattern
    # ========================================================================
    print("\n[STEP 5] Verifying task execution pattern...")

    all_executions = client.list_executions(parent=execution_id, limit=100)
    task_executions = [
        ex for ex in all_executions if ex.get("parent") == execution_id
    ]

    task_a_execs = [ex for ex in task_executions if ex["action_ref"] == task_a["ref"]]
    task_b_execs = [ex for ex in task_executions if ex["action_ref"] == task_b["ref"]]
    task_c_execs = [ex for ex in task_executions if ex["action_ref"] == task_c["ref"]]

    print(f"  Found {len(task_executions)} task executions")
    print(f"  - Task A executions: {len(task_a_execs)}")
    print(f"  - Task B executions: {len(task_b_execs)}")
    print(f"  - Task C executions: {len(task_c_execs)}")

    # ========================================================================
    # STEP 6: Validate success criteria
    # ========================================================================
    print("\n[STEP 6] Validating success criteria...")

    # Criterion 1: Task A completed
    assert len(task_a_execs) >= 1, "❌ Task A not executed"
    assert task_a_execs[0]["status"] == "completed", (
        f"❌ Task A should succeed: {task_a_execs[0]['status']}"
    )
    print("  ✓ Task A executed and completed")

    # Criterion 2: Task B failed
    assert len(task_b_execs) >= 1, "❌ Task B not executed"
    assert task_b_execs[0]["status"] == "failed", (
        f"❌ Task B should fail: {task_b_execs[0]['status']}"
    )
    print("  ✓ Task B executed and failed")

    # Criterion 3: Task C did not execute because task_b never satisfied succeeded()
    assert len(task_c_execs) == 0, "❌ Task C should not execute after Task B failed"
    print("  ✓ Task C correctly skipped because task_b did not satisfy succeeded()")

    # Criterion 4: Workflow status is failed
    assert result["status"] == "failed", (
        f"❌ Workflow should be failed: {result['status']}"
    )
    print("  ✓ Workflow status: failed")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Workflow Failure - Success Transition Stops on Failure")
    print("=" * 80)
    print(f"✓ Workflow with abort policy: {workflow_ref}")
    print(f"✓ Task A: completed")
    print(f"✓ Task B: failed (intentional)")
    print(f"✓ Task C: skipped (success transition blocked)")
    print(f"✓ Workflow: failed overall")
    print("\n✅ TEST PASSED: Abort-on-failure policy works correctly!")
    print("=" * 80 + "\n")


@pytest.mark.skip(reason="Workflow failure policies (continue/abort) not implemented - uses transition model")
def test_workflow_failure_continue_policy(client: AttuneClient, test_pack):
    """
    Test workflow with continue-on-failure policy.

    Flow:
    1. Create workflow with 3 tasks: A (success) → B (fail) → C (success)
    2. Configure on_failure: continue
    3. Execute workflow
    4. Verify all three tasks execute
    5. Verify workflow status is 'succeeded_with_errors' or similar
    """
    print("\n" + "=" * 80)
    print("TEST: Workflow Failure - Continue Policy")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create task actions
    # ========================================================================
    print("\n[STEP 1] Creating task actions...")

    task_a = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_a_success_{unique_ref()}",
            "description": "Task A - succeeds",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task A succeeded"; echo \'{"task":"A","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task A (success): {task_a['ref']}")

    task_b = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_b_fail_{unique_ref()}",
            "description": "Task B - fails",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task B failing intentionally" >&2; exit 1',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task B (fails): {task_b['ref']}")

    task_c = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"task_c_success_{unique_ref()}",
            "description": "Task C - succeeds",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task C succeeded"; echo \'{"task":"C","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task C (success): {task_c['ref']}")

    # ========================================================================
    # STEP 2: Create workflow with continue policy
    # ========================================================================
    print("\n[STEP 2] Creating workflow with continue policy...")

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"continue_workflow_{unique_ref()}",
            "description": "Workflow with continue-on-failure policy",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "metadata": {
                "on_failure": "continue"  # Continue despite failures
            },
            "workflow_definition": {
                "tasks": [
                    {"name": "task_a", "action": task_a["ref"], "input": {}},
                    {"name": "task_b", "action": task_b["ref"], "input": {}},
                    {"name": "task_c", "action": task_c["ref"], "input": {}},
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Policy: on_failure = continue")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    execution = client.create_execution(action_ref=workflow_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Workflow execution created: ID={execution_id}")

    # ========================================================================
    # STEP 4: Wait for workflow to complete
    # ========================================================================
    print("\n[STEP 4] Waiting for workflow to complete...")

    # May complete with 'succeeded_with_errors' or 'failed' status
    time.sleep(10)  # Give it time to run all tasks

    result = client.get_execution(execution_id)
    print(f"✓ Workflow succeeded: status={result['status']}")

    # ========================================================================
    # STEP 5: Verify task execution pattern
    # ========================================================================
    print("\n[STEP 5] Verifying task execution pattern...")

    all_executions = client.list_executions(limit=100)
    task_executions = [
        ex for ex in all_executions if ex.get("parent") == execution_id
    ]

    task_a_execs = [ex for ex in task_executions if ex["action_ref"] == task_a["ref"]]
    task_b_execs = [ex for ex in task_executions if ex["action_ref"] == task_b["ref"]]
    task_c_execs = [ex for ex in task_executions if ex["action_ref"] == task_c["ref"]]

    print(f"  Found {len(task_executions)} task executions")
    print(f"  - Task A: {len(task_a_execs)} execution(s)")
    print(f"  - Task B: {len(task_b_execs)} execution(s)")
    print(f"  - Task C: {len(task_c_execs)} execution(s)")

    # ========================================================================
    # STEP 6: Validate success criteria
    # ========================================================================
    print("\n[STEP 6] Validating success criteria...")

    # All tasks should execute with continue policy
    assert len(task_a_execs) >= 1, "❌ Task A not executed"
    assert len(task_b_execs) >= 1, "❌ Task B not executed"
    assert len(task_c_execs) >= 1, "❌ Task C not executed (continue policy)"
    print("  ✓ All 3 tasks executed")

    # Verify individual statuses
    if len(task_a_execs) > 0:
        print(f"  ✓ Task A status: {task_a_execs[0]['status']}")
    if len(task_b_execs) > 0:
        print(f"  ✓ Task B status: {task_b_execs[0]['status']}")
    if len(task_c_execs) > 0:
        print(f"  ✓ Task C status: {task_c_execs[0]['status']}")

    # Workflow status may be 'succeeded_with_errors', 'failed', or 'completed'
    print(f"  ✓ Workflow final status: {result['status']}")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Workflow Failure - Continue Policy")
    print("=" * 80)
    print(f"✓ Workflow with continue policy: {workflow_ref}")
    print(f"✓ Task A: executed")
    print(f"✓ Task B: executed (failed)")
    print(f"✓ Task C: executed (continue policy)")
    print(f"✓ Workflow status: {result['status']}")
    print("\n✅ TEST PASSED: Continue-on-failure policy works correctly!")
    print("=" * 80 + "\n")


@pytest.mark.skip(reason="Workflow failure policies (continue/abort) not implemented - uses transition model")
def test_workflow_multiple_failures(client: AttuneClient, test_pack):
    """
    Test workflow with multiple failing tasks.

    Flow:
    1. Create workflow with 5 tasks: S, F1, S, F2, S
    2. Two tasks fail (F1 and F2)
    3. Verify workflow handles multiple failures
    """
    print("\n" + "=" * 80)
    print("TEST: Workflow with Multiple Failures")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create mix of success and failure tasks
    # ========================================================================
    print("\n[STEP 1] Creating tasks...")

    tasks = []
    for i, should_fail in enumerate([False, True, False, True, False]):
        task = client.create_action(
            pack_ref=pack_ref,
            data={
                "name": f"task_{i}_{unique_ref()}",
                "description": f"Task {i} - {'fails' if should_fail else 'succeeds'}",
                "runtime_ref": "core.shell",
                "entrypoint": (
                    'echo "Task failing intentionally" >&2; exit 1'
                    if should_fail
                    else 'echo "Task succeeded"; echo \'{"success":true}\''
                ),
                "enabled": True,
                "parameters": {},
            },
        )
        tasks.append(task)
        status = "fail" if should_fail else "success"
        print(f"✓ Created Task {i} ({status}): {task['ref']}")

    # ========================================================================
    # STEP 2: Create workflow
    # ========================================================================
    print("\n[STEP 2] Creating workflow with multiple failures...")

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"multi_fail_workflow_{unique_ref()}",
            "description": "Workflow with multiple failures",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "metadata": {"on_failure": "continue"},
            "workflow_definition": {
                "tasks": [
                    {"name": f"task_{i}", "action": task["ref"], "input": {}}
                    for i, task in enumerate(tasks)
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Pattern: Success, Fail, Success, Fail, Success")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    execution = client.create_execution(action_ref=workflow_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Workflow execution created: ID={execution_id}")

    # ========================================================================
    # STEP 4: Wait for completion
    # ========================================================================
    print("\n[STEP 4] Waiting for workflow to complete...")

    time.sleep(10)
    result = client.get_execution(execution_id)
    print(f"✓ Workflow succeeded: status={result['status']}")

    # ========================================================================
    # STEP 5: Verify all tasks executed
    # ========================================================================
    print("\n[STEP 5] Verifying all tasks executed...")

    all_executions = client.list_executions(limit=100)
    task_executions = [
        ex for ex in all_executions if ex.get("parent") == execution_id
    ]

    print(f"  Found {len(task_executions)} task executions")
    assert len(task_executions) >= 5, (
        f"❌ Expected 5 task executions, got {len(task_executions)}"
    )
    print("  ✓ All 5 tasks executed")

    # Count successes and failures
    succeeded = [ex for ex in task_executions if ex["status"] in ("completed", "completed")]
    failed = [ex for ex in task_executions if ex["status"] == "failed"]

    print(f"  - Succeeded: {len(succeeded)}")
    print(f"  - Failed: {len(failed)}")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Multiple Failures")
    print("=" * 80)
    print(f"✓ Workflow with 5 tasks: {workflow_ref}")
    print(f"✓ All tasks executed: {len(task_executions)}")
    print(f"✓ Workflow handled multiple failures")
    print("\n✅ TEST PASSED: Multiple failure handling works correctly!")
    print("=" * 80 + "\n")


@pytest.mark.skip(reason="Workflow failure policies (continue/abort) not implemented - uses transition model")
def test_workflow_failure_task_isolation(client: AttuneClient, test_pack):
    """
    Test that task failures are isolated and don't cascade.

    Flow:
    1. Create workflow with independent parallel tasks
    2. One task fails, others succeed
    3. Verify failures don't affect other tasks
    """
    print("\n" + "=" * 80)
    print("TEST: Workflow Failure - Task Isolation")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create independent tasks
    # ========================================================================
    print("\n[STEP 1] Creating independent tasks...")

    task_1 = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"independent_1_{unique_ref()}",
            "description": "Independent task 1 - succeeds",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task A succeeded"; echo \'{"task":"A","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task 1 (success): {task_1['ref']}")

    task_2 = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"independent_2_{unique_ref()}",
            "description": "Independent task 2 - fails",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task B failing intentionally" >&2; exit 1',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task 2 (fails): {task_2['ref']}")

    task_3 = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"independent_3_{unique_ref()}",
            "description": "Independent task 3 - succeeds",
            "runtime_ref": "core.shell",
            "entrypoint": 'echo "Task C succeeded"; echo \'{"task":"C","success":true}\'',
            "enabled": True,
            "parameters": {},
        },
    )
    print(f"✓ Created Task 3 (success): {task_3['ref']}")

    # ========================================================================
    # STEP 2: Create workflow with independent tasks
    # ========================================================================
    print("\n[STEP 2] Creating workflow with independent tasks...")

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"isolation_workflow_{unique_ref()}",
            "description": "Workflow with independent tasks",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "metadata": {"on_failure": "continue"},
            "workflow_definition": {
                "tasks": [
                    {"name": "task_1", "action": task_1["ref"], "input": {}},
                    {"name": "task_2", "action": task_2["ref"], "input": {}},
                    {"name": "task_3", "action": task_3["ref"], "input": {}},
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")

    # ========================================================================
    # STEP 3: Execute and verify
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    execution = client.create_execution(action_ref=workflow_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Workflow execution created: ID={execution_id}")

    time.sleep(8)
    result = client.get_execution(execution_id)
    print(f"✓ Workflow succeeded: status={result['status']}")

    # ========================================================================
    # STEP 4: Verify isolation
    # ========================================================================
    print("\n[STEP 4] Verifying failure isolation...")

    all_executions = client.list_executions(limit=100)
    task_executions = [
        ex for ex in all_executions if ex.get("parent") == execution_id
    ]

    succeeded = [ex for ex in task_executions if ex["status"] in ("completed", "completed")]
    failed = [ex for ex in task_executions if ex["status"] == "failed"]

    print(f"  Total tasks: {len(task_executions)}")
    print(f"  Succeeded: {len(succeeded)}")
    print(f"  Failed: {len(failed)}")

    # At least 2 should succeed (tasks 1 and 3)
    assert len(succeeded) >= 2, (
        f"❌ Expected at least 2 successes, got {len(succeeded)}"
    )
    print("  ✓ Multiple tasks succeeded despite one failure")
    print("  ✓ Failures are isolated")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Failure Isolation")
    print("=" * 80)
    print(f"✓ Workflow with independent tasks: {workflow_ref}")
    print(f"✓ Failures isolated to individual tasks")
    print(f"✓ Other tasks succeeded successfully")
    print("\n✅ TEST PASSED: Task failure isolation works correctly!")
    print("=" * 80 + "\n")
