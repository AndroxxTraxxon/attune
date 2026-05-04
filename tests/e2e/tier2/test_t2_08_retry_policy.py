"""T2.8: workflow task retry policy contracts.

These tests describe and verify the retry behavior for workflow tasks.
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def _create_attempt_action(
    client: AttuneClient,
    pack_ref: str,
    *,
    name_prefix: str,
    succeed_on_attempt: int,
) -> dict:
    marker_name = f"attune_retry_{unique_ref()}.count"
    entrypoint = f"""
set -eu
marker="${{ATTUNE_ARTIFACTS_DIR:-/tmp}}/{marker_name}"
if [ -f "$marker" ]; then
  attempt="$(cat "$marker")"
else
  attempt=0
fi
attempt=$((attempt + 1))
printf '%s' "$attempt" > "$marker"
printf '{{"attempt":%s,"succeed_on_attempt":{succeed_on_attempt}}}\\n' "$attempt"
if [ "$attempt" -lt {succeed_on_attempt} ]; then
  echo "transient failure on attempt $attempt" >&2
  exit 1
fi
rm -f "$marker"
"""
    return client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"{name_prefix}_{unique_ref()}",
            "description": f"Attempt-counting action that succeeds on attempt {succeed_on_attempt}",
            "runtime_ref": "core.shell",
            "entrypoint": entrypoint,
            "enabled": True,
            "parameters": {},
        },
    )


def _create_always_failing_action(
    client: AttuneClient,
    pack_ref: str,
    *,
    name_prefix: str,
) -> dict:
    entrypoint = """
set -eu
echo '{"error":"transient connection failure"}' >&2
exit 1
"""
    return client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"{name_prefix}_{unique_ref()}",
            "description": "Always-failing retriable action",
            "runtime_ref": "core.shell",
            "entrypoint": entrypoint,
            "enabled": True,
            "parameters": {},
        },
    )


def _child_executions(client: AttuneClient, parent_id: int) -> list[dict]:
    summaries = [
        client.get_execution(execution["id"])
        for execution in client.list_executions(parent=parent_id, limit=100)
        if execution.get("parent") == parent_id
    ]
    return sorted(summaries, key=lambda execution: execution["created"])


def _task_children(client: AttuneClient, parent_id: int, task_name: str) -> list[dict]:
    children = _child_executions(client, parent_id)
    return [
        child
        for child in children
        if (child.get("workflow_task") or {}).get("task_name") == task_name
    ]


@pytest.mark.tier2
@pytest.mark.workflow
def test_retry_policy_eventual_success_preserves_lineage(client: AttuneClient, test_pack):
    """A transiently failing workflow task should retry and eventually succeed."""
    pack_ref = test_pack["ref"]
    action = _create_attempt_action(
        client,
        pack_ref,
        name_prefix="retry_eventual_success",
        succeed_on_attempt=3,
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"retry_eventual_success_{unique_ref()}",
        label="Retry Eventual Success",
        description="Task succeeds on the third total attempt",
        tasks=[
            {
                "name": "flaky_task",
                "action": action["ref"],
                "retry": {"count": 2, "delay": 1, "backoff": "constant"},
            }
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=20,
    )

    attempts = _task_children(client, execution["id"], "flaky_task")
    assert result["status"] == "completed"
    assert len(attempts) == 3
    assert [attempt["status"] for attempt in attempts] == ["failed", "failed", "completed"]
    assert [attempt["workflow_task"]["retry_count"] for attempt in attempts] == [0, 1, 2]
    assert all(attempt["parent"] == execution["id"] for attempt in attempts)
    assert all(attempt["action_ref"] == action["ref"] for attempt in attempts)
    assert attempts[1]["original_execution"] == attempts[0]["id"]
    assert attempts[2]["original_execution"] == attempts[0]["id"]


@pytest.mark.tier2
@pytest.mark.workflow
def test_retry_policy_exhaustion_fails_workflow_after_declared_attempts(
    client: AttuneClient, test_pack
):
    """Retry exhaustion should fail the workflow after initial attempt + count retries."""
    pack_ref = test_pack["ref"]
    action = _create_always_failing_action(
        client,
        pack_ref,
        name_prefix="retry_exhaustion",
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"retry_exhaustion_{unique_ref()}",
        label="Retry Exhaustion",
        description="Task exhausts two retries then fails the workflow",
        tasks=[
            {
                "name": "always_fails",
                "action": action["ref"],
                "retry": {"count": 2, "delay": 1, "backoff": "constant"},
            }
        ],
    )

    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="failed",
        timeout=20,
    )

    attempts = _task_children(client, execution["id"], "always_fails")
    assert result["status"] == "failed"
    assert len(attempts) == 3
    assert all(attempt["status"] == "failed" for attempt in attempts)
    assert [attempt["workflow_task"]["retry_count"] for attempt in attempts] == [0, 1, 2]
    assert attempts[-1]["workflow_task"]["max_retries"] == 2
    assert attempts[-1]["result"]["error"]


@pytest.mark.tier2
@pytest.mark.workflow
def test_retry_policy_no_retry_on_success(client: AttuneClient, test_pack):
    """A successful first attempt must not create retry executions."""
    pack_ref = test_pack["ref"]
    action = _create_attempt_action(
        client,
        pack_ref,
        name_prefix="retry_success_first_try",
        succeed_on_attempt=1,
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"retry_success_first_try_{unique_ref()}",
        label="Retry Success First Try",
        description="Retry policy should be inert when the first attempt succeeds",
        tasks=[
            {
                "name": "succeeds_immediately",
                "action": action["ref"],
                "retry": {"count": 3, "delay": 1, "backoff": "exponential"},
            }
        ],
    )

    started = time.monotonic()
    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=15,
    )
    elapsed = time.monotonic() - started

    attempts = _task_children(client, execution["id"], "succeeds_immediately")
    assert result["status"] == "completed"
    assert len(attempts) == 1
    assert attempts[0]["status"] == "completed"
    assert attempts[0]["workflow_task"]["retry_count"] == 0
    assert attempts[0].get("original_execution") is None
    assert elapsed < 3


@pytest.mark.tier2
@pytest.mark.workflow
def test_retry_policy_exponential_backoff_delays_retries(
    client: AttuneClient, test_pack
):
    """Exponential backoff should delay retries by the configured schedule."""
    pack_ref = test_pack["ref"]
    action = _create_attempt_action(
        client,
        pack_ref,
        name_prefix="retry_backoff",
        succeed_on_attempt=4,
    )
    workflow = client.create_workflow(
        pack_ref=pack_ref,
        name=f"retry_backoff_{unique_ref()}",
        label="Retry Exponential Backoff",
        description="Task uses 1s, 2s, 4s exponential retry delays",
        tasks=[
            {
                "name": "backoff_task",
                "action": action["ref"],
                "retry": {
                    "count": 3,
                    "delay": 1,
                    "backoff": "exponential",
                    "max_delay": 10,
                },
            }
        ],
    )

    started = time.monotonic()
    execution = client.create_execution(action_ref=workflow["ref"], parameters={})
    result = wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=25,
    )
    elapsed = time.monotonic() - started

    attempts = _task_children(client, execution["id"], "backoff_task")
    assert result["status"] == "completed"
    assert len(attempts) == 4
    assert [attempt["workflow_task"]["retry_count"] for attempt in attempts] == [0, 1, 2, 3]
    assert elapsed >= 7
