"""T3.15: Inquiry notification tests."""

import asyncio
import json
import os
from datetime import datetime, timedelta, timezone

import pytest
import websockets
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, unique_ref


def _notifier_ws_url(client: AttuneClient) -> str:
    base_url = os.getenv("ATTUNE_WS_URL", "ws://localhost:8081").rstrip("/")
    return f"{base_url}/ws?token={client.access_token}"


def _create_execution_for_inquiry(client: AttuneClient, pack_ref: str) -> dict:
    action = create_echo_action(
        client=client,
        pack_ref=pack_ref,
        action_ref=f"inquiry_notify_anchor_{unique_ref()}",
        description="Anchor execution for inquiry notification test",
    )
    return client.create_execution(
        action_ref=action["ref"],
        parameters={"message": "inquiry notification anchor"},
    )


async def _wait_for_inquiry_notification(
    websocket,
    *,
    notification_type: str,
    inquiry_id: int,
    timeout: float = 10.0,
) -> dict:
    deadline = asyncio.get_running_loop().time() + timeout
    while asyncio.get_running_loop().time() < deadline:
        remaining = max(0.1, deadline - asyncio.get_running_loop().time())
        message = json.loads(await asyncio.wait_for(websocket.recv(), timeout=remaining))
        if message.get("type") != "notification":
            continue
        if message.get("notification_type") != notification_type:
            continue
        if message.get("entity_type") != "inquiry":
            continue
        if message.get("entity_id") != inquiry_id:
            continue
        return message

    raise AssertionError(
        f"Did not receive {notification_type!r} notification for inquiry {inquiry_id}"
    )


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
def test_inquiry_creation_notification(client: AttuneClient, test_pack):
    """Creating an inquiry should persist notification-ready metadata."""
    execution = _create_execution_for_inquiry(client, test_pack["ref"])

    inquiry = client.create_inquiry(
        execution_id=execution["id"],
        prompt="Approve the notification test?",
        response_schema={
            "approved": {
                "type": "boolean",
                "description": "Whether the request is approved",
                "required": True,
            }
        },
    )

    assert inquiry["execution"] == execution["id"]
    assert inquiry["prompt"] == "Approve the notification test?"
    assert inquiry["status"] == "pending"
    assert inquiry["response_schema"]["approved"]["type"] == "boolean"
    assert inquiry["created"]
    assert inquiry["updated"]


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
def test_inquiry_response_notification(client: AttuneClient, test_pack):
    """Responding to an inquiry should update it to responded with response data."""
    execution = _create_execution_for_inquiry(client, test_pack["ref"])
    inquiry = client.create_inquiry(
        execution_id=execution["id"],
        prompt="Approve response notification test?",
    )

    response = client.respond_to_inquiry(
        inquiry["id"],
        response_data={"approved": True, "comment": "Approved by E2E"},
    )

    assert response["id"] == inquiry["id"]
    assert response["status"] == "responded"
    assert response["response"] == {"approved": True, "comment": "Approved by E2E"}
    assert response["responded_at"] is not None


@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
def test_inquiry_timeout_notification(client: AttuneClient, test_pack):
    """Expired pending inquiries should time out, notify subscribers, and reject responses."""
    pack_ref = test_pack["ref"]

    async def run_test() -> tuple[dict, dict, dict]:
        async with websockets.connect(_notifier_ws_url(client)) as websocket:
            welcome = json.loads(await asyncio.wait_for(websocket.recv(), timeout=3))
            assert welcome["type"] == "welcome"
            await websocket.send(
                json.dumps({"type": "subscribe", "filter": "entity_type:inquiry"})
            )

            execution = _create_execution_for_inquiry(client, pack_ref)
            timeout_at = datetime.now(timezone.utc) + timedelta(seconds=2)
            inquiry = client.create_inquiry(
                execution_id=execution["id"],
                prompt="This inquiry should time out",
                timeout_at=timeout_at.isoformat(),
            )

            timeout_notification = await _wait_for_inquiry_notification(
                websocket,
                notification_type="inquiry_timeout",
                inquiry_id=inquiry["id"],
                timeout=10,
            )
            timed_out = client.get_inquiry(inquiry["id"])
            return inquiry, timed_out, timeout_notification

    inquiry, timed_out, timeout_notification = asyncio.run(run_test())

    assert timed_out["id"] == inquiry["id"]
    assert timed_out["status"] == "timeout"
    assert timed_out["timeout_at"] is not None
    assert timeout_notification["payload"]["status"] == "timeout"
    assert timeout_notification["payload"]["execution"] == inquiry["execution"]
    with pytest.raises(Exception, match="(timeout|409|400|responded|terminal)"):
        client.respond_to_inquiry(
            inquiry["id"],
            response_data={"approved": True, "comment": "too late"},
        )

@pytest.mark.tier3
@pytest.mark.notifications
@pytest.mark.inquiry
@pytest.mark.websocket
def test_websocket_inquiry_notification_delivery(client: AttuneClient, test_pack):
    """Authenticated WebSocket subscribers receive inquiry create/respond events."""
    pack_ref = test_pack["ref"]

    async def run_test() -> tuple[dict, dict, dict]:
        async with websockets.connect(_notifier_ws_url(client)) as websocket:
            welcome = json.loads(await asyncio.wait_for(websocket.recv(), timeout=3))
            assert welcome["type"] == "welcome"
            await websocket.send(
                json.dumps({"type": "subscribe", "filter": "entity_type:inquiry"})
            )

            execution = _create_execution_for_inquiry(client, pack_ref)
            inquiry = client.create_inquiry(
                execution_id=execution["id"],
                prompt="Approve WebSocket inquiry notification test?",
            )

            created_notification = await _wait_for_inquiry_notification(
                websocket,
                notification_type="inquiry_created",
                inquiry_id=inquiry["id"],
            )

            client.respond_to_inquiry(
                inquiry["id"],
                response_data={"approved": True, "comment": "WebSocket test"},
            )
            responded_notification = await _wait_for_inquiry_notification(
                websocket,
                notification_type="inquiry_responded",
                inquiry_id=inquiry["id"],
            )

            return inquiry, created_notification, responded_notification

    inquiry, created_notification, responded_notification = asyncio.run(run_test())

    assert created_notification["payload"]["status"] == "pending"
    assert created_notification["payload"]["execution"] == inquiry["execution"]
    assert responded_notification["payload"]["status"] == "responded"
    assert responded_notification["payload"]["execution"] == inquiry["execution"]
