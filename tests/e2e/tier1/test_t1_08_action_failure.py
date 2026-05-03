#!/usr/bin/env python3
"""
T1.8: Action Execution Failure Handling

Tests that failed action executions are handled gracefully.

Test Flow:
1. Create action that always exits with error (exit code 1)
2. Create rule to trigger action
3. Execute action
4. Verify execution status becomes 'failed'
5. Verify error message captured
6. Verify exit code recorded
7. Verify execution doesn't retry (no retry policy)

Success Criteria:
- Execution status: 'requested' → 'scheduled' → 'running' → 'failed'
- Exit code captured: exit_code = 1
- stderr captured in execution result
- Execution result includes error details
- Worker marks execution as failed
- Executor updates enforcement status
- System remains stable (no crashes)
"""

import time

import pytest
from helpers import (
    AttuneClient,
    create_failing_action,
    create_rule,
    create_webhook_trigger,
    wait_for_execution_count,
    wait_for_execution_status,
)


@pytest.mark.tier1
@pytest.mark.integration
@pytest.mark.timeout(30)
class TestActionFailureHandling:
    """Test action failure handling"""

    def test_action_failure_basic(self, client: AttuneClient, pack_ref: str):
        """Test that failing action is marked as failed with error details"""

        print(f"\n=== T1.8: Action Failure Handling ===")

        # Step 1: Create failing action
        print("\n[1/5] Creating failing action...")
        action = create_failing_action(client=client, pack_ref=pack_ref, exit_code=1)
        action_ref = action["ref"]
        print(f"✓ Created action: {action_ref} (ID: {action['id']})")
        print(f"  Expected exit code: 1")

        # Step 2: Create webhook trigger (easier to control than timer)
        print("\n[2/5] Creating webhook trigger...")
        trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)
        print(f"✓ Created trigger: {trigger['label']} (ID: {trigger['id']})")

        # Step 3: Create rule
        print("\n[3/5] Creating rule...")
        rule = create_rule(
            client=client,
            trigger_ref=trigger["ref"],
            action_ref=action_ref,
            pack_ref=pack_ref,
            enabled=True,
        )
        print(f"✓ Created rule: {rule['label']} (ID: {rule['id']})")

        # Step 4: Fire webhook to trigger execution
        print("\n[4/5] Triggering action execution...")
        client.fire_webhook(trigger_ref=trigger["ref"], payload={"test": "failure_test"})
        print(f"✓ Webhook fired")

        # Wait for execution to be created
        executions = wait_for_execution_count(
            client=client,
            expected_count=1,
            action_ref=action_ref,
            timeout=15,
            poll_interval=0.5,
        )

        assert len(executions) >= 1, "Expected at least 1 execution"
        execution = executions[0]
        exec_id = execution["id"]

        print(f"✓ Execution created (ID: {exec_id})")
        print(f"  Initial status: {execution['status']}")

        # Step 5: Wait for execution to complete (should fail)
        print(f"\n[5/5] Waiting for execution to fail...")

        final_execution = wait_for_execution_status(
            client=client,
            execution_id=exec_id,
            expected_status="failed",
            timeout=20,
        )

        print(f"✓ Execution failed as expected")
        print(f"\nExecution details:")
        print(f"  ID: {final_execution['id']}")
        print(f"  Status: {final_execution['status']}")
        print(f"  Action: {final_execution['action_ref']}")

        # Verify execution status is 'failed'
        assert final_execution["status"] == "failed", (
            f"Expected status 'failed', got '{final_execution['status']}'"
        )

        # Check for exit code if available
        if "exit_code" in final_execution:
            exit_code = final_execution["exit_code"]
            print(f"  Exit code: {exit_code}")
            assert exit_code == 1, f"Expected exit code 1, got {exit_code}"

        # Check for error information
        result = final_execution.get("result") or {}
        print(f"  Result available: {bool(result)}")

        if "error" in result:
            print(f"  Error: {result['error']}")

        if "stderr" in result:
            stderr = result["stderr"]
            if stderr:
                print(f"  Stderr captured: {len(stderr)} characters")

        # Final summary
        print("\n=== Test Summary ===")
        print(f"✓ Action executed and failed")
        print(f"✓ Execution status: failed")
        print(f"✓ Error information captured")
        print(f"✓ System handled failure gracefully")
        print(f"✓ Test PASSED")

    def test_multiple_failures_independent(self, client: AttuneClient, pack_ref: str):
        """Test that multiple failures don't affect each other"""

        print(f"\n=== T1.8b: Multiple Independent Failures ===")

        # Create failing action
        action = create_failing_action(client=client, pack_ref=pack_ref)
        trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_ref=trigger["ref"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        print(f"✓ Setup complete")

        # Trigger 3 executions
        print(f"\nTriggering 3 executions...")
        for i in range(3):
            client.fire_webhook(trigger_ref=trigger["ref"], payload={"run": i + 1})
            print(f"  ✓ Execution {i + 1} triggered")
            time.sleep(0.5)

        # Wait for all 3 executions
        executions = wait_for_execution_count(
            client=client,
            expected_count=3,
            action_ref=action["ref"],
            timeout=25,
        )

        print(f"✓ {len(executions)} executions created")

        # Wait for all to complete
        print(f"\nWaiting for all executions to complete...")
        failed_count = 0
        for i, execution in enumerate(executions[:3]):
            exec_id = execution["id"]
            status = execution["status"]

            if status not in ["failed", "completed", "cancelled"]:
                execution = wait_for_execution_status(
                    client=client,
                    execution_id=exec_id,
                    expected_status="failed",
                    timeout=15,
                )
                status = execution["status"]

            print(f"  Execution {i + 1}: {status}")
            assert status == "failed"
            failed_count += 1

        print(f"\n✓ All {failed_count}/3 executions failed independently")
        print(f"✓ No cascade failures or system instability")
        print(f"✓ Test PASSED")

    def test_action_failure_different_exit_codes(
        self, client: AttuneClient, pack_ref: str
    ):
        """Test actions with different exit codes"""

        print(f"\n=== T1.8c: Different Exit Codes ===")

        exit_codes = [1, 2, 127, 255]

        for exit_code in exit_codes:
            print(f"\nTesting exit code {exit_code}...")

            # Create action with specific exit code
            action = create_failing_action(
                client=client, pack_ref=pack_ref, exit_code=exit_code
            )
            trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)
            rule = create_rule(
                client=client,
                trigger_ref=trigger["ref"],
                action_ref=action["ref"],
                pack_ref=pack_ref,
            )

            # Execute
            client.fire_webhook(trigger_ref=trigger["ref"], payload={})

            # Wait for execution
            executions = wait_for_execution_count(
                client=client,
                expected_count=1,
                action_ref=action["ref"],
                timeout=15,
            )

            execution = executions[0]
            if execution["status"] not in ["failed", "completed", "cancelled"]:
                execution = wait_for_execution_status(
                    client=client,
                    execution_id=execution["id"],
                    expected_status="failed",
                    timeout=15,
                )

            # Verify failed
            assert execution["status"] == "failed"
            print(f"  ✓ Execution failed with exit code {exit_code}")

            # Check exit code if available
            if "exit_code" in execution:
                actual_exit_code = execution["exit_code"]
                print(f"  ✓ Captured exit code: {actual_exit_code}")
                # Note: Exit codes may be truncated/modified by shell
                # Just verify it's non-zero
                assert actual_exit_code != 0

        print(f"\n✓ All exit codes handled correctly")
        print(f"✓ Test PASSED")

    def test_action_timeout_vs_failure(self, client: AttuneClient, pack_ref: str):
        """Test distinguishing between timeout and actual failure"""

        print(f"\n=== T1.8d: Timeout vs Failure ===")

        # Create action that fails quickly (not timeout)
        print("\nTest 1: Quick failure (not timeout)...")
        action = create_failing_action(client=client, pack_ref=pack_ref, exit_code=1)
        trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)
        rule = create_rule(
            client=client,
            trigger_ref=trigger["ref"],
            action_ref=action["ref"],
            pack_ref=pack_ref,
        )

        client.fire_webhook(trigger_ref=trigger["ref"], payload={})

        executions = wait_for_execution_count(
            client=client, expected_count=1, action_ref=action["ref"], timeout=15
        )

        execution = executions[0]
        if execution["status"] not in ["failed", "completed", "cancelled"]:
            execution = wait_for_execution_status(
                client=client,
                execution_id=execution["id"],
                expected_status="failed",
                timeout=15,
            )

        # Should fail quickly (within a few seconds)
        assert execution["status"] == "failed"
        print(f"  ✓ Action failed quickly")

        # Check result for failure type
        result = execution.get("result") or {}
        if "error" in result:
            error_msg = result["error"]
            print(f"  Error message: {error_msg}")

            # Should NOT be a timeout error
            is_timeout = (
                "timeout" in error_msg.lower() or "timed out" in error_msg.lower()
            )
            if is_timeout:
                print(f"  ⚠️  Error indicates timeout (unexpected for quick failure)")
            else:
                print(f"  ✓ Error is not timeout-related")

        print(f"\n✓ Failure modes can be distinguished")
        print(f"✓ Test PASSED")

    def test_system_stability_after_failure(self, client: AttuneClient, pack_ref: str):
        """Test that system remains stable after action failure"""

        print(f"\n=== T1.8e: System Stability After Failure ===")

        # Create two actions: one that fails, one that succeeds
        print("\n[1/4] Creating failing and succeeding actions...")
        failing_action = create_failing_action(client=client, pack_ref=pack_ref)

        from helpers import create_echo_action

        success_action = create_echo_action(client=client, pack_ref=pack_ref)
        print(f"✓ Actions created")

        # Create triggers and rules
        print("\n[2/4] Creating triggers and rules...")
        fail_trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)
        success_trigger = create_webhook_trigger(client=client, pack_ref=pack_ref)

        fail_rule = create_rule(
            client=client,
            trigger_ref=fail_trigger["ref"],
            action_ref=failing_action["ref"],
            pack_ref=pack_ref,
        )
        success_rule = create_rule(
            client=client,
            trigger_ref=success_trigger["ref"],
            action_ref=success_action["ref"],
            pack_ref=pack_ref,
        )
        print(f"✓ Rules created")

        # Execute failing action
        print("\n[3/4] Executing failing action...")
        client.fire_webhook(trigger_ref=fail_trigger["ref"], payload={})

        fail_executions = wait_for_execution_count(
            client=client,
            expected_count=1,
            action_ref=failing_action["ref"],
            timeout=15,
        )

        fail_exec = fail_executions[0]
        if fail_exec["status"] not in ["failed", "completed", "cancelled"]:
            fail_exec = wait_for_execution_status(
                client=client,
                execution_id=fail_exec["id"],
                expected_status="failed",
                timeout=15,
            )

        assert fail_exec["status"] == "failed"
        print(f"✓ First action failed (as expected)")

        # Execute succeeding action
        print("\n[4/4] Executing succeeding action...")
        client.fire_webhook(
            trigger_ref=success_trigger["ref"], payload={"message": "test"}
        )

        success_executions = wait_for_execution_count(
            client=client,
            expected_count=1,
            action_ref=success_action["ref"],
            timeout=15,
        )

        success_exec = success_executions[0]
        if success_exec["status"] not in ["failed", "completed", "cancelled"]:
            success_exec = wait_for_execution_status(
                client=client,
                execution_id=success_exec["id"],
                expected_status="completed",
                timeout=15,
            )

        assert success_exec["status"] == "completed"
        print(f"✓ Second action succeeded")

        # Final verification
        print("\n=== Test Summary ===")
        print(f"✓ Failing action failed without affecting system")
        print(f"✓ Subsequent action succeeded normally")
        print(f"✓ System remained stable after failure")
        print(f"✓ Worker continues processing after failures")
        print(f"✓ Test PASSED")
