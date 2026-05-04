"""
T2.2: Workflow Failure Handling

Tests that workflows handle child task failures using the canonical `next`
transition model.
"""

import pytest
from helpers import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def _create_shell_action(
    client: AttuneClient,
    pack_ref: str,
    name_prefix: str,
    *,
    succeeds: bool = True,
) -> dict:
    if succeeds:
        entrypoint = (
            f"echo '{name_prefix} succeeded'; "
            f"printf '{{\"task\":\"{name_prefix}\",\"success\":true}}\\n'"
        )
    else:
        entrypoint = f"echo '{name_prefix} failing intentionally' >&2; exit 1"

    return client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"{name_prefix}_{unique_ref()}",
            "description": f"{name_prefix} {'success' if succeeds else 'failure'} action",
            "runtime_ref": "core.shell",
            "entrypoint": entrypoint,
            "enabled": True,
            "parameters": {},
        },
    )


def _child_executions(client: AttuneClient, parent_id: int) -> list[dict]:
    return [
        execution
        for execution in client.list_executions(parent=parent_id, limit=100)
        if execution.get("parent") == parent_id
    ]


def _children_for_action(children: list[dict], action_ref: str) -> list[dict]:
    return [execution for execution in children if execution["action_ref"] == action_ref]


def test_workflow_failure_blocks_success_transition(client: AttuneClient, test_pack):
    """
    A failed task must not follow a success-only transition.
    """
    pack_ref = test_pack["ref"]

    task_a = _create_shell_action(client, pack_ref, "task_a_success", succeeds=True)
    task_b = _create_shell_action(client, pack_ref, "task_b_fail", succeeds=False)
    task_c = _create_shell_action(client, pack_ref, "task_c_skipped", succeeds=True)

    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"failure_transition_{unique_ref()}",
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

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="failed",
        timeout=20,
    )

    children = _child_executions(client, execution["id"])
    task_a_execs = _children_for_action(children, task_a["ref"])
    task_b_execs = _children_for_action(children, task_b["ref"])
    task_c_execs = _children_for_action(children, task_c["ref"])

    assert result["status"] == "failed"
    assert len(task_a_execs) == 1
    assert task_a_execs[0]["status"] == "completed"
    assert len(task_b_execs) == 1
    assert task_b_execs[0]["status"] == "failed"
    assert task_c_execs == []


def test_workflow_failure_recovery_transition(client: AttuneClient, test_pack):
    """
    A failed task can continue the workflow by following an explicit failed()
    transition to a recovery task.
    """
    pack_ref = test_pack["ref"]

    task_a = _create_shell_action(client, pack_ref, "recover_a", succeeds=True)
    task_b = _create_shell_action(client, pack_ref, "recover_b_fail", succeeds=False)
    recovery = _create_shell_action(client, pack_ref, "recover_c", succeeds=True)

    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"failure_recovery_{unique_ref()}",
        label="Workflow Failure Recovery",
        description="Workflow where failed() transitions recover from failure",
        tasks=[
            {
                "name": "task_a",
                "action": task_a["ref"],
                "next": [{"when": "{{ succeeded() }}", "do": ["task_b"]}],
            },
            {
                "name": "task_b",
                "action": task_b["ref"],
                "next": [{"when": "{{ failed() }}", "do": ["recovery"]}],
            },
            {"name": "recovery", "action": recovery["ref"]},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=20,
    )

    children = _child_executions(client, execution["id"])
    task_a_execs = _children_for_action(children, task_a["ref"])
    task_b_execs = _children_for_action(children, task_b["ref"])
    recovery_execs = _children_for_action(children, recovery["ref"])

    assert result["status"] == "completed"
    assert len(task_a_execs) == 1
    assert task_a_execs[0]["status"] == "completed"
    assert len(task_b_execs) == 1
    assert task_b_execs[0]["status"] == "failed"
    assert len(recovery_execs) == 1
    assert recovery_execs[0]["status"] == "completed"


def test_workflow_multiple_failures_recovered(client: AttuneClient, test_pack):
    """
    Multiple failures can be handled independently by failed() transitions.
    """
    pack_ref = test_pack["ref"]

    success_1 = _create_shell_action(client, pack_ref, "multi_success_1", succeeds=True)
    failure_1 = _create_shell_action(client, pack_ref, "multi_failure_1", succeeds=False)
    success_2 = _create_shell_action(client, pack_ref, "multi_success_2", succeeds=True)
    failure_2 = _create_shell_action(client, pack_ref, "multi_failure_2", succeeds=False)
    success_3 = _create_shell_action(client, pack_ref, "multi_success_3", succeeds=True)

    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"multi_failure_recovery_{unique_ref()}",
        label="Workflow Multiple Failure Recovery",
        tasks=[
            {
                "name": "success_1",
                "action": success_1["ref"],
                "next": [{"when": "{{ succeeded() }}", "do": ["failure_1"]}],
            },
            {
                "name": "failure_1",
                "action": failure_1["ref"],
                "next": [{"when": "{{ failed() }}", "do": ["success_2"]}],
            },
            {
                "name": "success_2",
                "action": success_2["ref"],
                "next": [{"when": "{{ succeeded() }}", "do": ["failure_2"]}],
            },
            {
                "name": "failure_2",
                "action": failure_2["ref"],
                "next": [{"when": "{{ failed() }}", "do": ["success_3"]}],
            },
            {"name": "success_3", "action": success_3["ref"]},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=30,
    )

    children = _child_executions(client, execution["id"])
    assert result["status"] == "completed"
    assert len(children) == 5
    assert len([child for child in children if child["status"] == "completed"]) == 3
    assert len([child for child in children if child["status"] == "failed"]) == 2


def test_workflow_parallel_task_failure_isolation(client: AttuneClient, test_pack):
    """
    Independent entry-point tasks should all dispatch even when one branch fails.
    """
    pack_ref = test_pack["ref"]

    success_1 = _create_shell_action(client, pack_ref, "parallel_success_1", succeeds=True)
    failure = _create_shell_action(client, pack_ref, "parallel_failure", succeeds=False)
    success_2 = _create_shell_action(client, pack_ref, "parallel_success_2", succeeds=True)

    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"parallel_failure_isolation_{unique_ref()}",
        label="Workflow Parallel Failure Isolation",
        tasks=[
            {"name": "success_1", "action": success_1["ref"]},
            {"name": "failure", "action": failure["ref"]},
            {"name": "success_2", "action": success_2["ref"]},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="failed",
        timeout=20,
    )

    children = _child_executions(client, execution["id"])
    success_1_execs = _children_for_action(children, success_1["ref"])
    failure_execs = _children_for_action(children, failure["ref"])
    success_2_execs = _children_for_action(children, success_2["ref"])

    assert result["status"] == "failed"
    assert len(children) == 3
    assert success_1_execs[0]["status"] == "completed"
    assert failure_execs[0]["status"] == "failed"
    assert success_2_execs[0]["status"] == "completed"
