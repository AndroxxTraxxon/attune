"""
T2.10: with_items workflow execution

Tests that workflow with_items expansion creates one child execution per item and
honors the current concurrency field.
"""

import time
from datetime import datetime

from helpers import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def _create_item_action(client: AttuneClient, pack_ref: str, name_prefix: str) -> dict:
    return client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"{name_prefix}_{unique_ref()}",
            "description": "Processes one with_items item",
            "runtime_ref": "core.shell",
            "entrypoint": (
                'echo "Processing item: $item"; '
                'sleep "${sleep_seconds:-1}"; '
                'echo "Completed item: $item"; '
                "printf '{\"item\":\"%s\",\"success\":true}\\n' \"$item\""
            ),
            "enabled": True,
            "param_schema": {
                "item": {"type": "string", "required": True},
                "sleep_seconds": {"type": "integer", "required": False},
            },
        },
    )


def _create_with_items_workflow(
    client: AttuneClient,
    pack_ref: str,
    action_ref: str,
    name_prefix: str,
    concurrency: int,
) -> dict:
    return client.create_workflow(
        pack_ref=pack_ref,
        name=f"{name_prefix}_{unique_ref()}",
        label=name_prefix.replace("_", " ").title(),
        description="Workflow with with_items expansion",
        param_schema={
            "items": {"type": "array", "required": True},
            "sleep_seconds": {"type": "integer", "required": False},
        },
        tasks=[
            {
                "name": "process_items",
                "action": action_ref,
                "with_items": "{{ parameters.items }}",
                "input": {
                    "item": "{{ item }}",
                    "sleep_seconds": "{{ parameters.sleep_seconds }}",
                },
                "concurrency": concurrency,
            }
        ],
    )


def _execute_and_get_children(
    client: AttuneClient,
    workflow_ref: str,
    items: list[str],
    *,
    sleep_seconds: int = 1,
    timeout: int = 120,
) -> tuple[dict, list[dict], float]:
    start_time = time.time()
    execution = client.create_execution(
        action_ref=workflow_ref,
        parameters={"items": items, "sleep_seconds": sleep_seconds},
    )
    execution_id = execution["id"]
    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="completed",
        timeout=timeout,
    )
    elapsed = time.time() - start_time
    child_summaries = client.list_executions(parent=execution_id, limit=max(len(items) + 5, 20))
    children = [client.get_execution(child["id"]) for child in child_summaries]
    return result, children, elapsed


def _assert_all_items_completed(children: list[dict], expected_count: int):
    assert len(children) == expected_count, (
        f"Expected {expected_count} child executions, got {len(children)}"
    )
    failed = [child for child in children if child["status"] != "completed"]
    assert failed == [], f"Expected all child executions to complete, got {failed}"


def _parse_time(value: str | None) -> datetime:
    assert value, "Execution timestamp is missing"
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def _execution_windows(children: list[dict]) -> list[tuple[datetime, datetime]]:
    windows = []
    for child in children:
        start = _parse_time(child.get("started_at"))
        end = _parse_time(child.get("updated"))
        assert end >= start, f"Execution {child['id']} ended before it started"
        windows.append((start, end))
    return windows


def _max_concurrent(children: list[dict]) -> int:
    events = []
    for start, end in _execution_windows(children):
        events.append((start, 1))
        events.append((end, -1))

    current = 0
    maximum = 0
    for _timestamp, delta in sorted(events, key=lambda item: (item[0], -item[1])):
        current += delta
        maximum = max(maximum, current)
    return maximum


def test_parallel_execution_basic(client: AttuneClient, test_pack):
    pack_ref = test_pack["ref"]
    items = ["item1", "item2", "item3", "item4", "item5"]
    action = _create_item_action(client, pack_ref, "parallel_action")
    workflow = _create_with_items_workflow(
        client,
        pack_ref,
        action["ref"],
        "parallel_workflow",
        concurrency=len(items),
    )

    result, children, _elapsed = _execute_and_get_children(
        client, workflow["ref"], items, sleep_seconds=1
    )

    assert result["status"] == "completed"
    _assert_all_items_completed(children, len(items))
    assert _max_concurrent(children) > 1, "Expected with_items children to overlap"


def test_parallel_execution_with_concurrency_limit(client: AttuneClient, test_pack):
    pack_ref = test_pack["ref"]
    items = [f"item{i}" for i in range(1, 11)]
    action = _create_item_action(client, pack_ref, "limited_parallel")
    workflow = _create_with_items_workflow(
        client,
        pack_ref,
        action["ref"],
        "limited_workflow",
        concurrency=3,
    )

    result, children, _elapsed = _execute_and_get_children(
        client, workflow["ref"], items, sleep_seconds=1
    )

    assert result["status"] == "completed"
    _assert_all_items_completed(children, len(items))
    max_concurrent = _max_concurrent(children)
    assert max_concurrent > 1, "Expected concurrency-limited children to overlap"
    assert max_concurrent <= 3, f"Expected concurrency limit 3, got {max_concurrent}"


def test_parallel_execution_sequential_mode(client: AttuneClient, test_pack):
    pack_ref = test_pack["ref"]
    items = ["item1", "item2", "item3"]
    action = _create_item_action(client, pack_ref, "sequential_action")
    workflow = _create_with_items_workflow(
        client,
        pack_ref,
        action["ref"],
        "sequential_workflow",
        concurrency=1,
    )

    result, children, elapsed = _execute_and_get_children(
        client, workflow["ref"], items, sleep_seconds=1
    )

    assert result["status"] == "completed"
    _assert_all_items_completed(children, len(items))
    assert _max_concurrent(children) == 1, "Expected sequential with_items execution"
    assert elapsed >= 3, f"Expected sequential execution to take at least 3s, took {elapsed:.1f}s"


def test_parallel_execution_large_batch(client: AttuneClient, test_pack):
    pack_ref = test_pack["ref"]
    items = [f"item{i:02d}" for i in range(1, 21)]
    action = _create_item_action(client, pack_ref, "large_batch")
    workflow = _create_with_items_workflow(
        client,
        pack_ref,
        action["ref"],
        "large_batch_workflow",
        concurrency=10,
    )

    result, children, _elapsed = _execute_and_get_children(
        client, workflow["ref"], items, sleep_seconds=1
    )

    assert result["status"] == "completed"
    _assert_all_items_completed(children, len(items))
    assert _max_concurrent(children) > 1, "Expected large batch children to overlap"
