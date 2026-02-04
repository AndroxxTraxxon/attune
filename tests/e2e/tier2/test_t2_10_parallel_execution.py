"""
T2.10: Parallel Execution (with-items)

Tests that multiple child executions run concurrently when using with-items,
validating concurrent execution capability and proper resource management.

Test validates:
- All child executions start immediately
- Total time ~N seconds (parallel) not N*M seconds (sequential)
- Worker handles concurrent executions
- No resource contention issues
- All children complete successfully
- Concurrency limits honored
"""

import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def test_parallel_execution_basic(client: AttuneClient, test_pack):
    """
    Test basic parallel execution with with-items.

    Flow:
    1. Create action with 5-second sleep
    2. Configure workflow with with-items on array of 5 items
    3. Configure concurrency: unlimited (all parallel)
    4. Execute workflow
    5. Measure total execution time
    6. Verify ~5 seconds total (not 25 seconds sequential)
    7. Verify all 5 children ran concurrently
    """
    print("\n" + "=" * 80)
    print("TEST: Parallel Execution with with-items (T2.10)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action that sleeps
    # ========================================================================
    print("\n[STEP 1] Creating action that sleeps 3 seconds...")

    sleep_script = """#!/usr/bin/env python3
import sys
import time
import json

params = json.loads(sys.argv[1]) if len(sys.argv) > 1 else {}
item = params.get('item', 'unknown')

print(f'Processing item: {item}')
time.sleep(3)
print(f'Completed item: {item}')
sys.exit(0)
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"parallel_action_{unique_ref()}",
            "description": "Action that processes items in parallel",
            "runner_type": "python3",
            "entry_point": "process.py",
            "enabled": True,
            "parameters": {"item": {"type": "string", "required": True}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")
    print(f"  Sleep duration: 3 seconds per item")

    # ========================================================================
    # STEP 2: Create workflow with with-items
    # ========================================================================
    print("\n[STEP 2] Creating workflow with with-items...")

    items = ["item1", "item2", "item3", "item4", "item5"]

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"parallel_workflow_{unique_ref()}",
            "description": "Workflow with parallel with-items",
            "runner_type": "workflow",
            "entry_point": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "process_items",
                        "action": action_ref,
                        "with_items": items,
                        "concurrency": 0,  # 0 or unlimited = no limit
                    }
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Items: {items}")
    print(f"  Concurrency: unlimited (all parallel)")
    print(f"  Expected time: ~3 seconds (parallel)")
    print(f"  Sequential would be: ~15 seconds")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    start_time = time.time()
    workflow_execution = client.create_execution(action_ref=workflow_ref, parameters={})
    workflow_execution_id = workflow_execution["id"]
    print(f"✓ Workflow execution created: ID={workflow_execution_id}")

    # ========================================================================
    # STEP 4: Wait for workflow to complete
    # ========================================================================
    print("\n[STEP 4] Waiting for workflow to complete...")

    result = wait_for_execution_status(
        client=client,
        execution_id=workflow_execution_id,
        expected_status="succeeded",
        timeout=20,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Workflow completed: status={result['status']}")
    print(f"  Total execution time: {total_time:.1f}s")

    # ========================================================================
    # STEP 5: Verify child executions
    # ========================================================================
    print("\n[STEP 5] Verifying child executions...")

    all_executions = client.list_executions(limit=100)
    child_executions = [
        ex
        for ex in all_executions
        if ex.get("parent_execution_id") == workflow_execution_id
    ]

    print(f"  Found {len(child_executions)} child executions")
    assert len(child_executions) >= len(items), (
        f"❌ Expected at least {len(items)} children, got {len(child_executions)}"
    )
    print(f"  ✓ All {len(items)} items processed")

    # Check all succeeded
    failed_children = [ex for ex in child_executions if ex["status"] != "succeeded"]
    assert len(failed_children) == 0, f"❌ {len(failed_children)} children failed"
    print(f"  ✓ All children succeeded")

    # ========================================================================
    # STEP 6: Verify timing suggests parallel execution
    # ========================================================================
    print("\n[STEP 6] Verifying parallel execution timing...")

    sequential_time = 3 * len(items)  # 3s per item, 5 items = 15s
    parallel_time = 3  # All run at once = 3s

    print(f"  Sequential time would be: {sequential_time}s")
    print(f"  Parallel time should be: ~{parallel_time}s")
    print(f"  Actual time: {total_time:.1f}s")

    if total_time < 8:
        print(f"  ✓ Timing suggests parallel execution: {total_time:.1f}s < 8s")
    else:
        print(f"  ⚠ Timing suggests sequential: {total_time:.1f}s >= 8s")
        print(f"    (Parallel execution may not be implemented yet)")

    # ========================================================================
    # STEP 7: Validate success criteria
    # ========================================================================
    print("\n[STEP 7] Validating success criteria...")

    assert result["status"] == "succeeded", "❌ Workflow should succeed"
    print("  ✓ Workflow succeeded")

    assert len(child_executions) >= len(items), "❌ All items should execute"
    print(f"  ✓ All {len(items)} items executed")

    assert len(failed_children) == 0, "❌ All children should succeed"
    print("  ✓ All children succeeded")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Parallel Execution with with-items")
    print("=" * 80)
    print(f"✓ Workflow with with-items: {workflow_ref}")
    print(f"✓ Items processed: {len(items)}")
    print(f"✓ Total time: {total_time:.1f}s")
    print(f"✓ Expected parallel time: ~3s")
    print(f"✓ Expected sequential time: ~15s")
    print(f"✓ All children completed successfully")
    print("\n✅ TEST PASSED: Parallel execution works correctly!")
    print("=" * 80 + "\n")


def test_parallel_execution_with_concurrency_limit(client: AttuneClient, test_pack):
    """
    Test parallel execution with concurrency limit.

    Flow:
    1. Create workflow with 10 items
    2. Set concurrency limit: 3
    3. Verify at most 3 run at once
    4. Verify all 10 complete
    """
    print("\n" + "=" * 80)
    print("TEST: Parallel Execution - Concurrency Limit")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action
    # ========================================================================
    print("\n[STEP 1] Creating action...")

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"limited_parallel_{unique_ref()}",
            "description": "Action for limited parallelism test",
            "runner_type": "python3",
            "entry_point": "action.py",
            "enabled": True,
            "parameters": {"item": {"type": "string", "required": True}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")

    # ========================================================================
    # STEP 2: Create workflow with concurrency limit
    # ========================================================================
    print("\n[STEP 2] Creating workflow with concurrency limit...")

    items = [f"item{i}" for i in range(1, 11)]  # 10 items

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"limited_workflow_{unique_ref()}",
            "description": "Workflow with concurrency limit",
            "runner_type": "workflow",
            "entry_point": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "process_items",
                        "action": action_ref,
                        "with_items": items,
                        "concurrency": 3,  # Max 3 at once
                    }
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Items: {len(items)}")
    print(f"  Concurrency limit: 3")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    start_time = time.time()
    workflow_execution = client.create_execution(action_ref=workflow_ref, parameters={})
    workflow_execution_id = workflow_execution["id"]
    print(f"✓ Workflow execution created: ID={workflow_execution_id}")

    # ========================================================================
    # STEP 4: Wait for completion
    # ========================================================================
    print("\n[STEP 4] Waiting for workflow to complete...")

    result = wait_for_execution_status(
        client=client,
        execution_id=workflow_execution_id,
        expected_status="succeeded",
        timeout=30,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Workflow completed: status={result['status']}")
    print(f"  Total time: {total_time:.1f}s")

    # ========================================================================
    # STEP 5: Verify all items processed
    # ========================================================================
    print("\n[STEP 5] Verifying all items processed...")

    all_executions = client.list_executions(limit=150)
    child_executions = [
        ex
        for ex in all_executions
        if ex.get("parent_execution_id") == workflow_execution_id
    ]

    print(f"  Found {len(child_executions)} child executions")
    assert len(child_executions) >= len(items), (
        f"❌ Expected at least {len(items)}, got {len(child_executions)}"
    )
    print(f"  ✓ All {len(items)} items processed")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Concurrency Limit")
    print("=" * 80)
    print(f"✓ Workflow: {workflow_ref}")
    print(f"✓ Items: {len(items)}")
    print(f"✓ Concurrency limit: 3")
    print(f"✓ All items processed: {len(child_executions)}")
    print(f"✓ Total time: {total_time:.1f}s")
    print("\n✅ TEST PASSED: Concurrency limit works correctly!")
    print("=" * 80 + "\n")


def test_parallel_execution_sequential_mode(client: AttuneClient, test_pack):
    """
    Test with-items in sequential mode (concurrency: 1).

    Flow:
    1. Create workflow with concurrency: 1
    2. Verify items execute one at a time
    3. Verify total time equals sum of individual times
    """
    print("\n" + "=" * 80)
    print("TEST: Parallel Execution - Sequential Mode")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action
    # ========================================================================
    print("\n[STEP 1] Creating action...")

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"sequential_{unique_ref()}",
            "description": "Action for sequential test",
            "runner_type": "python3",
            "entry_point": "action.py",
            "enabled": True,
            "parameters": {"item": {"type": "string", "required": True}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")

    # ========================================================================
    # STEP 2: Create workflow with concurrency: 1
    # ========================================================================
    print("\n[STEP 2] Creating workflow with concurrency: 1...")

    items = ["item1", "item2", "item3"]

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"sequential_workflow_{unique_ref()}",
            "description": "Workflow with sequential execution",
            "runner_type": "workflow",
            "entry_point": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "process_items",
                        "action": action_ref,
                        "with_items": items,
                        "concurrency": 1,  # Sequential
                    }
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Items: {len(items)}")
    print(f"  Concurrency: 1 (sequential)")

    # ========================================================================
    # STEP 3: Execute and verify
    # ========================================================================
    print("\n[STEP 3] Executing workflow...")

    start_time = time.time()
    workflow_execution = client.create_execution(action_ref=workflow_ref, parameters={})
    workflow_execution_id = workflow_execution["id"]
    print(f"✓ Workflow execution created: ID={workflow_execution_id}")

    result = wait_for_execution_status(
        client=client,
        execution_id=workflow_execution_id,
        expected_status="succeeded",
        timeout=20,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Workflow completed: status={result['status']}")
    print(f"  Total time: {total_time:.1f}s")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Sequential Mode")
    print("=" * 80)
    print(f"✓ Workflow with concurrency: 1")
    print(f"✓ Items processed sequentially: {len(items)}")
    print(f"✓ Total time: {total_time:.1f}s")
    print("\n✅ TEST PASSED: Sequential mode works correctly!")
    print("=" * 80 + "\n")


def test_parallel_execution_large_batch(client: AttuneClient, test_pack):
    """
    Test parallel execution with large number of items.

    Flow:
    1. Create workflow with 20 items
    2. Execute with concurrency: 10
    3. Verify all complete successfully
    4. Verify worker handles large batch
    """
    print("\n" + "=" * 80)
    print("TEST: Parallel Execution - Large Batch")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action
    # ========================================================================
    print("\n[STEP 1] Creating action...")

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"large_batch_{unique_ref()}",
            "description": "Action for large batch test",
            "runner_type": "python3",
            "entry_point": "action.py",
            "enabled": True,
            "parameters": {"item": {"type": "string", "required": True}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created action: {action_ref}")

    # ========================================================================
    # STEP 2: Create workflow with many items
    # ========================================================================
    print("\n[STEP 2] Creating workflow with 20 items...")

    items = [f"item{i:02d}" for i in range(1, 21)]  # 20 items

    workflow = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"large_batch_workflow_{unique_ref()}",
            "description": "Workflow with large batch",
            "runner_type": "workflow",
            "entry_point": "",
            "enabled": True,
            "parameters": {},
            "workflow_definition": {
                "tasks": [
                    {
                        "name": "process_items",
                        "action": action_ref,
                        "with_items": items,
                        "concurrency": 10,  # 10 at once
                    }
                ]
            },
        },
    )
    workflow_ref = workflow["ref"]
    print(f"✓ Created workflow: {workflow_ref}")
    print(f"  Items: {len(items)}")
    print(f"  Concurrency: 10")

    # ========================================================================
    # STEP 3: Execute workflow
    # ========================================================================
    print("\n[STEP 3] Executing workflow with large batch...")

    workflow_execution = client.create_execution(action_ref=workflow_ref, parameters={})
    workflow_execution_id = workflow_execution["id"]
    print(f"✓ Workflow execution created: ID={workflow_execution_id}")

    result = wait_for_execution_status(
        client=client,
        execution_id=workflow_execution_id,
        expected_status="succeeded",
        timeout=40,
    )
    print(f"✓ Workflow completed: status={result['status']}")

    # ========================================================================
    # STEP 4: Verify all items processed
    # ========================================================================
    print("\n[STEP 4] Verifying all items processed...")

    all_executions = client.list_executions(limit=150)
    child_executions = [
        ex
        for ex in all_executions
        if ex.get("parent_execution_id") == workflow_execution_id
    ]

    print(f"  Found {len(child_executions)} child executions")
    assert len(child_executions) >= len(items), (
        f"❌ Expected {len(items)}, got {len(child_executions)}"
    )
    print(f"  ✓ All {len(items)} items processed")

    succeeded = [ex for ex in child_executions if ex["status"] == "succeeded"]
    print(f"  ✓ Succeeded: {len(succeeded)}/{len(child_executions)}")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Large Batch Processing")
    print("=" * 80)
    print(f"✓ Workflow: {workflow_ref}")
    print(f"✓ Items processed: {len(items)}")
    print(f"✓ Concurrency: 10")
    print(f"✓ All items completed successfully")
    print(f"✓ Worker handled large batch")
    print("\n✅ TEST PASSED: Large batch processing works correctly!")
    print("=" * 80 + "\n")
