"""T3.9: workflow-pausing inquiry action contracts.

The direct inquiry API works today. These tests specify the missing workflow
integration contract: a built-in inquiry action pauses a workflow task until a
human response resumes it.
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, unique_ref
from helpers.polling import wait_for_execution_status


pytestmark = [
    pytest.mark.tier3,
    pytest.mark.inquiry,
    pytest.mark.workflow,
    pytest.mark.orchestration,
]


def _assert_core_ask_available(client: AttuneClient) -> None:
    action_refs = {action["ref"] for action in client.list_actions(limit=500)}
    assert "core.ask" in action_refs, "core.ask must be the built-in workflow inquiry action"


def _child_executions(client: AttuneClient, parent_id: int) -> list[dict]:
    return [
        execution
        for execution in client.list_executions(parent=parent_id, limit=100)
        if execution.get("parent") == parent_id
    ]


def _task_child(client: AttuneClient, parent_id: int, task_name: str) -> dict:
    deadline = time.time() + 10
    while time.time() < deadline:
        matches = [
            child
            for child in _child_executions(client, parent_id)
            if (child.get("workflow_task") or {}).get("task_name") == task_name
        ]
        if len(matches) == 1:
            return matches[0]
        time.sleep(0.25)
    assert False, f"Expected exactly one child execution for task {task_name!r}"


def _pending_inquiry_for_execution(client: AttuneClient, execution_id: int) -> dict:
    deadline = time.time() + 10
    while time.time() < deadline:
        matches = [
            inquiry
            for inquiry in client.list_inquiries(limit=500, status="pending")
            if inquiry.get("execution") == execution_id
        ]
        if len(matches) == 1:
            return matches[0]
        time.sleep(0.25)
    assert False, f"Expected one pending inquiry for execution {execution_id}"


def test_sequential_multi_step_approvals(client: AttuneClient, test_pack):
    """Each approval response should resume the workflow and create the next gate."""
    _assert_core_ask_available(client)
    pack_ref = test_pack["ref"]
    final_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"approval_final_{unique_ref()}",
        description="Final action after all approval gates",
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"sequential_approvals_{unique_ref()}",
        label="Sequential Approval Workflow",
        description="Three inquiry gates followed by one final action",
        tasks=[
            {
                "name": "manager_approval",
                "action": "core.ask",
                "input": {
                    "prompt": "Manager approval required",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [{"when": "{{ result().response.approved == true }}", "do": ["director_approval"]}],
            },
            {
                "name": "director_approval",
                "action": "core.ask",
                "input": {
                    "prompt": "Director approval required",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [{"when": "{{ result().response.approved == true }}", "do": ["vp_approval"]}],
            },
            {
                "name": "vp_approval",
                "action": "core.ask",
                "input": {
                    "prompt": "VP approval required",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [{"when": "{{ result().response.approved == true }}", "do": ["execute_deployment"]}],
            },
            {
                "name": "execute_deployment",
                "action": final_action["ref"],
                "input": {"message": "all approvals received"},
            },
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})

    manager_task = _task_child(client, execution["id"], "manager_approval")
    manager_inquiry = _pending_inquiry_for_execution(client, manager_task["id"])
    client.respond_to_inquiry(manager_inquiry["id"], {"approved": True, "comment": "manager"})

    director_task = _task_child(client, execution["id"], "director_approval")
    director_inquiry = _pending_inquiry_for_execution(client, director_task["id"])
    client.respond_to_inquiry(director_inquiry["id"], {"approved": True, "comment": "director"})

    vp_task = _task_child(client, execution["id"], "vp_approval")
    vp_inquiry = _pending_inquiry_for_execution(client, vp_task["id"])
    client.respond_to_inquiry(vp_inquiry["id"], {"approved": True, "comment": "vp"})

    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=30,
    )
    final_task = _task_child(client, execution["id"], "execute_deployment")
    assert result["status"] == "completed"
    assert final_task["status"] == "completed"


def test_conditional_approval_workflow(client: AttuneClient, test_pack):
    """Inquiry response data should drive Orquesta-style conditional branches."""
    _assert_core_ask_available(client)
    pack_ref = test_pack["ref"]
    approved_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"approval_path_{unique_ref()}",
        description="Runs only when approved",
    )
    denied_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"denial_path_{unique_ref()}",
        description="Runs only when denied",
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"conditional_approval_{unique_ref()}",
        label="Conditional Approval Workflow",
        description="Routes based on inquiry response",
        tasks=[
            {
                "name": "approval_gate",
                "action": "core.ask",
                "input": {
                    "prompt": "Approve the request?",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [
                    {"when": "{{ result().response.approved == true }}", "do": ["approved_path"]},
                    {"when": "{{ result().response.approved == false }}", "do": ["denied_path"]},
                ],
            },
            {"name": "approved_path", "action": approved_action["ref"], "input": {"message": "approved"}},
            {"name": "denied_path", "action": denied_action["ref"], "input": {"message": "denied"}},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    gate_task = _task_child(client, execution["id"], "approval_gate")
    inquiry = _pending_inquiry_for_execution(client, gate_task["id"])
    client.respond_to_inquiry(inquiry["id"], {"approved": False, "comment": "denied"})

    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=30,
    )
    children = _child_executions(client, execution["id"])
    assert result["status"] == "completed"
    assert [c for c in children if c["action_ref"] == approved_action["ref"]] == []
    denied_children = [c for c in children if c["action_ref"] == denied_action["ref"]]
    assert len(denied_children) == 1
    assert denied_children[0]["status"] == "completed"


def test_approval_with_timeout_and_escalation(client: AttuneClient, test_pack):
    """A timed-out inquiry task should follow a timed_out() escalation transition."""
    _assert_core_ask_available(client)
    pack_ref = test_pack["ref"]
    escalation_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"approval_escalation_{unique_ref()}",
        description="Runs after inquiry timeout",
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"approval_timeout_escalation_{unique_ref()}",
        label="Approval Timeout Escalation",
        description="Escalates when an inquiry is not answered before task timeout",
        tasks=[
            {
                "name": "approval_gate",
                "action": "core.ask",
                "timeout": 2,
                "input": {
                    "prompt": "Approve before timeout",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [{"when": "{{ timed_out() }}", "do": ["escalate"]}],
            },
            {"name": "escalate", "action": escalation_action["ref"], "input": {"message": "escalated"}},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=30,
    )
    approval_task = _task_child(client, execution["id"], "approval_gate")
    escalation_task = _task_child(client, execution["id"], "escalate")
    assert result["status"] == "completed"
    assert approval_task["workflow_task"]["timed_out"] is True
    assert escalation_task["status"] == "completed"


def test_approval_denial_stops_workflow(client: AttuneClient, test_pack):
    """A denied approval should stop the workflow when no denial transition exists."""
    _assert_core_ask_available(client)
    pack_ref = test_pack["ref"]
    final_action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"denial_should_skip_{unique_ref()}",
        description="Should not execute after denial",
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"approval_denial_stops_{unique_ref()}",
        label="Approval Denial Stops Workflow",
        description="Denied gate has only an approval transition",
        tasks=[
            {
                "name": "approval_gate",
                "action": "core.ask",
                "input": {
                    "prompt": "Approve to continue?",
                    "response_schema": {"approved": {"type": "boolean", "required": True}},
                },
                "next": [{"when": "{{ result().response.approved == true }}", "do": ["final_step"]}],
            },
            {"name": "final_step", "action": final_action["ref"], "input": {"message": "approved"}},
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    gate_task = _task_child(client, execution["id"], "approval_gate")
    inquiry = _pending_inquiry_for_execution(client, gate_task["id"])
    client.respond_to_inquiry(inquiry["id"], {"approved": False, "comment": "denied"})

    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=30,
    )
    children = _child_executions(client, execution["id"])
    assert result["status"] == "completed"
    assert [child for child in children if child["action_ref"] == final_action["ref"]] == []
