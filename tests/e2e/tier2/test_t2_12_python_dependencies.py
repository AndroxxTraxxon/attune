"""
T2.12: Python runtime dependency execution.
"""

import json

from helpers import AttuneClient
from helpers.polling import UnexpectedTerminalStatusError, wait_for_execution_status


def _execute(client: AttuneClient, action_ref: str, timeout: int = 60) -> dict:
    execution = client.create_execution(action_ref=action_ref, parameters={})
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


def test_python_action_with_requests(client: AttuneClient, test_pack):
    result = _execute(client, f"{test_pack['ref']}.python_requests")
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert "requests_version" in output


def test_python_action_multiple_dependencies(client: AttuneClient, test_pack):
    result = _execute(client, f"{test_pack['ref']}.python_multi_deps")
    output = _output(result)
    assert result["status"] == "completed"
    assert output.get("success") is True
    assert "requests_version" in output
    assert "pyyaml_version" in output


def test_python_action_dependency_isolation(client: AttuneClient, test_pack):
    first = _execute(client, f"{test_pack['ref']}.python_requests")
    second = _execute(client, f"{test_pack['ref']}.python_requests", timeout=30)
    assert first["status"] == "completed"
    assert second["status"] == "completed"
    assert _output(first).get("requests_version")
    assert _output(second).get("requests_version")


def test_python_action_missing_dependency(client: AttuneClient, test_pack):
    execution = client.create_execution(
        action_ref=f"{test_pack['ref']}.python_missing_dep",
        parameters={},
    )
    try:
        wait_for_execution_status(
            client=client,
            execution_id=execution["id"],
            expected_status="completed",
            timeout=30,
        )
    except UnexpectedTerminalStatusError as exc:
        assert exc.actual == "failed"
    else:
        raise AssertionError("Missing dependency action should fail")
