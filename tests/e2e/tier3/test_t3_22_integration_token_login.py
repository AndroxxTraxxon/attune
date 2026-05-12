"""T3.22: passwordless integration-token login flow."""

import pytest

from helpers import AttuneClient
from helpers.fixtures import unique_ref


pytestmark = [
    pytest.mark.tier3,
    pytest.mark.security,
    pytest.mark.rbac,
]


def _data(response, expected_statuses=(200, 201)):
    assert response.status_code in expected_statuses, response.text
    body = response.json()
    return body.get("data", body) if isinstance(body, dict) else body


def test_integration_token_login_uses_identity_permissions(client: AttuneClient):
    """Integration tokens log in as their identity and inherit limited RBAC access."""
    login = f"integration_token_{unique_ref()}@example.com"
    identity_id = None

    try:
        identity = _data(
            client.post(
                "/api/v1/identities",
                json={
                    "login": login,
                    "display_name": "E2E Integration Token User",
                    "attributes": {"source": "e2e_integration_token"},
                },
            )
        )
        identity_id = identity["id"]

        assignment = _data(
            client.post(
                "/api/v1/permissions/assignments",
                json={
                    "identity_id": identity_id,
                    "permission_set_ref": "core.viewer",
                },
            )
        )
        assert assignment["permission_set_ref"] == "core.viewer"

        token_response = _data(
            client.post(
                f"/api/v1/identities/{identity_id}/integration-tokens",
                json={
                    "label": f"e2e-token-{unique_ref()}",
                    "description": "E2E passwordless login token",
                },
            )
        )
        integration_token = token_response["token"]
        assert integration_token
        assert token_response["integration_token"]["identity_id"] == identity_id
        assert token_response["integration_token"]["active"] is True

        token_client = AttuneClient(
            base_url=client.base_url,
            timeout=client.timeout,
            auto_login=False,
        )
        login_data = _data(
            token_client.session.post(
                "/auth/token-login",
                json={"token": integration_token},
                timeout=client.timeout,
            )
        )
        assert login_data["token_type"] == "Bearer"
        assert login_data["access_token"]
        assert login_data["refresh_token"]
        assert login_data["user"]["id"] == identity_id
        assert login_data["user"]["login"] == login

        token_client.session.headers.update(
            {"Authorization": f"Bearer {login_data['access_token']}"}
        )

        me = _data(token_client.session.get("/auth/me", timeout=client.timeout))
        assert me["id"] == identity_id
        assert me["login"] == login
        assert "core.viewer" in me.get("assigned_permission_set_refs", [])

        packs = _data(
            token_client.session.get("/api/v1/packs?limit=100", timeout=client.timeout)
        )
        assert "core" in {pack["ref"] for pack in packs}

        actions = _data(
            token_client.session.get("/api/v1/actions?limit=100", timeout=client.timeout)
        )
        assert "core.echo" in {action["ref"] for action in actions}

        denied = token_client.session.get("/api/v1/identities", timeout=client.timeout)
        assert denied.status_code == 403, denied.text
    finally:
        if identity_id is not None:
            response = client.delete(f"/api/v1/identities/{identity_id}")
            assert response.status_code in (200, 204, 404), response.text
