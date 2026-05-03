"""
API Tests: Work Queue API

Ported from crates/api/tests/work_queue_api_tests.rs
Tests queue lifecycle: create, enqueue, merge-patch, update, delete items.
"""

import uuid

import pytest


def _uid():
    return uuid.uuid4().hex[:8]


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
