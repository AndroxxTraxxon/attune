"""
T2.13: Node.js runtime execution.
"""

import json

from helpers import AttuneClient
from helpers.polling import wait_for_execution_status


def _execute(client: AttuneClient, action_ref: str, parameters: dict | None = None, timeout: int = 60) -> dict:
    execution = client.create_execution(action_ref=action_ref, parameters=parameters or {})
    return wait_for_execution_status(
        client=client,
        execution_id=execution["id"],
        expected_status="completed",
        timeout=timeout,
    )


def _output(result: dict) -> dict:
    raw = result.get("result", {})
    if raw.get("stdout"):
        return json.loads(raw["stdout"])
    return raw


def test_nodejs_action_basic(client: AttuneClient, test_pack):
    result = _execute(
        client,
        f"{test_pack['ref']}.node_basic",
        parameters={"message": "Test message"},
        timeout=30,
    )
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert output.get("message") == "Test message"
    assert output.get("nodeVersion", "").startswith("v")


def test_nodejs_action_with_axios(client: AttuneClient, test_pack):
    result = _execute(client, f"{test_pack['ref']}.node_axios")
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert output.get("axiosVersion")


def test_nodejs_action_multiple_packages(client: AttuneClient, test_pack):
    result = _execute(client, f"{test_pack['ref']}.node_multi_packages")
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert output.get("axiosAvailable") is True
    assert output.get("lodashVersion")
    assert output.get("sum") == 15


def test_nodejs_action_async_await(client: AttuneClient, test_pack):
    result = _execute(client, f"{test_pack['ref']}.node_async", timeout=30)
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert output.get("delaysCompleted") == 2
