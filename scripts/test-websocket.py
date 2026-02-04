#!/usr/bin/env python3
"""
Simple WebSocket test script for Attune Notifier Service

Usage: python3 scripts/test-websocket.py
"""

import asyncio
import json
import sys
from datetime import datetime

try:
    import websockets
except ImportError:
    print("❌ Error: websockets library not installed")
    print("   Install with: pip3 install websockets")
    sys.exit(1)

WS_URL = "ws://localhost:8081/ws"
RECONNECT_DELAY = 3  # seconds


async def test_websocket():
    """Connect to WebSocket and test event notifications"""

    print(f"🔌 Connecting to Attune Notifier Service...")
    print(f"   URL: {WS_URL}\n")

    message_count = 0
    event_count = 0

    try:
        async with websockets.connect(WS_URL) as websocket:
            print("✅ Connected to notifier service\n")

            # Subscribe to event notifications
            subscribe_msg = {"type": "subscribe", "filter": "entity_type:event"}
            await websocket.send(json.dumps(subscribe_msg))
            print("📡 Subscribed to entity_type:event\n")
            print("⏱️  Waiting for notifications... (Press Ctrl+C to exit)\n")

            # Listen for messages
            async for message in websocket:
                message_count += 1

                try:
                    data = json.loads(message)

                    if data.get("type") == "welcome":
                        timestamp = datetime.now().strftime("%H:%M:%S")
                        print(f"[{timestamp}] 👋 Welcome message received")
                        print(f"           Client ID: {data.get('client_id')}")
                        print(f"           Message: {data.get('message')}\n")

                    elif "notification_type" in data:
                        # This is a notification
                        event_count += 1
                        timestamp = datetime.now().strftime("%H:%M:%S")

                        print(f"[{timestamp}] 🔔 Event notification #{event_count}")
                        print(f"           Type: {data.get('notification_type')}")
                        print(
                            f"           Entity: {data.get('entity_type')} (ID: {data.get('entity_id')})"
                        )

                        payload_data = data.get("payload", {}).get("data", {})
                        if payload_data:
                            print(
                                f"           Trigger: {payload_data.get('trigger_ref', 'N/A')}"
                            )
                            print(
                                f"           Source: {payload_data.get('source_ref', 'N/A')}"
                            )
                        print()

                    else:
                        print(f"📨 Unknown message format: {data}\n")

                except json.JSONDecodeError as e:
                    print(f"❌ Failed to parse message: {e}")
                    print(f"   Raw data: {message}\n")

    except websockets.exceptions.WebSocketException as e:
        print(f"\n❌ WebSocket error: {e}")
    except KeyboardInterrupt:
        print(f"\n\n👋 Shutting down...")
        print(f"   Total messages received: {message_count}")
        print(f"   Event notifications: {event_count}")
    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        import traceback

        traceback.print_exc()


def main():
    """Main entry point"""
    try:
        asyncio.run(test_websocket())
    except KeyboardInterrupt:
        print("\n\nExiting...")
        sys.exit(0)


if __name__ == "__main__":
    main()
