"""
API Tests: Work Queue API

Ported from crates/api/tests/work_queue_api_tests.rs
Tests queue lifecycle: create, enqueue, merge-patch, update, delete items.
"""

import time
import uuid
from pathlib import Path
from tempfile import TemporaryDirectory

import pytest


def _uid():
    return uuid.uuid4().hex[:8]


def _wait_for_queue_item_status(client, queue_ref, item_id, expected_status, timeout=60, poll=1):
    """Poll a queue item until it reaches the expected status."""
    deadline = time.time() + timeout
    last_item = None
    while time.time() < deadline:
        resp = client.session.get(
            f"{client.base_url}/api/v1/queues/{queue_ref}/items",
            params={"per_page": 100},
            timeout=10,
        )
        resp.raise_for_status()
        items = resp.json().get("data", [])
        last_item = next((item for item in items if item["id"] == item_id), None)
        if last_item and last_item["status"] == expected_status:
            return last_item
        if last_item and last_item["status"] in {"completed", "failed", "skipped"}:
            raise AssertionError(
                f"Queue item {item_id} reached terminal status "
                f"{last_item['status']!r}, not {expected_status!r}: {last_item}"
            )
        time.sleep(poll)

    raise TimeoutError(
        f"Queue item {item_id} did not reach {expected_status!r} within {timeout}s "
        f"(last item: {last_item})"
    )


def _wait_for_queue_execution(client, action_ref, item_id, timeout=60, poll=1):
    """Poll executions until the queue dispatch action for the item completes."""
    deadline = time.time() + timeout
    last_matches = []
    while time.time() < deadline:
        executions = client.list_executions(action_ref=action_ref, limit=100)
        last_matches = []
        for execution in executions:
            execution_detail = client.get_execution(execution["id"])
            if (
                (execution_detail.get("config") or {})
                .get("queue_item", {})
                .get("id")
                == item_id
            ):
                last_matches.append(execution_detail)
        terminal = [
            execution
            for execution in last_matches
            if execution["status"] in {"completed", "failed", "timeout", "cancelled"}
        ]
        if terminal:
            return terminal[0]
        time.sleep(poll)

    raise TimeoutError(
        f"No terminal queue dispatch execution found for item {item_id} "
        f"and action {action_ref} within {timeout}s (matches: {last_matches})"
    )


@pytest.mark.api
class TestWorkQueueApi:
    """Work queue definition and item lifecycle via API."""

    def test_queue_merge_patch_enqueue_and_pending_item_lifecycle(self, client, pack_ref):
        """
        Create ad-hoc queue → enqueue item → merge-patch duplicate key →
        update item → list items → delete item → verify queue.
        """
        s = client.session
        base = client.base_url
        uid = _uid()

        # Need an action for dispatch_action_ref
        actions = client.list_actions(pack_ref=pack_ref)
        assert len(actions) > 0, f"No actions found in pack {pack_ref}"
        action_ref = actions[0]["ref"]

        queue_ref = f"adhoc.queue_{uid}"

        # Create queue
        resp = s.post(
            f"{base}/api/v1/queues",
            json={
                "ref": queue_ref,
                "label": "API Queue",
                "dispatch_action_ref": action_ref,
                "enabled": False,
                "accepting_new_items": True,
                "allow_pending_update": True,
                "update_strategy": "merge_patch",
                "batch_mode": "batch",
                "item_schema": {
                    "customer": {"type": "string", "required": True},
                    "flags": {"type": "object"},
                },
                "config": {"ack_contract": {"version": 2}},
            },
            timeout=10,
        )
        assert resp.status_code == 201, resp.text

        # Enqueue first item
        resp = s.post(
            f"{base}/api/v1/queues/{queue_ref}/items",
            json={
                "item_key": "order-123",
                "priority": 9,
                "payload": {"customer": "alice", "flags": {"first": True}},
                "metadata": {"attempt": 1},
            },
            timeout=10,
        )
        assert resp.status_code == 201, resp.text
        item_id = resp.json()["data"]["id"]
        assert resp.json()["data"]["enqueue_source"] == "api"

        # Enqueue duplicate key → merge patch
        resp = s.post(
            f"{base}/api/v1/queues/{queue_ref}/items",
            json={
                "item_key": "order-123",
                "payload": {
                    "customer": "alice",
                    "flags": {"first": False, "second": True},
                    "status": "retrying",
                },
                "metadata": {"worker": "api-test"},
            },
            timeout=10,
        )
        assert resp.status_code == 200, resp.text
        merged = resp.json()["data"]
        assert merged["id"] == item_id
        assert merged["priority"] == 9  # unchanged
        assert merged["payload"]["customer"] == "alice"
        assert merged["payload"]["flags"]["first"] is False
        assert merged["payload"]["flags"]["second"] is True
        assert merged["payload"]["status"] == "retrying"
        assert merged["metadata"]["attempt"] == 1
        assert merged["metadata"]["worker"] == "api-test"

        # Update item directly
        resp = s.put(
            f"{base}/api/v1/queues/{queue_ref}/items/{item_id}",
            json={
                "priority": 12,
                "payload": {"customer": "bob"},
                "metadata": {"manual": True},
            },
            timeout=10,
        )
        assert resp.status_code == 200, resp.text
        updated = resp.json()["data"]
        assert updated["priority"] == 12
        assert updated["payload"]["customer"] == "bob"
        assert updated["metadata"]["manual"] is True

        # List items
        resp = s.get(
            f"{base}/api/v1/queues/{queue_ref}/items",
            params={"statuses": "queued,retry"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["pagination"]["total_items"] == 1
        assert resp.json()["data"][0]["id"] == item_id

        # Delete item
        resp = s.delete(
            f"{base}/api/v1/queues/{queue_ref}/items/{item_id}", timeout=10
        )
        assert resp.status_code == 200

        # Verify queue metadata
        resp = s.get(f"{base}/api/v1/queues/{queue_ref}", timeout=10)
        assert resp.status_code == 200
        q = resp.json()["data"]
        assert q["batch_mode"] == "batch"
        assert q["item_schema"]["customer"]["type"] == "string"

        # Cleanup
        resp = s.delete(f"{base}/api/v1/queues/{queue_ref}", timeout=10)

    def test_pack_managed_queue_blocks_mutations(self, client, pack_ref):
        """
        Pack-managed (non-adhoc) queues block label/delete mutations
        but allow operational toggles (enabled, accepting_new_items).

        Note: This test creates the queue via API as adhoc since we can't
        use direct DB access. The Rust test uses DB repo to set is_adhoc=false.
        We test what we can through the API — primarily the adhoc queue CRUD
        and toggling flow.
        """
        s = client.session
        base = client.base_url
        uid = _uid()

        actions = client.list_actions(pack_ref=pack_ref)
        assert len(actions) > 0
        action_ref = actions[0]["ref"]

        queue_ref = f"adhoc.toggle_{uid}"

        # Create adhoc queue
        resp = s.post(
            f"{base}/api/v1/queues",
            json={
                "ref": queue_ref,
                "label": "Toggle Queue",
                "dispatch_action_ref": action_ref,
                "accepting_new_items": True,
                "enabled": True,
            },
            timeout=10,
        )
        assert resp.status_code == 201, resp.text

        # Toggle operational flags
        resp = s.put(
            f"{base}/api/v1/queues/{queue_ref}",
            json={"enabled": False, "accepting_new_items": False},
            timeout=10,
        )
        assert resp.status_code == 200
        toggled = resp.json()["data"]
        assert toggled["enabled"] is False
        assert toggled["accepting_new_items"] is False

        # Cleanup
        resp = s.delete(f"{base}/api/v1/queues/{queue_ref}", timeout=10)
        assert resp.status_code == 200

    def test_enabled_queue_dispatches_item_and_applies_ack(self, client):
        """
        Enabled queue → enqueue item → executor leases item → worker runs action →
        completion listener applies execution.result.queue_ack to the item.
        """
        uid = _uid()
        dispatch_pack_ref = f"wq_dispatch_{uid}"
        queue_ref = f"{dispatch_pack_ref}.dispatch_e2e"
        action_ref = f"{dispatch_pack_ref}.queue_ack"

        with TemporaryDirectory(prefix="attune-wq-dispatch-pack-") as tmp:
            pack_dir = Path(tmp) / dispatch_pack_ref
            actions_dir = pack_dir / "actions"
            actions_dir.mkdir(parents=True)
            (pack_dir / "pack.yaml").write_text(
                "\n".join(
                    [
                        f"ref: {dispatch_pack_ref}",
                        "name: Work Queue Dispatch E2E",
                        "label: Work Queue Dispatch E2E",
                        "description: Pack for queue dispatch E2E coverage",
                        "version: 1.0.0",
                        "is_standard: false",
                    ]
                )
                + "\n"
            )
            (actions_dir / "queue_ack.yaml").write_text(
                "\n".join(
                    [
                        f"ref: {action_ref}",
                        "name: queue_ack",
                        "label: Queue Ack",
                        "description: Acknowledge a leased work queue item",
                        "enabled: true",
                        "runner_type: shell",
                        "entry_point: queue_ack.sh",
                        "output_format: json",
                    ]
                )
                + "\n"
            )
            (actions_dir / "queue_ack.sh").write_text(
                "\n".join(
                    [
                        "#!/usr/bin/env bash",
                        "set -euo pipefail",
                        "INPUT=$(cat)",
                        "ITEM_ID=$(printf '%s' \"$INPUT\" | "
                        "sed -n 's/.*\"queue_item\"[^{]*{[^}]*\"id\"[[:space:]]*:[[:space:]]*"
                        "\\([0-9][0-9]*\\).*/\\1/p')",
                        "if [ -z \"$ITEM_ID\" ]; then",
                        "  echo \"missing queue_item.id in input: $INPUT\" >&2",
                        "  exit 1",
                        "fi",
                        "printf '{\"queue_ack\":{\"version\":1,\"items\":[{\"id\":%s,"
                        "\"status\":\"completed\",\"summary\":{\"source\":\"e2e\"}}]},"
                        "\"seen_item_id\":%s}\\n' \"$ITEM_ID\" \"$ITEM_ID\"",
                    ]
                )
                + "\n"
            )
            client.upload_pack(str(pack_dir), force=True)

            resp = client.session.post(
                f"{client.base_url}/api/v1/queues",
                json={
                    "ref": queue_ref,
                    "pack_ref": dispatch_pack_ref,
                    "label": "Dispatch E2E Queue",
                    "dispatch_action_ref": action_ref,
                    "enabled": True,
                    "accepting_new_items": True,
                    "batch_mode": "single",
                    "action_params": {
                        "queue": "{{ queue }}",
                        "queue_item": "{{ queue_item }}",
                        "item": "{{ item }}",
                    },
                    "config": {"ack_contract": {"version": 1}},
                },
                timeout=10,
            )
            assert resp.status_code == 201, resp.text

            resp = client.session.post(
                f"{client.base_url}/api/v1/queues/{queue_ref}/items",
                json={
                    "item_key": f"dispatch-{uid}",
                    "payload": {"customer": "alice", "order_id": uid},
                    "metadata": {"source": "e2e"},
                },
                timeout=10,
            )
            assert resp.status_code == 201, resp.text
            item_id = resp.json()["data"]["id"]

            item = _wait_for_queue_item_status(
                client, queue_ref, item_id, "completed", timeout=75
            )
            assert item["leased_execution"] is None
            assert item["lease_token"] is None
            assert item["ack_summary"]["status"] == "completed"
            assert item["ack_summary"]["summary"]["source"] == "e2e"

            execution = _wait_for_queue_execution(
                client, action_ref, item_id, timeout=10
            )
            assert execution["status"] == "completed"
            assert execution["config"]["queue"]["ref"] == queue_ref
            assert execution["config"]["queue_item"]["id"] == item_id
            assert execution["result"]["queue_ack"]["items"][0]["id"] == item_id

            client.session.put(
                f"{client.base_url}/api/v1/queues/{queue_ref}",
                json={"enabled": False, "accepting_new_items": False},
                timeout=10,
            )
            client.session.delete(f"{client.base_url}/api/v1/queues/{queue_ref}", timeout=10)
            client.delete_pack(dispatch_pack_ref)
