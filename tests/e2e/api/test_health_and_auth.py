"""
API Tests: Health & Authentication

Ported from crates/api/tests/health_and_auth_tests.rs
Tests health endpoints, user registration, login, token refresh, LDAP stubs.
"""

import uuid

import pytest
import requests


def _uid():
    return uuid.uuid4().hex[:8]


BASE_PASSWORD = "SecurePassword123!"


@pytest.mark.api
class TestHealthEndpoints:
    """Health check endpoints (no auth required)."""

    def test_health_check(self, client):
        resp = client.session.get(f"{client.base_url}/health", timeout=5)
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "ok"

    def test_health_detailed(self, client):
        resp = client.session.get(f"{client.base_url}/health/detailed", timeout=5)
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "ok"
        assert body["database"] == "connected"
        assert isinstance(body["version"], str)

    def test_health_ready(self, client):
        resp = client.session.get(f"{client.base_url}/health/ready", timeout=5)
        assert resp.status_code == 200

    def test_health_live(self, client):
        resp = client.session.get(f"{client.base_url}/health/live", timeout=5)
        assert resp.status_code == 200


@pytest.mark.api
class TestUserRegistration:
    """User registration endpoint tests."""

    def test_register_user(self, api_base_url):
        login = f"newuser_{_uid()}"
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/register",
            json={
                "login": login,
                "password": BASE_PASSWORD,
                "display_name": "New User",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body["data"]["access_token"], str)
        assert isinstance(body["data"]["refresh_token"], str)
        assert body["data"]["user"]["login"] == login
        assert body["data"]["user"]["display_name"] == "New User"

    def test_register_duplicate_user(self, api_base_url):
        login = f"dup_{_uid()}"
        s = requests.Session()
        # First registration
        resp1 = s.post(
            f"{api_base_url}/auth/register",
            json={"login": login, "password": BASE_PASSWORD, "display_name": "Dup"},
            timeout=10,
        )
        assert resp1.status_code == 200
        # Second registration with same login
        resp2 = s.post(
            f"{api_base_url}/auth/register",
            json={"login": login, "password": BASE_PASSWORD, "display_name": "Dup"},
            timeout=10,
        )
        assert resp2.status_code == 409

    def test_register_invalid_password(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/register",
            json={
                "login": f"weak_{_uid()}",
                "password": "weak",
                "display_name": "Weak",
            },
            timeout=10,
        )
        assert resp.status_code == 422


@pytest.mark.api
class TestLogin:
    """Login endpoint tests."""

    def test_login_success(self, api_base_url):
        login = f"loginuser_{_uid()}"
        s = requests.Session()
        # Register first
        s.post(
            f"{api_base_url}/auth/register",
            json={
                "login": login,
                "password": BASE_PASSWORD,
                "display_name": "Login User",
            },
            timeout=10,
        )
        # Login
        resp = s.post(
            f"{api_base_url}/auth/login",
            json={"login": login, "password": BASE_PASSWORD},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body["data"]["access_token"], str)
        assert isinstance(body["data"]["refresh_token"], str)
        assert body["data"]["user"]["login"] == login

    def test_login_wrong_password(self, api_base_url):
        login = f"wrongpw_{_uid()}"
        s = requests.Session()
        s.post(
            f"{api_base_url}/auth/register",
            json={
                "login": login,
                "password": BASE_PASSWORD,
                "display_name": "WrongPW",
            },
            timeout=10,
        )
        resp = s.post(
            f"{api_base_url}/auth/login",
            json={"login": login, "password": "WrongPassword123!"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_login_nonexistent_user(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/login",
            json={"login": f"noone_{_uid()}", "password": BASE_PASSWORD},
            timeout=10,
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestLdapAuth:
    """LDAP auth endpoint tests (LDAP not configured)."""

    def test_ldap_login_returns_501_when_not_configured(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/ldap/login",
            json={"login": "jdoe", "password": "secret"},
            timeout=10,
        )
        assert resp.status_code == 501

    def test_ldap_login_validates_empty_login(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/ldap/login",
            json={"login": "", "password": "secret"},
            timeout=10,
        )
        assert resp.status_code == 422

    def test_ldap_login_validates_empty_password(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/ldap/login",
            json={"login": "jdoe", "password": ""},
            timeout=10,
        )
        assert resp.status_code == 422

    def test_ldap_login_validates_missing_fields(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/ldap/login",
            json={},
            timeout=10,
        )
        assert resp.status_code == 422


@pytest.mark.api
class TestAuthSettings:
    """Auth settings endpoint tests."""

    def test_auth_settings_includes_ldap_fields_disabled(self, api_base_url):
        s = requests.Session()
        resp = s.get(f"{api_base_url}/auth/settings", timeout=10)
        assert resp.status_code == 200
        data = resp.json()["data"]
        # LDAP not configured → disabled/null
        assert data["ldap_enabled"] is False
        assert data["ldap_visible_by_default"] is False
        assert data["ldap_provider_name"] is None
        assert data["ldap_provider_label"] is None
        assert data["ldap_provider_icon_url"] is None
        # Core fields present
        assert isinstance(data["authentication_enabled"], bool)
        assert isinstance(data["local_password_enabled"], bool)
        assert isinstance(data["oidc_enabled"], bool)
        assert isinstance(data["self_registration_enabled"], bool)


@pytest.mark.api
class TestCurrentUser:
    """GET /auth/me tests."""

    def test_get_current_user(self, client):
        resp = client.session.get(
            f"{client.base_url}/auth/me", timeout=10
        )
        assert resp.status_code == 200
        data = resp.json()["data"]
        assert isinstance(data["id"], int)
        assert isinstance(data["login"], str)

    def test_get_current_user_unauthorized(self, api_base_url):
        s = requests.Session()
        resp = s.get(f"{api_base_url}/auth/me", timeout=10)
        assert resp.status_code == 401

    def test_get_current_user_invalid_token(self, api_base_url):
        s = requests.Session()
        s.headers["Authorization"] = "Bearer invalid-token"
        resp = s.get(f"{api_base_url}/auth/me", timeout=10)
        assert resp.status_code == 401


@pytest.mark.api
class TestTokenRefresh:
    """Token refresh endpoint tests."""

    def test_refresh_token(self, api_base_url):
        login = f"refresh_{_uid()}"
        s = requests.Session()
        reg = s.post(
            f"{api_base_url}/auth/register",
            json={
                "login": login,
                "password": BASE_PASSWORD,
                "display_name": "Refresh User",
            },
            timeout=10,
        )
        refresh_token = reg.json()["data"]["refresh_token"]
        # Use refresh token
        resp = s.post(
            f"{api_base_url}/auth/refresh",
            json={"refresh_token": refresh_token},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body["data"]["access_token"], str)
        assert isinstance(body["data"]["refresh_token"], str)

    def test_refresh_with_invalid_token(self, api_base_url):
        s = requests.Session()
        resp = s.post(
            f"{api_base_url}/auth/refresh",
            json={"refresh_token": "invalid-refresh-token"},
            timeout=10,
        )
        assert resp.status_code == 401
