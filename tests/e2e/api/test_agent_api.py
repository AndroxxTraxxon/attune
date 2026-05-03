"""
API Tests: Agent Endpoints

Ported from crates/api/tests/agent_tests.rs
Tests agent binary distribution endpoints when agent config is not set.
"""

import pytest
import requests


@pytest.mark.api
class TestAgentInfo:
    """GET /api/v1/agent/info."""

    def test_agent_info_not_configured(self, api_base_url):
        resp = requests.get(f"{api_base_url}/api/v1/agent/info", timeout=10)
        assert resp.status_code in (200, 503)
        if resp.status_code == 503:
            body = resp.json()
            assert body["error"] == "Not configured"

    def test_agent_info_no_auth_required(self, api_base_url):
        """Endpoint is publicly accessible (no RequireAuth middleware)."""
        resp = requests.get(f"{api_base_url}/api/v1/agent/info", timeout=10)
        # Must NOT be 401 — the endpoint has no auth middleware.
        assert resp.status_code != 401
        assert resp.status_code in (200, 503)


@pytest.mark.api
class TestAgentBinary:
    """GET /api/v1/agent/binary."""

    def test_agent_binary_not_configured(self, api_base_url):
        resp = requests.get(f"{api_base_url}/api/v1/agent/binary", timeout=10)
        assert resp.status_code in (401, 403, 404, 503)

    def test_agent_binary_no_auth_required(self, api_base_url):
        resp = requests.get(f"{api_base_url}/api/v1/agent/binary", timeout=10)
        # The endpoint has no JWT RequireAuth middleware. A 401 here is the
        # endpoint's bootstrap-token guard when agent distribution is enabled.
        assert resp.status_code in (401, 403, 404, 503)

    def test_agent_binary_invalid_arch(self, api_base_url):
        """Invalid arch is validated only after bootstrap/config checks pass."""
        resp = requests.get(
            f"{api_base_url}/api/v1/agent/binary", params={"arch": "mips"}, timeout=10
        )
        assert resp.status_code in (400, 401, 403, 503)
