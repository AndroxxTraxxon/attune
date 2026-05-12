"""
API Tests: Operational Visibility

Verifies worker health/cordon API behavior, dead-worker execution
reconciliation, system alert registration, and sensor log endpoint shape.
"""

import json
import os
import time
import uuid

import psycopg
import pytest


def _uid():
    return uuid.uuid4().hex[:8]


def _db_url():
    return os.environ.get(
        "DATABASE_URL", "postgresql://attune:attune@localhost:5432/attune"
    )


def _create_worker(
    name: str,
    *,
    role: str = "action",
    status: str = "active",
    heartbeat_sql: str = "NOW()",
    capabilities: dict | None = None,
) -> int:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                f"""
                INSERT INTO worker (
                    name, worker_type, worker_role, status, capabilities, meta,
                    last_heartbeat
                )
                VALUES (%s, 'local', %s, %s, %s::jsonb, '{{}}'::jsonb, {heartbeat_sql})
                RETURNING id
                """,
                (name, role, status, json.dumps(capabilities or {})),
            )
            row = cur.fetchone()
            assert row is not None
            worker_id = row[0]
        conn.commit()
    return worker_id


def _delete_worker(worker_id: int):
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute("DELETE FROM worker WHERE id = %s", (worker_id,))
        conn.commit()


def _create_running_execution(worker_id: int, action_ref: str) -> int:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO execution (
                    action_ref, config, env_vars, worker, status, started_at,
                    created, updated
                )
                VALUES (
                    %s, '{}'::jsonb, '{}'::jsonb, %s, 'running',
                    NOW() - INTERVAL '10 minutes',
                    NOW() - INTERVAL '10 minutes',
                    NOW() - INTERVAL '10 minutes'
                )
                RETURNING id
                """,
                (action_ref, worker_id),
            )
            row = cur.fetchone()
            assert row is not None
            execution_id = row[0]
        conn.commit()
    return execution_id


def _delete_execution(execution_id: int):
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute("DELETE FROM execution WHERE id = %s", (execution_id,))
        conn.commit()


def _find_worker(items: list[dict], worker_id: int) -> dict:
    for item in items:
        if item.get("id") == worker_id:
            return item
    raise AssertionError(f"Worker {worker_id} not found in response: {items}")


def _list_workers(client, **params) -> list[dict]:
    resp = client.session.get(
        f"{client.base_url}/api/v1/workers", params=params, timeout=15
    )
    assert resp.status_code == 200, resp.text
    body = resp.json()
    return body.get("items", body.get("data", body))


def _delete_alert_events(correlation_id: str):
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                """
                DELETE FROM event
                WHERE trigger_ref = 'core.alert'
                  AND payload->>'correlation_id' = %s
                """,
                (correlation_id,),
            )
        conn.commit()


def _alert_event_count(correlation_id: str) -> int:
    with psycopg.connect(_db_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(
                """
                SELECT COUNT(*)
                FROM event
                WHERE trigger_ref = 'core.alert'
                  AND payload->>'correlation_id' = %s
                """,
                (correlation_id,),
            )
            row = cur.fetchone()
            return row[0] if row else 0


@pytest.mark.integration
class TestOperationalVisibility:
    def test_worker_health_filters_and_cordon_lifecycle(self, client):
        worker_name = f"e2e-stale-worker-{_uid()}"
        worker_id = _create_worker(
            worker_name,
            heartbeat_sql="NOW() - INTERVAL '10 minutes'",
            capabilities={"runtimes": [{"name": "shell", "versions": []}]},
        )

        try:
            offline_workers = _list_workers(
                client, role="action", health_state="offline", page_size=100
            )
            worker = _find_worker(offline_workers, worker_id)
            assert worker["name"] == worker_name
            assert worker["worker_role"] == "action"
            assert worker["heartbeat_stale"] is True
            assert worker["health_state"] == "offline"
            assert worker["cordoned"] is False
            assert isinstance(worker["heartbeat_age_seconds"], int)

            reason = f"e2e maintenance {_uid()}"
            cordon_resp = client.session.post(
                f"{client.base_url}/api/v1/workers/{worker_id}/cordon",
                json={"reason": reason},
                timeout=15,
            )
            assert cordon_resp.status_code == 200, cordon_resp.text
            cordoned = cordon_resp.json()
            assert cordoned["cordoned"] is True
            assert cordoned["cordon_reason"] == reason
            assert cordoned["health_state"] == "cordoned"
            assert cordoned["cordoned_by"] is not None
            assert cordoned["cordoned_at"] is not None

            cordoned_workers = _list_workers(
                client, cordoned=True, health_state="cordoned", page_size=100
            )
            assert _find_worker(cordoned_workers, worker_id)["cordoned"] is True

            uncordon_resp = client.session.post(
                f"{client.base_url}/api/v1/workers/{worker_id}/uncordon",
                timeout=15,
            )
            assert uncordon_resp.status_code == 200, uncordon_resp.text
            uncordoned = uncordon_resp.json()
            assert uncordoned["cordoned"] is False
            assert uncordoned["cordon_reason"] is None
            assert uncordoned["cordoned_by"] is None
            assert uncordoned["cordoned_at"] is None
            assert uncordoned["health_state"] == "offline"
        finally:
            _delete_worker(worker_id)

    def test_sensor_worker_health_load_summary(self, client):
        worker_name = f"e2e-sensor-worker-{_uid()}"
        worker_id = _create_worker(
            worker_name,
            role="sensor",
            capabilities={
                "labels": {"zone": "e2e"},
                "taints": [{"key": "dedicated", "value": "sensors"}],
                "max_concurrent_sensors": 5,
                "sensor_processes_running": 2,
                "sensor_processes_monitored": 3,
                "active_rules": 1,
            },
        )

        try:
            workers = _list_workers(
                client, role="sensor", health_state="busy", page_size=100
            )
            worker = _find_worker(workers, worker_id)
            assert worker["worker_role"] == "sensor"
            assert worker["status"] == "busy"
            assert worker["health_state"] == "busy"
            assert worker["heartbeat_stale"] is False
            assert worker["load"]["sensor_processes_running"] == 2
            assert worker["load"]["sensor_processes_monitored"] == 3
            assert worker["load"]["active_rules"] == 1
            assert worker["load"]["max_concurrent_sensors"] == 5
            assert worker["load"]["utilization_percent"] == 40
        finally:
            _delete_worker(worker_id)

    @pytest.mark.slow
    def test_running_execution_on_dead_worker_becomes_abandoned(self, client):
        worker_name = f"e2e-dead-worker-{_uid()}"
        worker_id = _create_worker(
            worker_name,
            status="inactive",
            heartbeat_sql="NOW() - INTERVAL '10 minutes'",
        )
        action_ref = f"e2e.operational_visibility.{_uid()}"
        execution_id = _create_running_execution(worker_id, action_ref)
        correlation_id = f"execution:{execution_id}:worker_unavailable"

        try:
            deadline = time.time() + 75
            execution = None
            while time.time() < deadline:
                execution = client.get_execution(execution_id)
                if execution["status"] == "abandoned":
                    break
                time.sleep(2)

            assert execution is not None
            assert execution["status"] == "abandoned", execution
            result = execution.get("result") or {}
            assert result["abandoned_by"] == "execution_timeout_monitor"
            assert result["original_status"] == "running"
            assert result["worker"]["id"] == worker_id
            assert result["worker"]["cordoned"] is False

            deadline = time.time() + 15
            while time.time() < deadline and _alert_event_count(correlation_id) == 0:
                time.sleep(1)
            assert _alert_event_count(correlation_id) >= 1
        finally:
            _delete_alert_events(correlation_id)
            _delete_execution(execution_id)
            _delete_worker(worker_id)

    def test_core_alert_trigger_is_registered(self, client):
        resp = client.session.get(
            f"{client.base_url}/api/v1/triggers/core.alert", timeout=15
        )
        assert resp.status_code == 200, resp.text
        trigger = resp.json()["data"]
        assert trigger["ref"] == "core.alert"
        assert trigger["pack_ref"] == "core"
        assert trigger["enabled"] is True

        schema = trigger.get("out_schema") or {}
        for field in [
            "severity",
            "category",
            "failure_type",
            "component_type",
            "observed_at",
            "summary",
            "details",
        ]:
            assert field in schema

    def test_core_queue_lifecycle_triggers_are_registered(self, client):
        for trigger_ref in ["core.queue_started", "core.queue_empty"]:
            resp = client.session.get(
                f"{client.base_url}/api/v1/triggers/{trigger_ref}", timeout=15
            )
            assert resp.status_code == 200, resp.text
            trigger = resp.json()["data"]
            assert trigger["ref"] == trigger_ref
            assert trigger["pack_ref"] == "core"
            assert trigger["enabled"] is True

            schema = trigger.get("out_schema") or {}
            for field in [
                "event_type",
                "queue_id",
                "queue_ref",
                "dispatch_id",
                "execution_id",
                "leased_item_count",
                "observed_at",
            ]:
                assert field in schema

    def test_sensor_log_summary_endpoint_exposes_stream_refs(self, client):
        sensor_ref = "core.timer_sensor"
        resp = client.session.get(
            f"{client.base_url}/api/v1/sensors/{sensor_ref}/logs", timeout=15
        )
        assert resp.status_code == 200, resp.text
        body = resp.json()
        assert body["sensor_ref"] == sensor_ref

        logs = {entry["stream"]: entry for entry in body["logs"]}
        assert logs["stdout"]["artifact_ref"] == f"sensor.{sensor_ref}.stdout"
        assert logs["stderr"]["artifact_ref"] == f"sensor.{sensor_ref}.stderr"

        invalid_stream = client.session.get(
            f"{client.base_url}/api/v1/sensors/{sensor_ref}/logs/events",
            params={"tail": 10},
            timeout=15,
        )
        assert invalid_stream.status_code == 422, invalid_stream.text
