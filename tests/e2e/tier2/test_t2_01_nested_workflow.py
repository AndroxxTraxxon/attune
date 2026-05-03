"""
T2.1: Nested Workflow Execution

Tests that parent workflows can call child workflows, creating a proper
execution hierarchy with correct parent-child relationships.

Test validates:
- Multi-level execution hierarchy (parent → child → grandchildren)
- parent chains are correct
- Execution tree structure is maintained
- Results propagate up from children to parent
- Parent waits for all descendants to complete
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, unique_ref
from helpers.polling import (
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.skip(reason="Nested workflow orchestration timing out - needs investigation")
def test_nested_workflow_execution(client: AttuneClient, test_pack):
    """
    Test that workflows can call child workflows, creating proper execution hierarchy.

    Execution tree:
        Parent Workflow (execution_id=1)
        └─ Child Workflow (execution_id=2, parent=1)
           ├─ Task 1 (execution_id=3, parent=2)
           └─ Task 2 (execution_id=4, parent=2)
    """
    print("\n" + "=" * 80)
    print("TEST: Nested Workflow Execution (T2.1)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create child actions that will be called by child workflow
    # ========================================================================
    print("\n[STEP 1] Creating child actions...")

    task1_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_name=f"task1_{unique_ref()}",
        echo_message="Task 1 executed",
    )
    print(f"✓ Created task1 action: {task1_action['ref']}")

    task2_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_name=f"task2_{unique_ref()}",
        echo_message="Task 2 executed",
    )
    print(f"✓ Created task2 action: {task2_action['ref']}")

    # ========================================================================
    # STEP 2: Create child workflow action (calls task1 and task2)
    # ========================================================================
    print("\n[STEP 2] Creating child workflow action...")

    child_workflow_action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"child_workflow_{unique_ref()}",
            "description": "Child workflow with 2 tasks",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "child_task_1",
                        "action": task1_action["ref"],
                        "input": {},
                    },
                    {
                        "name": "child_task_2",
                        "action": task2_action["ref"],
                        "input": {},
                    },
                ]
            },
        },
    )
    child_workflow_ref = child_workflow_action["ref"]
    print(f"✓ Created child workflow: {child_workflow_ref}")
    print(f"  - Tasks: child_task_1, child_task_2")

    # ========================================================================
    # STEP 3: Create parent workflow action (calls child workflow)
    # ========================================================================
    print("\n[STEP 3] Creating parent workflow action...")

    parent_workflow_action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"parent_workflow_{unique_ref()}",
            "description": "Parent workflow that calls child workflow",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "call_child_workflow",
                        "action": child_workflow_ref,
                        "input": {},
                    }
                ]
            },
        },
    )
    parent_workflow_ref = parent_workflow_action["ref"]
    print(f"✓ Created parent workflow: {parent_workflow_ref}")
    print(f"  - Calls: {child_workflow_ref}")

    # ========================================================================
    # STEP 4: Execute parent workflow
    # ========================================================================
    print("\n[STEP 4] Executing parent workflow...")

    parent_execution = client.create_execution(
        action_ref=parent_workflow_ref, parameters={}
    )
    parent = parent_execution["id"]
    print(f"✓ Parent execution created: ID={parent}")

    # ========================================================================
    # STEP 5: Wait for parent to complete
    # ========================================================================
    print("\n[STEP 5] Waiting for parent workflow to complete...")

    parent_result = wait_for_execution_status(
        client=client,
        execution_id=parent,
        expected_status="completed",
        timeout=30,
    )
    print(f"✓ Parent workflow succeeded: status={parent_result['status']}")

    # ========================================================================
    # STEP 6: Verify execution hierarchy
    # ========================================================================
    print("\n[STEP 6] Verifying execution hierarchy...")

    # Get all executions for this test
    all_executions = client.list_executions(limit=100)

    # Filter to our executions (parent and children)
    our_executions = [
        ex
        for ex in all_executions
        if ex["id"] == parent
        or ex.get("parent") == parent
    ]

    print(f"  Found {len(our_executions)} total executions")

    # Build execution tree
    parent_exec = None
    child_workflow_exec = None
    grandchild_execs = []

    for ex in our_executions:
        if ex["id"] == parent:
            parent_exec = ex
        elif ex.get("parent") == parent:
            # This is the child workflow execution
            child_workflow_exec = ex

    assert parent_exec is not None, "Parent execution not found"
    assert child_workflow_exec is not None, "Child workflow execution not found"

    print(f"\n  Execution Tree:")
    print(f"  └─ Parent (ID={parent_exec['id']}, status={parent_exec['status']})")
    print(
        f"     └─ Child Workflow (ID={child_workflow_exec['id']}, parent={child_workflow_exec.get('parent')}, status={child_workflow_exec['status']})"
    )

    # Find grandchildren (task executions under child workflow)
    child_workflow_id = child_workflow_exec["id"]
    grandchild_execs = [
        ex
        for ex in all_executions
        if ex.get("parent") == child_workflow_id
    ]

    print(f"     Found {len(grandchild_execs)} grandchild executions:")
    for gc in grandchild_execs:
        print(
            f"        └─ Task (ID={gc['id']}, parent={gc.get('parent')}, action={gc['action_ref']}, status={gc['status']})"
        )

    # ========================================================================
    # STEP 7: Validate success criteria
    # ========================================================================
    print("\n[STEP 7] Validating success criteria...")

    # Criterion 1: At least 3 execution levels exist
    assert parent_exec is not None, "❌ Parent execution missing"
    assert child_workflow_exec is not None, "❌ Child workflow execution missing"
    assert len(grandchild_execs) >= 2, (
        f"❌ Expected at least 2 grandchild executions, got {len(grandchild_execs)}"
    )
    print("  ✓ 3 execution levels exist: parent → child → grandchildren")

    # Criterion 2: parent chain is correct
    assert child_workflow_exec["parent"] == parent, (
        f"❌ Child workflow parent_id incorrect: expected {parent}, got {child_workflow_exec['parent']}"
    )
    print(f"  ✓ Child workflow parent = {parent}")

    for gc in grandchild_execs:
        assert gc["parent"] == child_workflow_id, (
            f"❌ Grandchild parent_id incorrect: expected {child_workflow_id}, got {gc['parent']}"
        )
    print(f"  ✓ All grandchildren have parent = {child_workflow_id}")

    # Criterion 3: All executions succeeded successfully
    assert parent_exec["status"] in ("completed", "completed"), (
        f"❌ Parent status not succeeded: {parent_exec['status']}"
    )
    assert child_workflow_exec["status"] in ("completed", "completed"), (
        f"❌ Child workflow status not succeeded: {child_workflow_exec['status']}"
    )

    for gc in grandchild_execs:
        assert gc["status"] in ("completed", "completed"), (
            f"❌ Grandchild {gc['id']} status not succeeded: {gc['status']}"
        )
    print("  ✓ All executions succeeded successfully")

    # Criterion 4: Verify execution tree structure
    # Parent should have started first, then child, then grandchildren
    parent_start = parent_exec.get("start_timestamp")
    child_start = child_workflow_exec.get("start_timestamp")

    if parent_start and child_start:
        assert child_start >= parent_start, "❌ Child started before parent"
        print(f"  ✓ Execution order correct: parent started before child")

    # Criterion 5: Verify all task executions reference correct actions
    task_refs = {gc["action_ref"] for gc in grandchild_execs}
    expected_refs = {task1_action["ref"], task2_action["ref"]}

    assert task_refs == expected_refs, (
        f"❌ Task action refs don't match: expected {expected_refs}, got {task_refs}"
    )
    print(f"  ✓ All task actions executed correctly")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Nested Workflow Execution")
    print("=" * 80)
    print(f"✓ Parent workflow executed: {parent_workflow_ref}")
    print(f"✓ Child workflow executed: {child_workflow_ref}")
    print(f"✓ Execution hierarchy validated:")
    print(f"  - Parent execution ID: {parent}")
    print(f"  - Child workflow execution ID: {child_workflow_id}")
    print(f"  - Grandchild executions: {len(grandchild_execs)}")
    print(f"✓ All {1 + 1 + len(grandchild_execs)} executions succeeded")
    print(f"✓ parent chains correct")
    print(f"✓ Execution tree structure maintained")
    print("\n✅ TEST PASSED: Nested workflow execution works correctly!")
    print("=" * 80 + "\n")


@pytest.mark.skip(reason="Nested workflow orchestration timing out - needs investigation")
def test_deeply_nested_workflow(client: AttuneClient, test_pack):
    """
    Test deeper nesting: 3 levels of workflows (great-grandchildren).

    Execution tree:
        Level 0: Root Workflow
        └─ Level 1: Child Workflow
           └─ Level 2: Grandchild Workflow
              └─ Level 3: Task Action
    """
    print("\n" + "=" * 80)
    print("TEST: Deeply Nested Workflow (3 Levels)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create leaf action (level 3)
    # ========================================================================
    print("\n[STEP 1] Creating leaf action...")

    leaf_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_name=f"leaf_{unique_ref()}",
        echo_message="Leaf action at level 3",
    )
    print(f"✓ Created leaf action: {leaf_action['ref']}")

    # ========================================================================
    # STEP 2: Create grandchild workflow (level 2)
    # ========================================================================
    print("\n[STEP 2] Creating grandchild workflow (level 2)...")

    grandchild_workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"grandchild_wf_{unique_ref()}",
            "description": "Grandchild workflow (level 2)",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "call_leaf",
                        "action": leaf_action["ref"],
                        "input": {},
                    }
                ]
            },
        },
    )
    print(f"✓ Created grandchild workflow: {grandchild_workflow['ref']}")

    # ========================================================================
    # STEP 3: Create child workflow (level 1)
    # ========================================================================
    print("\n[STEP 3] Creating child workflow (level 1)...")

    child_workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"child_wf_{unique_ref()}",
            "description": "Child workflow (level 1)",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "call_grandchild",
                        "action": grandchild_workflow["ref"],
                        "input": {},
                    }
                ]
            },
        },
    )
    print(f"✓ Created child workflow: {child_workflow['ref']}")

    # ========================================================================
    # STEP 4: Create root workflow (level 0)
    # ========================================================================
    print("\n[STEP 4] Creating root workflow (level 0)...")

    root_workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"root_wf_{unique_ref()}",
            "description": "Root workflow (level 0)",
            "runner_type": "workflow",
            "entrypoint": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "call_child",
                        "action": child_workflow["ref"],
                        "input": {},
                    }
                ]
            },
        },
    )
    print(f"✓ Created root workflow: {root_workflow['ref']}")

    # ========================================================================
    # STEP 5: Execute root workflow
    # ========================================================================
    print("\n[STEP 5] Executing root workflow...")

    root_execution = client.create_execution(
        action_ref=root_workflow["ref"], parameters={}
    )
    root_execution_id = root_execution["id"]
    print(f"✓ Root execution created: ID={root_execution_id}")

    # ========================================================================
    # STEP 6: Wait for completion
    # ========================================================================
    print("\n[STEP 6] Waiting for all nested workflows to complete...")

    root_result = wait_for_execution_status(
        client=client,
        execution_id=root_execution_id,
        expected_status="completed",
        timeout=40,
    )
    print(f"✓ Root workflow succeeded: status={root_result['status']}")

    # ========================================================================
    # STEP 7: Verify 4-level hierarchy
    # ========================================================================
    print("\n[STEP 7] Verifying 4-level execution hierarchy...")

    all_executions = client.list_executions(limit=100)

    # Build hierarchy by following parent chain
    def find_children(parent_id):
        return [
            ex for ex in all_executions if ex.get("parent") == parent_id
        ]

    level0 = [ex for ex in all_executions if ex["id"] == root_execution_id][0]
    level1 = find_children(level0["id"])
    level2 = []
    for l1 in level1:
        level2.extend(find_children(l1["id"]))
    level3 = []
    for l2 in level2:
        level3.extend(find_children(l2["id"]))

    print(f"\n  Execution Hierarchy:")
    print(f"  Level 0 (Root):       {len([level0])} execution")
    print(f"  Level 1 (Child):      {len(level1)} execution(s)")
    print(f"  Level 2 (Grandchild): {len(level2)} execution(s)")
    print(f"  Level 3 (Leaf):       {len(level3)} execution(s)")

    # ========================================================================
    # STEP 8: Validate success criteria
    # ========================================================================
    print("\n[STEP 8] Validating success criteria...")

    assert len(level1) >= 1, (
        f"❌ Expected at least 1 level 1 execution, got {len(level1)}"
    )
    assert len(level2) >= 1, (
        f"❌ Expected at least 1 level 2 execution, got {len(level2)}"
    )
    assert len(level3) >= 1, (
        f"❌ Expected at least 1 level 3 execution, got {len(level3)}"
    )
    print("  ✓ All 4 execution levels present")

    # Verify all succeeded
    all_execs = [level0] + level1 + level2 + level3
    for ex in all_execs:
        assert ex["status"] in ("completed", "completed"), (
            f"❌ Execution {ex['id']} failed: {ex['status']}"
        )
    print(f"  ✓ All {len(all_execs)} executions succeeded")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Deeply Nested Workflow (3 Levels)")
    print("=" * 80)
    print(f"✓ 4-level execution hierarchy created:")
    print(f"  - Root workflow (level 0)")
    print(f"  - Child workflow (level 1)")
    print(f"  - Grandchild workflow (level 2)")
    print(f"  - Leaf action (level 3)")
    print(f"✓ Total executions: {len(all_execs)}")
    print(f"✓ All executions succeeded")
    print(f"✓ parent chain validated")
    print("\n✅ TEST PASSED: Deep nesting works correctly!")
    print("=" * 80 + "\n")
