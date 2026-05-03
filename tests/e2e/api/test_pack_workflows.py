"""
API Tests: Pack Workflow Sync & Validate

Ported from crates/api/tests/pack_workflow_tests.rs
Tests pack workflow sync/validate endpoints and pack lifecycle with workflows.
"""

import uuid

import pytest
import requests


def _uid():
    return uuid.uuid4().hex[:8]


def _create_pack(client, suffix=None):
    """Create a unique pack and return its data."""
    ref = f"pwftest_{suffix or _uid()}"
    resp = client.session.post(
        f"{client.base_url}/api/v1/packs",
        json={
            "ref": ref,
            "label": f"Pack WF Test {ref}",
            "version": "1.0.0",
            "description": "Pack for workflow sync/validate tests",
        },
        timeout=client.timeout,
    )
    resp.raise_for_status()
    return resp.json()["data"]


@pytest.mark.api
class TestSyncPackWorkflows:
    """Pack workflow sync endpoint tests."""

    def test_sync_pack_workflows_endpoint(self, client):
        """Sync succeeds (or returns empty results) for an existing pack."""
        pack = _create_pack(client)

        resp = client.session.post(
            f"{client.base_url}/api/v1/packs/{pack['ref']}/workflows/sync",
            json={},
            timeout=client.timeout,
        )
        # Should succeed — may return 0 workflows if no files on disk
        assert resp.status_code in (200, 201)

    def test_sync_nonexistent_pack_returns_404(self, client):
        resp = client.session.post(
            f"{client.base_url}/api/v1/packs/nonexistent_pack_{_uid()}/workflows/sync",
            json={},
            timeout=client.timeout,
        )
        assert resp.status_code == 404

    def test_sync_workflows_requires_authentication(self, api_base_url):
        """Unauthenticated sync request should be rejected."""
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/api/v1/packs/any_pack/workflows/sync",
            json={},
            timeout=10,
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestValidatePackWorkflows:
    """Pack workflow validate endpoint tests."""

    def test_validate_pack_workflows_endpoint(self, client):
        """Validate succeeds for an existing pack (even with no workflows)."""
        pack = _create_pack(client)

        resp = client.session.post(
            f"{client.base_url}/api/v1/packs/{pack['ref']}/workflows/validate",
            json={},
            timeout=client.timeout,
        )
        assert resp.status_code in (200, 201)

    def test_validate_nonexistent_pack_returns_404(self, client):
        resp = client.session.post(
            f"{client.base_url}/api/v1/packs/nonexistent_pack_{_uid()}/workflows/validate",
            json={},
            timeout=client.timeout,
        )
        assert resp.status_code == 404

    def test_validate_workflows_requires_authentication(self, api_base_url):
        """Unauthenticated validate request should be rejected."""
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/api/v1/packs/any_pack/workflows/validate",
            json={},
            timeout=10,
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestPackLifecycleWithWorkflows:
    """Pack creation/update and workflow auto-sync behavior."""

    def test_pack_creation_with_auto_sync(self, client):
        """Pack creation via API succeeds; auto-sync runs if workflows exist on disk."""
        uid = _uid()
        ref = f"autosync_{uid}"
        resp = client.session.post(
            f"{client.base_url}/api/v1/packs",
            json={
                "ref": ref,
                "label": "Auto Sync Pack",
                "version": "1.0.0",
                "description": "A test pack with auto-sync",
            },
            timeout=client.timeout,
        )
        assert resp.status_code == 201

        # Verify the pack was created
        get_resp = client.session.get(
            f"{client.base_url}/api/v1/packs/{ref}",
            timeout=client.timeout,
        )
        assert get_resp.status_code == 200
        assert get_resp.json()["data"]["ref"] == ref

    def test_pack_update_with_auto_resync(self, client):
        """Pack update via API triggers workflow resync if applicable."""
        pack = _create_pack(client)

        resp = client.session.put(
            f"{client.base_url}/api/v1/packs/{pack['ref']}",
            json={
                "label": "Updated Test Pack",
                "version": "1.1.0",
            },
            timeout=client.timeout,
        )
        assert resp.status_code == 200
        assert resp.json()["data"]["version"] == "1.1.0"
