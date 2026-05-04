"""
API Tests: Identity & Permission Management

Ported from crates/api/tests/permissions_api_tests.rs
Tests identity CRUD and permission-set assignment flow (admin-only).
"""

import uuid

import pytest
import requests


def _uid():
    return uuid.uuid4().hex[:8]


@pytest.mark.api
class TestIdentityPermissions:
    """Admin identity CRUD and permission assignment flow."""

    def test_identity_crud_and_permission_assignment_flow(self, client):
        """
        Full lifecycle: create identity → list → update → get → assign perm
        → list perms → delete assignment → delete identity → verify 404.
        """
        s = client.session
        base = client.base_url
        uid = _uid()
        login = f"managed_{uid}"

        # Create identity
        resp = s.post(
            f"{base}/api/v1/identities",
            json={
                "login": login,
                "display_name": "Managed User",
                "password": f"ManagedPass{uid}!",
                "attributes": {"department": "platform"},
            },
            timeout=10,
        )
        assert resp.status_code == 201, resp.text
        identity_id = resp.json()["data"]["id"]

        # Verify the created identity directly. The list endpoint is paginated
        # and a reused E2E stack can contain enough identities that this new
        # row is not guaranteed to appear on the first page.
        resp = s.get(f"{base}/api/v1/identities/{identity_id}", timeout=10)
        assert resp.status_code == 200
        assert resp.json()["data"]["login"] == login

        # Update identity
        resp = s.put(
            f"{base}/api/v1/identities/{identity_id}",
            json={
                "display_name": "Managed User Updated",
                "attributes": {"department": "security"},
            },
            timeout=10,
        )
        assert resp.status_code == 200

        # Get identity — verify update
        resp = s.get(f"{base}/api/v1/identities/{identity_id}", timeout=10)
        assert resp.status_code == 200
        data = resp.json()["data"]
        assert data["display_name"] == "Managed User Updated"
        assert data["attributes"]["department"] == "security"

        # List permission sets
        resp = s.get(f"{base}/api/v1/permissions/sets", timeout=10)
        assert resp.status_code == 200

        # Assign core.admin permission
        resp = s.post(
            f"{base}/api/v1/permissions/assignments",
            json={
                "identity_id": identity_id,
                "permission_set_ref": "core.admin",
            },
            timeout=10,
        )
        assert resp.status_code == 201
        assignment_id = resp.json()["data"]["id"]
        assert resp.json()["data"]["permission_set_ref"] == "core.admin"

        # List identity permissions — should include core.admin
        resp = s.get(
            f"{base}/api/v1/identities/{identity_id}/permissions", timeout=10
        )
        assert resp.status_code == 200
        perm_refs = [p["permission_set_ref"] for p in resp.json()]
        assert "core.admin" in perm_refs

        # Delete assignment
        resp = s.delete(
            f"{base}/api/v1/permissions/assignments/{assignment_id}", timeout=10
        )
        assert resp.status_code == 200

        # Delete identity
        resp = s.delete(f"{base}/api/v1/identities/{identity_id}", timeout=10)
        assert resp.status_code == 200

        # Verify deleted — should be 404
        resp = s.get(f"{base}/api/v1/identities/{identity_id}", timeout=10)
        assert resp.status_code == 404

    def test_plain_authenticated_user_cannot_manage_identities(
        self, unique_user_client
    ):
        """A normal (non-admin) user should get 403 on identity management."""
        s = unique_user_client.session
        base = unique_user_client.base_url
        resp = s.get(f"{base}/api/v1/identities", timeout=10)
        assert resp.status_code == 403
