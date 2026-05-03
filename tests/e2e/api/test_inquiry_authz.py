"""
API Tests: Inquiry Authorization

Ported from crates/api/tests/inquiry_authz_tests.rs
Tests the inquiry respond endpoint's identity, execution-token,
privilege-loop, and ancestor-chain authorization logic.
"""

import os
import time
import uuid

import jwt
import psycopg
import pytest
import requests


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _uid():
    return uuid.uuid4().hex[:8]


def _jwt_secret():
    """JWT secret matching the running API service."""
    return os.environ.get(
        "ATTUNE__SECURITY__JWT_SECRET",
        os.environ.get(
            "JWT_SECRET",
            os.environ.get(
                "ATTUNE_JWT_SECRET", "docker-dev-secret-change-in-production"
            ),
        ),
    )


def _db_url():
    return os.environ.get(
        "DATABASE_URL", "postgresql://attune:attune@localhost:5432/attune"
    )


def _make_access_token(identity_id: int, login: str = "testuser") -> str:
    now = int(time.time())
    payload = {
        "sub": str(identity_id),
        "login": login,
        "token_type": "access",
        "iat": now,
        "exp": now + 3600,
    }
    return jwt.encode(payload, _jwt_secret(), algorithm="HS256")


def _make_execution_token(
    identity_id: int, execution_id: int, action_ref: str
) -> str:
    now = int(time.time())
    payload = {
        "sub": str(identity_id),
        "login": f"execution:{execution_id}",
        "token_type": "execution",
        "scope": "execution",
        "metadata": {
            "execution_id": execution_id,
            "action_ref": action_ref,
        },
        "iat": now,
        "exp": now + 3600,
    }
    return jwt.encode(payload, _jwt_secret(), algorithm="HS256")


def _create_identity(suffix: str) -> int:
    login = f"inqtest_{suffix}"
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO identity (login, display_name, attributes) "
                "VALUES (%s, %s, '{}') RETURNING id",
                (login, login),
            )
            row = cur.fetchone()
            assert row is not None
            identity_id = row[0]
        conn.commit()
    return identity_id


def _create_pack_and_action(suffix: str) -> tuple:
    """Returns (pack_id, action_id, action_ref)."""
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            pack_ref = f"inqauthz_{suffix}"
            cur.execute(
                "INSERT INTO pack (ref, label, version, conf_schema, config, "
                "meta, tags, runtime_deps, dependencies, installers) "
                "VALUES (%s, %s, '1.0.0', '{}', '{}', '{}', '{}', '{}', '{}', '{}') "
                "RETURNING id",
                (pack_ref, f"InqAuthz {suffix}"),
            )
            pack_id = cur.fetchone()[0]

            action_ref = f"{pack_ref}.ask"
            cur.execute(
                "INSERT INTO action (ref, pack, pack_ref, label, entrypoint, "
                "required_worker_runtimes) "
                "VALUES (%s, %s, %s, 'Ask', 'ask.sh', '{}') RETURNING id",
                (action_ref, pack_id, pack_ref),
            )
            action_id = cur.fetchone()[0]
        conn.commit()
    return pack_id, action_id, action_ref


def _create_execution(
    action_id: int, action_ref: str, parent: int | None = None
) -> int:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            if parent is None:
                cur.execute(
                    "INSERT INTO execution (action, action_ref, status, config) "
                    "VALUES (%s, %s, 'running', '{}') RETURNING id",
                    (action_id, action_ref),
                )
            else:
                cur.execute(
                    "INSERT INTO execution (action, action_ref, status, config, parent) "
                    "VALUES (%s, %s, 'running', '{}', %s) RETURNING id",
                    (action_id, action_ref, parent),
                )
            exec_id = cur.fetchone()[0]
        conn.commit()
    return exec_id


def _create_inquiry(execution_id: int, assigned_to: int | None = None) -> int:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO inquiry (execution, prompt, status, assigned_to) "
                "VALUES (%s, 'Approve?', 'pending', %s) RETURNING id",
                (execution_id, assigned_to),
            )
            inquiry_id = cur.fetchone()[0]
        conn.commit()
    return inquiry_id


def _get_inquiry_status(inquiry_id: int) -> str:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT status FROM inquiry WHERE id = %s", (inquiry_id,)
            )
            row = cur.fetchone()
            assert row is not None
            return row[0]


def _respond(base_url: str, inquiry_id: int, token: str) -> requests.Response:
    return requests.post(
        f"{base_url}/api/v1/inquiries/{inquiry_id}/respond",
        headers={"Authorization": f"Bearer {token}"},
        json={"response": {"approved": True}},
        timeout=10,
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.api
class TestInquiryAuthz:
    """Inquiry respond endpoint authorization checks."""

    def test_assignee_with_access_token_can_respond(self, api_base_url):
        """Assignee identity responds with their own access token → 200."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        assignee_id = _create_identity(f"assignee_{uid}")
        exec_id = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_id, assigned_to=assignee_id)

        token = _make_access_token(assignee_id, f"inqtest_assignee_{uid}")
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 200, resp.text

    def test_non_assignee_access_token_is_forbidden(self, api_base_url):
        """Different identity tries to respond → 403."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        assignee_id = _create_identity(f"assignee_{uid}")
        other_id = _create_identity(f"other_{uid}")
        exec_id = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_id, assigned_to=assignee_id)

        token = _make_access_token(other_id, f"inqtest_other_{uid}")
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 403, resp.text

    def test_execution_token_self_response_is_blocked(self, api_base_url):
        """Execution token for the SAME execution that created the inquiry → 403."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        identity_id = _create_identity(f"self_{uid}")
        exec_id = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_id, assigned_to=identity_id)

        token = _make_execution_token(identity_id, exec_id, action_ref)
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 403, resp.text
        assert "privilege loop" in resp.text.lower()

    def test_execution_token_for_different_execution_can_respond_when_assignee(
        self, api_base_url
    ):
        """Token for a different execution, but identity is the assignee → 200."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        identity_id = _create_identity(f"diff_{uid}")

        # Execution A creates the inquiry assigned to identity
        exec_a = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_a, assigned_to=identity_id)

        # Token is for a DIFFERENT execution B (unrelated)
        exec_b = _create_execution(action_id, action_ref)
        token = _make_execution_token(identity_id, exec_b, action_ref)
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 200, resp.text

    def test_execution_token_for_different_execution_blocked_when_not_assignee(
        self, api_base_url
    ):
        """Token for a different execution, identity is NOT the assignee → 403."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        assignee_id = _create_identity(f"assignee2_{uid}")
        caller_id = _create_identity(f"caller_{uid}")

        exec_a = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_a, assigned_to=assignee_id)

        exec_b = _create_execution(action_id, action_ref)
        token = _make_execution_token(caller_id, exec_b, action_ref)
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 403, resp.text

    def test_unassigned_inquiry_accepts_any_authenticated_caller(
        self, api_base_url
    ):
        """No assigned_to set → any authenticated caller can respond → 200."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        caller_id = _create_identity(f"any_{uid}")
        exec_id = _create_execution(action_id, action_ref)
        # No assigned_to
        inquiry_id = _create_inquiry(exec_id, assigned_to=None)

        token = _make_access_token(caller_id, f"inqtest_any_{uid}")
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 200, resp.text

    def test_nested_execution_token_self_response_is_blocked(
        self, api_base_url
    ):
        """Parent A creates inquiry, child B's token tries to respond → 403."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        identity_id = _create_identity(f"nested_{uid}")

        # A creates inquiry
        exec_a = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_a, assigned_to=identity_id)

        # B is a child of A
        exec_b = _create_execution(action_id, action_ref, parent=exec_a)
        token = _make_execution_token(identity_id, exec_b, action_ref)
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 403, resp.text
        assert "descendant" in resp.text.lower() or "privilege loop" in resp.text.lower()

    def test_deeply_nested_execution_token_self_response_is_blocked(
        self, api_base_url
    ):
        """A→B→C chain: C's token tries to respond to A's inquiry → 403."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        identity_id = _create_identity(f"deep_{uid}")

        # A creates inquiry
        exec_a = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_a, assigned_to=identity_id)

        # B is child of A, C is child of B
        exec_b = _create_execution(action_id, action_ref, parent=exec_a)
        exec_c = _create_execution(action_id, action_ref, parent=exec_b)
        token = _make_execution_token(identity_id, exec_c, action_ref)
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 403, resp.text
        assert "descendant" in resp.text.lower() or "privilege loop" in resp.text.lower()

    def test_responded_by_recorded_for_access_token(self, api_base_url):
        """After successful response, verify the inquiry is marked responded."""
        uid = _uid()
        _, action_id, action_ref = _create_pack_and_action(uid)
        identity_id = _create_identity(f"record_{uid}")
        exec_id = _create_execution(action_id, action_ref)
        inquiry_id = _create_inquiry(exec_id, assigned_to=identity_id)

        token = _make_access_token(identity_id, f"inqtest_record_{uid}")
        resp = _respond(api_base_url, inquiry_id, token)
        assert resp.status_code == 200, resp.text

        # Verify DB state
        status = _get_inquiry_status(inquiry_id)
        assert status == "responded"
