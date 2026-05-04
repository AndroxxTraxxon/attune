"""
API Tests: Webhook Security Features

Ported from crates/api/tests/webhook_security_tests.rs
Tests HMAC signature verification, rate limiting, IP whitelisting,
payload size limits, and combined security features.
"""

import hashlib
import hmac
import json
import os
import uuid

import psycopg
import pytest
import requests


def _uid():
    return uuid.uuid4().hex[:8]


def _db_url():
    return os.environ.get(
        "DATABASE_URL", "postgresql://attune:attune@localhost:5432/attune"
    )


def _set_webhook_config(trigger_ref: str, config: dict):
    """Set webhook_config JSONB directly in the database."""
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE trigger SET webhook_config = %s::jsonb WHERE ref = %s",
                (json.dumps(config), trigger_ref),
            )
        conn.commit()


def _get_trigger_id(trigger_ref: str) -> int:
    """Get trigger ID by ref."""
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute("SELECT id FROM trigger WHERE ref = %s", (trigger_ref,))
            row = cur.fetchone()
            if row:
                return row[0]
    raise ValueError(f"Trigger not found: {trigger_ref}")


def _generate_hmac_signature(payload: bytes, secret: str, algorithm: str) -> str:
    """Generate HMAC signature in the format expected by the API."""
    if algorithm == "sha256":
        digest = hmac.new(secret.encode(), payload, hashlib.sha256).hexdigest()
        return f"sha256={digest}"
    elif algorithm == "sha512":
        digest = hmac.new(secret.encode(), payload, hashlib.sha512).hexdigest()
        return f"sha512={digest}"
    elif algorithm == "sha1":
        digest = hmac.new(secret.encode(), payload, hashlib.sha1).hexdigest()
        return f"sha1={digest}"
    else:
        raise ValueError(f"Unsupported algorithm: {algorithm}")


def _setup_webhook_trigger(client) -> tuple:
    """Create a pack/trigger and enable webhook. Returns (trigger_ref, webhook_key)."""
    uid = _uid()
    pack_ref = f"whsec_{uid}"
    trigger_ref = f"{pack_ref}.trigger"

    client.create_pack(ref=pack_ref, label=f"Security Test {uid}")
    client.create_trigger(pack_ref=pack_ref, ref=trigger_ref, label="Sec Trigger")
    enable_resp = client.post(f"/api/v1/triggers/{trigger_ref}/webhooks/enable")
    webhook_key = enable_resp["data"]["webhook_key"]

    return trigger_ref, webhook_key


# ============================================================================
# HMAC SIGNATURE TESTS
# ============================================================================


@pytest.mark.api
class TestWebhookHMAC:
    """HMAC signature verification tests."""

    def test_hmac_sha256_valid(self, client):
        """Valid HMAC-SHA256 signature passes verification."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        hmac_secret = "test-secret-key-12345"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": hmac_secret, "algorithm": "sha256"},
        })

        payload = json.dumps({"payload": {"event": "test", "data": {"foo": "bar"}}})
        payload_bytes = payload.encode()
        signature = _generate_hmac_signature(payload_bytes, hmac_secret, "sha256")

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload_bytes,
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": signature,
            },
            timeout=10,
        )
        assert resp.status_code == 200

    def test_hmac_sha512_valid(self, client):
        """Valid HMAC-SHA512 signature passes verification."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        hmac_secret = "test-secret-sha512"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": hmac_secret, "algorithm": "sha512"},
        })

        payload = json.dumps({"payload": {"message": "test"}})
        payload_bytes = payload.encode()
        signature = _generate_hmac_signature(payload_bytes, hmac_secret, "sha512")

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload_bytes,
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": signature,
            },
            timeout=10,
        )
        assert resp.status_code == 200

    def test_hmac_invalid_signature(self, client):
        """Invalid HMAC signature is rejected with 401."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        hmac_secret = "test-secret-key"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": hmac_secret, "algorithm": "sha256"},
        })

        payload = json.dumps({"payload": {"message": "test"}})

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": "sha256=invalid_signature_here",
            },
            timeout=10,
        )
        assert resp.status_code == 401

    def test_hmac_missing_signature(self, client):
        """Missing signature when HMAC is enabled returns 401."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": "secret", "algorithm": "sha256"},
        })

        payload = json.dumps({"payload": {"message": "test"}})

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_hmac_wrong_secret(self, client):
        """Signature computed with wrong secret is rejected."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        correct_secret = "correct-secret"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": correct_secret, "algorithm": "sha256"},
        })

        payload = json.dumps({"payload": {"message": "test"}})
        payload_bytes = payload.encode()
        # Sign with wrong secret
        wrong_signature = _generate_hmac_signature(payload_bytes, "wrong-secret", "sha256")

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload_bytes,
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": wrong_signature,
            },
            timeout=10,
        )
        assert resp.status_code == 401

    def test_hmac_hub_signature_header(self, client):
        """X-Hub-Signature-256 header also accepted for HMAC."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        hmac_secret = "hub-secret"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": hmac_secret, "algorithm": "sha256"},
        })

        payload = json.dumps({"payload": {"event": "push"}})
        payload_bytes = payload.encode()
        signature = _generate_hmac_signature(payload_bytes, hmac_secret, "sha256")

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload_bytes,
            headers={
                "Content-Type": "application/json",
                "X-Hub-Signature-256": signature,
            },
            timeout=10,
        )
        assert resp.status_code == 200


# ============================================================================
# RATE LIMITING TESTS
# ============================================================================


@pytest.mark.api
class TestWebhookRateLimit:
    """Rate limiting tests."""

    def test_rate_limit_enforced(self, client):
        """Exceeding rate limit returns 429."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "rate_limit": {"enabled": True, "requests": 3, "window_seconds": 60},
        })

        s = requests.Session()
        payload = json.dumps({"payload": {"message": "test"}})

        # First 3 should succeed
        for i in range(3):
            resp = s.post(
                f"{client.base_url}/api/v1/webhooks/{webhook_key}",
                data=payload.encode(),
                headers={"Content-Type": "application/json"},
                timeout=10,
            )
            assert resp.status_code == 200, f"Request {i + 1} should succeed"

        # 4th should be rate limited
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 429

    def test_rate_limit_disabled_allows_many_requests(self, client):
        """Without rate limiting, many requests succeed."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        # No rate_limit config set — default is disabled

        s = requests.Session()
        payload = json.dumps({"payload": {"message": "test"}})

        for _ in range(10):
            resp = s.post(
                f"{client.base_url}/api/v1/webhooks/{webhook_key}",
                data=payload.encode(),
                headers={"Content-Type": "application/json"},
                timeout=10,
            )
            assert resp.status_code == 200


# ============================================================================
# IP WHITELISTING TESTS
# ============================================================================


@pytest.mark.api
class TestWebhookIPWhitelist:
    """IP whitelist tests."""

    def test_ip_whitelist_allowed_cidr(self, client):
        """IP within whitelisted CIDR range is allowed."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "ip_whitelist": {
                "enabled": True,
                "ips": ["192.168.1.0/24", "10.0.0.1"],
            },
        })

        s = requests.Session()
        payload = json.dumps({"payload": {"message": "test"}})

        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={
                "Content-Type": "application/json",
                "X-Forwarded-For": "192.168.1.100",
            },
            timeout=10,
        )
        assert resp.status_code == 200

    def test_ip_whitelist_allowed_exact(self, client):
        """Exact IP match in whitelist is allowed."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "ip_whitelist": {
                "enabled": True,
                "ips": ["192.168.1.0/24", "10.0.0.1"],
            },
        })

        s = requests.Session()
        payload = json.dumps({"payload": {"message": "test"}})

        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={
                "Content-Type": "application/json",
                "X-Forwarded-For": "10.0.0.1",
            },
            timeout=10,
        )
        assert resp.status_code == 200

    def test_ip_whitelist_blocked(self, client):
        """IP not in whitelist returns 403."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "ip_whitelist": {"enabled": True, "ips": ["192.168.1.0/24"]},
        })

        s = requests.Session()
        payload = json.dumps({"payload": {"message": "test"}})

        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={
                "Content-Type": "application/json",
                "X-Forwarded-For": "8.8.8.8",
            },
            timeout=10,
        )
        assert resp.status_code == 403


# ============================================================================
# PAYLOAD SIZE LIMIT TESTS
# ============================================================================


@pytest.mark.api
class TestWebhookPayloadSize:
    """Payload size limit tests."""

    def test_payload_size_limit_enforced(self, client):
        """Payload exceeding size limit returns 400."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        # Set 1 KB limit
        _set_webhook_config(trigger_ref, {"payload_size_limit_kb": 1})

        # Create payload > 1 KB
        large_data = "x" * 2000
        payload = json.dumps({"payload": {"large_field": large_data}})

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 400

    def test_payload_size_within_limit(self, client):
        """Payload within size limit succeeds."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        # Set 10 KB limit
        _set_webhook_config(trigger_ref, {"payload_size_limit_kb": 10})

        # Small payload
        payload = json.dumps({"payload": {"message": "This is a small payload"}})

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 200


# ============================================================================
# COMBINED SECURITY FEATURES TESTS
# ============================================================================


@pytest.mark.api
class TestWebhookCombinedSecurity:
    """Combined security features tests."""

    def test_all_security_features_pass(self, client):
        """Request passing all security checks succeeds."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)
        hmac_secret = "all-features-secret"

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": hmac_secret, "algorithm": "sha256"},
            "rate_limit": {"enabled": True, "requests": 10, "window_seconds": 60},
            "ip_whitelist": {"enabled": True, "ips": ["192.168.1.0/24"]},
            "payload_size_limit_kb": 10,
        })

        payload = json.dumps({"payload": {"message": "test with all features"}})
        payload_bytes = payload.encode()
        signature = _generate_hmac_signature(payload_bytes, hmac_secret, "sha256")

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload_bytes,
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": signature,
                "X-Forwarded-For": "192.168.1.50",
            },
            timeout=10,
        )
        assert resp.status_code == 200

    def test_multiple_security_failures(self, client):
        """Request failing IP whitelist is rejected with 403."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        _set_webhook_config(trigger_ref, {
            "hmac": {"enabled": True, "secret": "secret", "algorithm": "sha256"},
            "ip_whitelist": {"enabled": True, "ips": ["10.0.0.0/8"]},
        })

        payload = json.dumps({"payload": {"message": "test"}})

        # Wrong IP + missing signature — should fail on IP first
        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=payload.encode(),
            headers={
                "Content-Type": "application/json",
                "X-Forwarded-For": "8.8.8.8",
            },
            timeout=10,
        )
        assert resp.status_code == 403


# ============================================================================
# EDGE CASES AND ERROR SCENARIOS
# ============================================================================


@pytest.mark.api
class TestWebhookEdgeCases:
    """Edge cases and error scenarios."""

    def test_malformed_json(self, client):
        """Malformed JSON body returns 400."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=b"{invalid json here",
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 400 or resp.status_code == 422

    def test_empty_payload(self, client):
        """Empty body returns 400."""
        trigger_ref, webhook_key = _setup_webhook_trigger(client)

        s = requests.Session()
        resp = s.post(
            f"{client.base_url}/api/v1/webhooks/{webhook_key}",
            data=b"",
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert resp.status_code == 400 or resp.status_code == 422
