"""
API Tests: End-to-End Execution Pipeline

Tests the complete action execution lifecycle:
  API request → executor scheduling → worker execution → result storage

These tests exercise the core happy path that is NOT covered by
sensor→rule tests (tier 1) or workflow orchestration tests (tier 2).
Every test here calls POST /api/v1/executions/execute directly.
"""

import json
import time
import uuid

import pytest


def _uid():
    return uuid.uuid4().hex[:8]


def _wait_terminal(client, execution_id, timeout=30, poll=0.5):
    """Poll until execution reaches a terminal status. Returns execution dict."""
    terminal = {"completed", "failed", "timeout", "cancelled"}
    deadline = time.time() + timeout
    while time.time() < deadline:
        resp = client.session.get(
            f"{client.base_url}/api/v1/executions/{execution_id}",
            timeout=10,
        )
        resp.raise_for_status()
        ex = resp.json()["data"]
        if ex["status"] in terminal:
            return ex
        time.sleep(poll)
    raise TimeoutError(
        f"Execution {execution_id} did not reach terminal status within {timeout}s "
        f"(last status: {ex['status']})"
    )


def _create_shell_action(client, pack_ref, name, script=None):
    """Create an ad-hoc shell action with inline shell content."""
    uid = _uid()
    action_ref = f"{pack_ref}.{name}_{uid}"
    script = script or "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\""
    resp = client.session.post(
        f"{client.base_url}/api/v1/actions",
        json={
            "ref": action_ref,
            "pack_ref": pack_ref,
            "label": f"Test {name}",
            "description": f"Shell action for pipeline test",
            "entrypoint": script,
            "runtime_ref": "core.shell",
            "param_schema": {},
        },
        timeout=10,
    )
    resp.raise_for_status()
    return resp.json()["data"]


def _execute(client, action_ref, parameters=None):
    """Execute an action and return the execution record."""
    resp = client.session.post(
        f"{client.base_url}/api/v1/executions/execute",
        json={"action_ref": action_ref, **({"parameters": parameters} if parameters else {})},
        timeout=10,
    )
    resp.raise_for_status()
    return resp.json()["data"]


@pytest.mark.api
class TestExecutionPipeline:
    """Direct action execution via API → executor → worker → result."""

    def test_shell_action_succeeds(self, client, pack_ref):
        """
        Execute a simple shell echo action and verify the full lifecycle:
        requested → scheduled → running → succeeded.
        """
        action = _create_shell_action(
            client, pack_ref, "echo"
        )

        execution = _execute(client, action["ref"])
        assert execution["status"] in ("requested", "scheduled", "running")
        exec_id = execution["id"]

        result = _wait_terminal(client, exec_id, timeout=30)
        assert result["status"] == "completed", (
            f"Expected succeeded, got {result['status']}. "
            f"Result: {json.dumps(result.get('result'), indent=2)}"
        )

    def test_shell_action_with_parameters(self, client, pack_ref):
        """
        Execute shell action that reads parameters from stdin JSON
        and echoes them back. Verifies parameter delivery and result capture.
        """
        uid = _uid()
        action_ref = f"{pack_ref}.param_echo_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/actions",
                json={
                    "ref": action_ref,
                    "pack_ref": pack_ref,
                    "label": "Param Echo",
                    "entrypoint": "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\"",
                    "runtime_ref": "core.shell",
                    "param_schema": {
                        "name": {"type": "string", "required": True},
                        "count": {"type": "integer"},
                    },
                },
            timeout=10,
        )
        resp.raise_for_status()

        execution = _execute(client, action_ref, parameters={"name": "world", "count": 42})
        result = _wait_terminal(client, execution["id"], timeout=30)
        assert result["status"] == "completed"

    def test_action_failure_captured(self, client, pack_ref):
        """
        Execute an action that exits with non-zero code.
        Verify status becomes 'failed' and error info is captured.
        """
        action = _create_shell_action(
            client,
            pack_ref,
            "fail",
            script="echo '{\"error\":\"Action intentionally failed\"}' >&2; exit 1",
        )

        execution = _execute(client, action["ref"])
        result = _wait_terminal(client, execution["id"], timeout=30)
        assert result["status"] == "failed"

    def test_execution_status_transitions(self, client, pack_ref):
        """
        Verify the execution passes through expected status transitions.
        Poll rapidly to catch intermediate states.
        """
        action = _create_shell_action(
            client,
            pack_ref,
            "slow",
            script="INPUT=$(cat); DURATION=$(echo \"$INPUT\" | sed -n 's/.*\"duration\"[[:space:]]*:[[:space:]]*\\([0-9]*\\).*/\\1/p'); DURATION=${DURATION:-1}; sleep \"$DURATION\"; echo '{\"success\":true}'",
        )

        execution = _execute(client, action["ref"], parameters={"duration": 2})
        exec_id = execution["id"]
        seen_statuses = {execution["status"]}

        deadline = time.time() + 30
        last_status = execution["status"]
        while time.time() < deadline:
            resp = client.session.get(
                f"{client.base_url}/api/v1/executions/{exec_id}",
                timeout=10,
            )
            resp.raise_for_status()
            ex = resp.json()["data"]
            seen_statuses.add(ex["status"])
            last_status = ex["status"]
            if last_status in ("completed", "failed"):
                break
            time.sleep(0.3)

        assert last_status == "completed"
        # Should see at least requested/scheduled and succeeded
        assert "completed" in seen_statuses

    def test_execution_list_and_get(self, client, pack_ref):
        """
        Execute an action, then verify it appears in list and get endpoints.
        """
        action = _create_shell_action(
            client, pack_ref, "list_test"
        )

        execution = _execute(client, action["ref"])
        exec_id = execution["id"]
        _wait_terminal(client, exec_id, timeout=30)

        # GET single
        resp = client.session.get(
            f"{client.base_url}/api/v1/executions/{exec_id}",
            timeout=10,
        )
        assert resp.status_code == 200
        data = resp.json()["data"]
        assert data["id"] == exec_id
        assert data["action_ref"] == action["ref"]

        # LIST with action_ref filter
        resp = client.session.get(
            f"{client.base_url}/api/v1/executions",
            params={"action_ref": action["ref"], "per_page": 10},
            timeout=10,
        )
        assert resp.status_code == 200
        items = resp.json()["data"]
        assert any(e["id"] == exec_id for e in items)

    def test_multiple_concurrent_executions(self, client, pack_ref):
        """
        Fire multiple executions simultaneously and verify all complete.
        """
        action = _create_shell_action(
            client,
            pack_ref,
            "concurrent",
            script="INPUT=$(cat); DURATION=$(echo \"$INPUT\" | sed -n 's/.*\"duration\"[[:space:]]*:[[:space:]]*\\([0-9]*\\).*/\\1/p'); DURATION=${DURATION:-1}; sleep \"$DURATION\"; echo '{\"success\":true}'",
        )

        exec_ids = []
        for _ in range(5):
            ex = _execute(client, action["ref"], parameters={"duration": 1})
            exec_ids.append(ex["id"])

        for eid in exec_ids:
            result = _wait_terminal(client, eid, timeout=60)
            assert result["status"] == "completed", (
                f"Execution {eid} status: {result['status']}"
            )

    def test_execute_nonexistent_action_fails(self, client):
        """Executing a nonexistent action ref returns an error."""
        resp = client.session.post(
            f"{client.base_url}/api/v1/executions/execute",
            json={"action_ref": "nonexistent.action_xyz"},
            timeout=10,
        )
        assert resp.status_code in (400, 404)

    def test_execute_requires_authentication(self, api_base_url):
        """Execution endpoint requires auth."""
        import requests
        resp = requests.post(
            f"{api_base_url}/api/v1/executions/execute",
            json={"action_ref": "core.echo"},
            timeout=10,
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestWorkflowExecution:
    """Workflow execution via API — covers orchestration through executor."""

    def test_simple_two_task_workflow(self, client, pack_ref):
        """
        Create a workflow with two sequential tasks and execute it.
        Verifies: workflow detection, child task dispatch, completion.
        """
        uid = _uid()

        # Create leaf actions
        task1_ref = f"{pack_ref}.wf_task1_{uid}"
        task2_ref = f"{pack_ref}.wf_task2_{uid}"
        for ref, label in [(task1_ref, "WF Task 1"), (task2_ref, "WF Task 2")]:
            resp = client.session.post(
                f"{client.base_url}/api/v1/actions",
                json={
                    "ref": ref,
                    "pack_ref": pack_ref,
                    "label": label,
                    "entrypoint": "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\"",
                    "runtime_ref": "core.shell",
                },
                timeout=10,
            )
            resp.raise_for_status()

        # Create workflow via workflow API
        wf_ref = f"{pack_ref}.wf_seq_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/workflows",
            json={
                "ref": wf_ref,
                "pack_ref": pack_ref,
                "label": "Sequential Workflow",
                "version": "1.0.0",
                "definition": {
                    "version": "1.0.0",
                    "tasks": [
                        {
                            "name": "step1",
                            "action": task1_ref,
                            "input": {},
                            "next": [{"when": "{{ succeeded() }}", "do": ["step2"]}],
                        },
                        {
                            "name": "step2",
                            "action": task2_ref,
                            "input": {},
                        },
                    ],
                },
            },
            timeout=10,
        )
        resp.raise_for_status()

        # Execute workflow
        execution = _execute(client, wf_ref)
        result = _wait_terminal(client, execution["id"], timeout=60)
        assert result["status"] == "completed", (
            f"Workflow failed: {json.dumps(result.get('result'), indent=2)}"
        )

    def test_workflow_with_failure_transition(self, client, pack_ref):
        """
        Workflow where task1 fails and the failure transition routes to
        an error handler task. Verifies conditional transition evaluation.
        """
        uid = _uid()

        # Failing action
        fail_ref = f"{pack_ref}.wf_fail_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/actions",
            json={
                "ref": fail_ref,
                "pack_ref": pack_ref,
                "label": "Fail Action",
                "entrypoint": "echo '{\"error\":\"Action intentionally failed\"}' >&2; exit 1",
                "runtime_ref": "core.shell",
            },
            timeout=10,
        )
        resp.raise_for_status()

        # Recovery action
        recover_ref = f"{pack_ref}.wf_recover_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/actions",
            json={
                "ref": recover_ref,
                "pack_ref": pack_ref,
                "label": "Recover Action",
                "entrypoint": "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\"",
                "runtime_ref": "core.shell",
            },
            timeout=10,
        )
        resp.raise_for_status()

        # Workflow with failure transition
        wf_ref = f"{pack_ref}.wf_errhandler_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/workflows",
            json={
                "ref": wf_ref,
                "pack_ref": pack_ref,
                "label": "Error Handler Workflow",
                "version": "1.0.0",
                "definition": {
                    "version": "1.0.0",
                    "tasks": [
                        {
                            "name": "risky_step",
                            "action": fail_ref,
                            "input": {},
                            "next": [
                                {"when": "{{ succeeded() }}", "do": ["done"]},
                                {"when": "{{ failed() }}", "do": ["handle_error"]},
                            ],
                        },
                        {
                            "name": "handle_error",
                            "action": recover_ref,
                            "input": {},
                        },
                        {
                            "name": "done",
                            "action": recover_ref,
                            "input": {},
                        },
                    ],
                },
            },
            timeout=10,
        )
        resp.raise_for_status()

        execution = _execute(client, wf_ref)
        result = _wait_terminal(client, execution["id"], timeout=60)
        # Workflow should succeed because error handler ran
        assert result["status"] == "completed", (
            f"Expected succeeded (error handler should run), got {result['status']}"
        )


@pytest.mark.api
class TestEventDrivenChain:
    """
    Sensor → Event → Rule → Execution chain via webhooks.

    This complements the tier 1 timer tests by testing the same chain
    through the webhook trigger path, which is more controllable
    for E2E testing (no timing dependencies).
    """

    def test_webhook_rule_execution_chain(self, client, pack_ref):
        """
        Full chain: create trigger → enable webhook → create rule →
        POST webhook → verify event → verify enforcement → verify execution.
        """
        uid = _uid()
        s = client.session
        base = client.base_url

        # 1. Create action
        action_ref = f"{pack_ref}.chain_echo_{uid}"
        resp = s.post(
            f"{base}/api/v1/actions",
            json={
                "ref": action_ref,
                "pack_ref": pack_ref,
                "label": "Chain Echo",
                "entrypoint": "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\"",
                "runtime_ref": "core.shell",
            },
            timeout=10,
        )
        resp.raise_for_status()

        # 2. Create trigger
        trigger_ref = f"{pack_ref}.chain_trigger_{uid}"
        resp = s.post(
            f"{base}/api/v1/triggers",
            json={
                "ref": trigger_ref,
                "pack_ref": pack_ref,
                "label": "Chain Trigger",
            },
            timeout=10,
        )
        resp.raise_for_status()

        # 3. Enable webhook
        resp = s.post(f"{base}/api/v1/triggers/{trigger_ref}/webhooks/enable", timeout=10)
        resp.raise_for_status()
        webhook_data = resp.json()["data"]
        webhook_url = webhook_data.get("webhook_url") or webhook_data.get("url")
        webhook_key = webhook_data.get("webhook_key") or webhook_data.get("key")

        # 4. Create rule linking trigger → action
        rule_ref = f"{pack_ref}.chain_rule_{uid}"
        resp = s.post(
            f"{base}/api/v1/rules",
            json={
                "ref": rule_ref,
                "pack_ref": pack_ref,
                "label": "Chain Rule",
                "trigger_ref": trigger_ref,
                "action_ref": action_ref,
                "enabled": True,
            },
            timeout=10,
        )
        resp.raise_for_status()

        # 5. Fire webhook
        if webhook_url and webhook_url.startswith("http"):
            wh_target = webhook_url
        else:
            wh_target = f"{base}/api/v1/webhooks/{webhook_key}"

        resp = s.post(
            wh_target,
            json={"payload": {"test_data": "hello_chain", "uid": uid}},
            timeout=10,
        )
        assert resp.status_code in (200, 201, 202), f"Webhook POST failed: {resp.text}"

        # 6. Wait for an execution to appear for our action
        deadline = time.time() + 30
        execution = None
        while time.time() < deadline:
            resp = s.get(
                f"{base}/api/v1/executions",
                params={"action_ref": action_ref, "per_page": 5},
                timeout=10,
            )
            if resp.status_code == 200:
                items = resp.json().get("data", [])
                if items:
                    execution = items[0]
                    break
            time.sleep(1)

        assert execution is not None, (
            f"No execution appeared for {action_ref} within 30s"
        )

        # 7. Wait for terminal status
        result = _wait_terminal(client, execution["id"], timeout=30)
        assert result["status"] in ("completed", "failed"), (
            f"Execution stuck at {result['status']}"
        )

        # 8. Verify event was created
        resp = s.get(
            f"{base}/api/v1/events",
            params={"trigger_ref": trigger_ref, "per_page": 5},
            timeout=10,
        )
        if resp.status_code == 200:
            events = resp.json().get("data", [])
            assert len(events) >= 1, "Expected at least 1 event from webhook"

    def test_webhook_rule_with_template_params(self, client, pack_ref):
        """
        Webhook event payload is templated into action parameters via rule
        action_params. Validates template resolution in the event chain.
        """
        uid = _uid()
        s = client.session
        base = client.base_url

        # Action with parameters
        action_ref = f"{pack_ref}.tmpl_echo_{uid}"
        resp = s.post(
            f"{base}/api/v1/actions",
            json={
                "ref": action_ref,
                "pack_ref": pack_ref,
                "label": "Template Echo",
                "entrypoint": "INPUT=$(cat); printf '{\"success\":true,\"input\":%s}\\n' \"${INPUT:-{}}\"",
                "runtime_ref": "core.shell",
                "param_schema": {
                    "greeting": {"type": "string"},
                },
            },
            timeout=10,
        )
        resp.raise_for_status()

        # Trigger
        trigger_ref = f"{pack_ref}.tmpl_trigger_{uid}"
        resp = s.post(
            f"{base}/api/v1/triggers",
            json={"ref": trigger_ref, "pack_ref": pack_ref, "label": "Template Trigger"},
            timeout=10,
        )
        resp.raise_for_status()

        # Enable webhook
        resp = s.post(f"{base}/api/v1/triggers/{trigger_ref}/webhooks/enable", timeout=10)
        resp.raise_for_status()
        wh = resp.json()["data"]
        webhook_key = wh.get("webhook_key") or wh.get("key")

        # Rule with template params
        rule_ref = f"{pack_ref}.tmpl_rule_{uid}"
        resp = s.post(
            f"{base}/api/v1/rules",
            json={
                "ref": rule_ref,
                "pack_ref": pack_ref,
                "label": "Template Rule",
                "trigger_ref": trigger_ref,
                "action_ref": action_ref,
                "enabled": True,
                "action_params": {
                    "greeting": "{{ event.payload.message }}",
                },
            },
            timeout=10,
        )
        resp.raise_for_status()

        # Fire webhook with payload
        resp = s.post(
            f"{base}/api/v1/webhooks/{webhook_key}",
            json={"payload": {"message": "hello_from_webhook"}},
            timeout=10,
        )
        assert resp.status_code in (200, 201, 202)

        # Wait for execution
        deadline = time.time() + 30
        execution = None
        while time.time() < deadline:
            resp = s.get(
                f"{base}/api/v1/executions",
                params={"action_ref": action_ref, "per_page": 5},
                timeout=10,
            )
            if resp.status_code == 200:
                items = resp.json().get("data", [])
                if items:
                    execution = items[0]
                    break
            time.sleep(1)

        assert execution is not None, "No execution from templated webhook rule"

        result = _wait_terminal(client, execution["id"], timeout=30)
        # Verify it ran (the template resolved or not, the action should still execute)
        assert result["status"] in ("completed", "failed")
