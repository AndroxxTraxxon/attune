"""
API Tests: SSE Execution Stream

Ported from crates/api/tests/sse_execution_stream_tests.rs

Tests verify:
1. PostgreSQL LISTEN/NOTIFY correctly triggers notifications
2. The SSE endpoint streams execution updates in real-time
3. Filtering by execution_id works correctly
4. Authentication is properly enforced
5. Streaming all executions (no filter) works
"""

import json
import os
import threading
import time
import uuid

import httpx
import psycopg
import pytest
import requests


def _uid():
    return uuid.uuid4().hex[:8]


def _db_url():
    return os.environ.get(
        "DATABASE_URL", "postgresql://attune:attune@localhost:5432/attune"
    )


def _api_base():
    return os.environ.get("ATTUNE_API_URL", "http://localhost:8080")


def _create_test_pack_and_action(conn):
    """Create a test pack and action directly via DB, return (pack_id, action_id, action_ref)."""
    uid = _uid()
    pack_ref = f"sse_test_{uid}"
    action_ref = f"{pack_ref}.action"

    with conn.cursor() as cur:
        cur.execute(
            """INSERT INTO pack (ref, label, description, version, conf_schema, config, meta, tags, runtime_deps, dependencies, is_standard, installers)
            VALUES (%s, %s, %s, '1.0.0', '{}', '{}', '{}', '{}', '{}', '{}', false, '{}')
            RETURNING id""",
            (pack_ref, f"SSE Test Pack {uid}", "Pack for SSE testing"),
        )
        pack_id = cur.fetchone()[0]

        cur.execute(
            """INSERT INTO action (ref, pack, pack_ref, label, description, entrypoint, param_schema, out_schema, is_adhoc, accesses_mcp, required_worker_runtimes)
            VALUES (%s, %s, %s, %s, %s, 'test.sh', NULL, NULL, false, false, '{}')
            RETURNING id""",
            (action_ref, pack_id, pack_ref, "Test Action", "SSE test action"),
        )
        action_id = cur.fetchone()[0]

    conn.commit()
    return pack_id, action_id, action_ref


def _create_execution(conn, action_id, action_ref):
    """Create a scheduled execution directly via DB, return execution id."""
    with conn.cursor() as cur:
        cur.execute(
            """INSERT INTO execution (action, action_ref, status, config)
            VALUES (%s, %s, 'scheduled', '{}')
            RETURNING id""",
            (action_id, action_ref),
        )
        exec_id = cur.fetchone()[0]
    conn.commit()
    return exec_id


def _update_execution_status(conn, exec_id, status):
    """Update execution status via DB (triggers PG NOTIFY)."""
    with conn.cursor() as cur:
        cur.execute(
            "UPDATE execution SET status = %s WHERE id = %s",
            (status, exec_id),
        )
    conn.commit()


def _get_auth_token():
    """Login and return an access token."""
    base = _api_base()
    login = os.environ.get("TEST_USER_LOGIN", "test@attune.local")
    password = os.environ.get("TEST_USER_PASSWORD", "TestPass123!")
    resp = requests.post(
        f"{base}/auth/login",
        json={"login": login, "password": password},
        timeout=10,
    )
    resp.raise_for_status()
    return resp.json()["data"]["access_token"]


def _parse_sse_events(lines):
    """Parse SSE event lines into list of parsed JSON data payloads."""
    events = []
    for line in lines:
        if line.startswith("data: "):
            try:
                data = json.loads(line[6:])
                events.append(data)
            except json.JSONDecodeError:
                pass
    return events


def _collect_sse_events(url, token, timeout_sec=10, stop_event=None):
    """
    Connect to SSE stream and collect events until timeout or stop_event is set.
    Returns list of parsed JSON event payloads.
    """
    events = []
    try:
        timeout = httpx.Timeout(timeout_sec + 5, connect=5.0, read=2.0)
        with httpx.Client(timeout=timeout) as http:
            with http.stream(
                "GET",
                url,
                headers={"Authorization": f"Bearer {token}"},
                timeout=timeout,
            ) as resp:
                resp.raise_for_status()
                deadline = time.time() + timeout_sec
                lines = resp.iter_lines()
                while True:
                    if stop_event and stop_event.is_set():
                        break
                    if time.time() > deadline:
                        break
                    try:
                        line = next(lines)
                    except httpx.ReadTimeout:
                        if stop_event and stop_event.is_set():
                            break
                        continue
                    except StopIteration:
                        break
                    if line.startswith("data: "):
                        try:
                            data = json.loads(line[6:])
                            events.append(data)
                        except json.JSONDecodeError:
                            pass
    except (httpx.ReadTimeout, httpx.RemoteProtocolError, httpx.StreamClosed):
        pass
    return events


@pytest.mark.api
class TestSSEExecutionStream:
    """SSE execution stream endpoint tests."""

    def test_sse_stream_receives_execution_updates(self, api_base_url):
        """
        Connect to SSE stream for an execution, update status via DB
        (running → succeeded), verify both events received.
        """
        token = _get_auth_token()
        conn = psycopg.connect(_db_url())

        try:
            _pack_id, action_id, action_ref = _create_test_pack_and_action(conn)
            exec_id = _create_execution(conn, action_id, action_ref)

            sse_url = f"{api_base_url}/api/v1/executions/stream?execution_id={exec_id}"
            stop = threading.Event()
            collected = []

            def consume_sse():
                nonlocal collected
                collected = _collect_sse_events(sse_url, token, timeout_sec=12, stop_event=stop)

            consumer = threading.Thread(target=consume_sse, daemon=True)
            consumer.start()

            # Wait for SSE connection to establish
            time.sleep(1.0)

            # Update to running
            _update_execution_status(conn, exec_id, "running")
            time.sleep(0.5)

            # Update to succeeded
            _update_execution_status(conn, exec_id, "completed")
            time.sleep(1.0)

            stop.set()
            consumer.join(timeout=5)

            # Filter for execution updates matching our ID
            our_events = [
                e for e in collected
                if e.get("entity_type") == "execution"
                and e.get("entity_id") == exec_id
            ]

            statuses = []
            for ev in our_events:
                payload = ev.get("payload") or ev.get("data") or ev
                status = payload.get("status") if isinstance(payload, dict) else None
                if status:
                    statuses.append(status)

            assert "running" in statuses, (
                f"Should have received 'running' status update. Got events: {collected}"
            )
            assert "completed" in statuses, (
                f"Should have received 'completed' status update. Got events: {collected}"
            )
        finally:
            conn.close()

    def test_sse_stream_filters_by_execution_id(self, api_base_url):
        """
        Subscribe to exec1 only, update both exec1 and exec2,
        verify only exec1 events received.
        """
        token = _get_auth_token()
        conn = psycopg.connect(_db_url())

        try:
            _pack_id, action_id, action_ref = _create_test_pack_and_action(conn)
            exec1_id = _create_execution(conn, action_id, action_ref)
            exec2_id = _create_execution(conn, action_id, action_ref)

            # Subscribe only to exec1
            sse_url = f"{api_base_url}/api/v1/executions/stream?execution_id={exec1_id}"
            stop = threading.Event()
            collected = []

            def consume_sse():
                nonlocal collected
                collected = _collect_sse_events(sse_url, token, timeout_sec=10, stop_event=stop)

            consumer = threading.Thread(target=consume_sse, daemon=True)
            consumer.start()
            time.sleep(1.0)

            # Update exec2 first (should NOT appear)
            _update_execution_status(conn, exec2_id, "running")
            time.sleep(0.3)

            # Update exec1 (SHOULD appear)
            _update_execution_status(conn, exec1_id, "running")
            time.sleep(1.0)

            stop.set()
            consumer.join(timeout=5)

            exec1_events = [
                e for e in collected
                if e.get("entity_type") == "execution"
                and e.get("entity_id") == exec1_id
            ]
            exec2_events = [
                e for e in collected
                if e.get("entity_type") == "execution"
                and e.get("entity_id") == exec2_id
            ]

            assert len(exec1_events) > 0, (
                f"Should have received update for exec1 ({exec1_id}). Got: {collected}"
            )
            assert len(exec2_events) == 0, (
                f"Should NOT have received update for exec2 ({exec2_id}). Got: {collected}"
            )
        finally:
            conn.close()

    def test_sse_stream_requires_authentication(self, api_base_url):
        """Hit SSE endpoint without auth, expect 401."""
        sse_url = f"{api_base_url}/api/v1/executions/stream"
        resp = requests.get(sse_url, timeout=10)
        assert resp.status_code == 401, (
            f"Expected 401 Unauthorized, got {resp.status_code}"
        )

    def test_sse_stream_all_executions(self, api_base_url):
        """
        Subscribe without execution_id filter, update two execs,
        verify both received.
        """
        token = _get_auth_token()
        conn = psycopg.connect(_db_url())

        try:
            _pack_id, action_id, action_ref = _create_test_pack_and_action(conn)
            exec1_id = _create_execution(conn, action_id, action_ref)
            exec2_id = _create_execution(conn, action_id, action_ref)

            # Subscribe to ALL (no filter)
            sse_url = f"{api_base_url}/api/v1/executions/stream"
            stop = threading.Event()
            collected = []

            def consume_sse():
                nonlocal collected
                collected = _collect_sse_events(sse_url, token, timeout_sec=12, stop_event=stop)

            consumer = threading.Thread(target=consume_sse, daemon=True)
            consumer.start()
            time.sleep(1.0)

            # Update exec1
            _update_execution_status(conn, exec1_id, "running")
            time.sleep(0.3)

            # Update exec2
            _update_execution_status(conn, exec2_id, "running")
            time.sleep(1.5)

            stop.set()
            consumer.join(timeout=5)

            received_ids = {
                e.get("entity_id")
                for e in collected
                if e.get("entity_type") == "execution"
            }

            assert exec1_id in received_ids, (
                f"Should have received update for exec1 ({exec1_id}). "
                f"Received IDs: {received_ids}"
            )
            assert exec2_id in received_ids, (
                f"Should have received update for exec2 ({exec2_id}). "
                f"Received IDs: {received_ids}"
            )
        finally:
            conn.close()

    def test_postgresql_notify_trigger_fires(self):
        """
        Listen on PG NOTIFY channel, update execution,
        verify notification fires with correct payload.
        """
        conn = psycopg.connect(_db_url(), autocommit=True)
        writer_conn = psycopg.connect(_db_url())

        try:
            # Set up test data using writer connection
            _pack_id, action_id, action_ref = _create_test_pack_and_action(writer_conn)
            exec_id = _create_execution(writer_conn, action_id, action_ref)

            # Listen on the execution status changed channel
            conn.execute("LISTEN execution_status_changed")

            received = False
            received_payload = None

            def update_execution():
                """Update execution in a separate thread after short delay."""
                time.sleep(0.5)
                _update_execution_status(writer_conn, exec_id, "running")

            updater = threading.Thread(target=update_execution, daemon=True)
            updater.start()

            # Wait for notification
            gen = conn.notifies(timeout=10)
            for notify in gen:
                payload = json.loads(notify.payload)
                if payload.get("entity_id") == exec_id:
                    received = True
                    received_payload = payload
                    break

            updater.join(timeout=5)

            assert received, (
                f"Should have received PostgreSQL NOTIFY for execution {exec_id}"
            )
            assert received_payload["entity_type"] == "execution"
            assert received_payload["entity_id"] == exec_id
        finally:
            conn.close()
            writer_conn.close()
