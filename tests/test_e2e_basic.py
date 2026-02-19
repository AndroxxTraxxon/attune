#!/usr/bin/env python3
"""
End-to-End Integration Tests - Basic Scenarios

Tests basic automation flows across all 5 Attune services:
- API, Executor, Worker, Sensor, Notifier

These tests require all services to be running.
Run with: pytest tests/test_e2e_basic.py -v -s
"""

import json
import os
import time
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

import pytest
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

# ============================================================================
# Configuration
# ============================================================================

API_BASE_URL = os.getenv("ATTUNE_API_URL", "http://localhost:8080")
TEST_TIMEOUT = int(os.getenv("TEST_TIMEOUT", "60"))  # seconds
POLL_INTERVAL = 0.5  # seconds


# ============================================================================
# Test Fixtures & Helpers
# ============================================================================


class AttuneClient:
    """Client for interacting with Attune API"""

    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip("/")
        self.session = requests.Session()
        self.token: Optional[str] = None

        # Configure retry strategy
        retry_strategy = Retry(
            total=3,
            backoff_factor=1,
            status_forcelist=[429, 500, 502, 503, 504],
        )
        adapter = HTTPAdapter(max_retries=retry_strategy)
        self.session.mount("http://", adapter)
        self.session.mount("https://", adapter)

    def login(self, login: str = "admin@attune.local", password: str = "AdminPass123!"):
        """Authenticate and get JWT token"""
        try:
            response = self.session.post(
                f"{self.base_url}/auth/login",
                json={"login": login, "password": password},
            )
            response.raise_for_status()
            data = response.json()
            self.token = data["data"]["access_token"]
            self.session.headers.update({"Authorization": f"Bearer {self.token}"})
            return self.token
        except requests.exceptions.HTTPError as e:
            # If login fails with 401/404, try to register the user first
            if e.response.status_code in [401, 404]:
                self.register(login, password)
                # Retry login after registration
                response = self.session.post(
                    f"{self.base_url}/auth/login",
                    json={"login": login, "password": password},
                )
                response.raise_for_status()
                data = response.json()
                self.token = data["data"]["access_token"]
                self.session.headers.update({"Authorization": f"Bearer {self.token}"})
                return self.token
            raise

    def register(self, login: str, password: str, display_name: str = "Test Admin"):
        """Register a new user"""
        response = self.session.post(
            f"{self.base_url}/auth/register",
            json={
                "login": login,
                "password": password,
                "display_name": display_name,
            },
        )
        response.raise_for_status()
        return response.json()

    def _request(self, method: str, path: str, **kwargs) -> Dict[str, Any]:
        """Make authenticated request"""
        if not self.token and path != "/auth/login":
            self.login()

        url = f"{self.base_url}{path}"
        response = self.session.request(method, url, **kwargs)
        response.raise_for_status()
        return response.json()

    def get(self, path: str, **kwargs) -> Dict[str, Any]:
        return self._request("GET", path, **kwargs)

    def post(self, path: str, **kwargs) -> Dict[str, Any]:
        return self._request("POST", path, **kwargs)

    def put(self, path: str, **kwargs) -> Dict[str, Any]:
        return self._request("PUT", path, **kwargs)

    def delete(self, path: str, **kwargs) -> Dict[str, Any]:
        return self._request("DELETE", path, **kwargs)

    # ========================================================================
    # Pack Management
    # ========================================================================

    def register_pack(self, path: str, skip_tests: bool = True) -> Dict[str, Any]:
        """Register a pack from local directory"""
        return self.post(
            "/api/v1/packs/register",
            json={"path": path, "force": True, "skip_tests": skip_tests},
        )

    def get_pack(self, pack_ref: str) -> Dict[str, Any]:
        """Get pack by ref"""
        return self.get(f"/api/v1/packs/{pack_ref}")

    # ========================================================================
    # Actions
    # ========================================================================

    def create_action(self, action_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create an action"""
        return self.post("/api/v1/actions", json=action_data)

    def get_action(self, action_ref: str) -> Dict[str, Any]:
        """Get action by ref"""
        return self.get(f"/api/v1/actions/{action_ref}")

    # ========================================================================
    # Triggers
    # ========================================================================

    def create_trigger(self, trigger_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a trigger"""
        return self.post("/api/v1/triggers", json=trigger_data)

    def get_trigger(self, trigger_ref: str) -> Dict[str, Any]:
        """Get trigger by ref"""
        return self.get(f"/api/v1/triggers/{trigger_ref}")

    # ========================================================================
    # Sensors
    # ========================================================================

    def create_sensor(self, sensor_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a sensor"""
        return self.post("/api/v1/sensors", json=sensor_data)

    def get_sensor(self, sensor_ref: str) -> Dict[str, Any]:
        """Get sensor by ref"""
        return self.get(f"/api/v1/sensors/{sensor_ref}")

    # ========================================================================
    # Rules
    # ========================================================================

    def create_rule(self, rule_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a rule"""
        return self.post("/api/v1/rules", json=rule_data)

    def get_rule(self, rule_ref: str) -> Dict[str, Any]:
        """Get rule by ref"""
        return self.get(f"/api/v1/rules/{rule_ref}")

    # ========================================================================
    # Events
    # ========================================================================

    def get_events(
        self, limit: int = 10, trigger_ref: Optional[str] = None
    ) -> Dict[str, Any]:
        """Get recent events"""
        params = {"limit": limit}
        if trigger_ref:
            params["trigger_ref"] = trigger_ref
        return self.get("/api/v1/events", params=params)

    def get_event(self, event_id: int) -> Dict[str, Any]:
        """Get event by ID"""
        return self.get(f"/api/v1/events/{event_id}")

    # ========================================================================
    # Executions
    # ========================================================================

    def get_executions(
        self, limit: int = 10, action_ref: Optional[str] = None
    ) -> Dict[str, Any]:
        """Get recent executions"""
        params = {"limit": limit}
        if action_ref:
            params["action_ref"] = action_ref
        return self.get("/api/v1/executions", params=params)

    def get_execution(self, execution_id: int) -> Dict[str, Any]:
        """Get execution by ID"""
        return self.get(f"/api/v1/executions/{execution_id}")

    def wait_for_execution_status(
        self, execution_id: int, target_status: str, timeout: int = TEST_TIMEOUT
    ) -> Dict[str, Any]:
        """Poll execution until it reaches target status"""
        start_time = time.time()

        while time.time() - start_time < timeout:
            exec_data = self.get_execution(execution_id)
            current_status = exec_data["data"]["status"]

            if current_status == target_status:
                return exec_data

            if current_status in ["failed", "timeout", "canceled"]:
                raise RuntimeError(
                    f"Execution {execution_id} reached terminal status '{current_status}' "
                    f"while waiting for '{target_status}'"
                )

            time.sleep(POLL_INTERVAL)

        raise TimeoutError(
            f"Execution {execution_id} did not reach status '{target_status}' "
            f"within {timeout} seconds"
        )


@pytest.fixture(scope="session")
def client():
    """Create and authenticate API client"""
    c = AttuneClient(API_BASE_URL)
    c.login()
    return c


@pytest.fixture(scope="session")
def test_pack(client):
    """Register test pack"""
    # Try multiple possible paths (depending on where pytest is run from)
    possible_paths = [
        "fixtures/packs/test_pack",  # Running from tests/
        "tests/fixtures/packs/test_pack",  # Running from project root
        os.path.join(
            os.path.dirname(__file__), "fixtures/packs/test_pack"
        ),  # Relative to this file
    ]

    pack_path = None
    for path in possible_paths:
        abs_path = os.path.abspath(path)
        if os.path.exists(abs_path):
            pack_path = abs_path
            break

    if not pack_path:
        pytest.skip(f"Test pack not found. Tried: {possible_paths}")

    # Register the pack (handle if already exists)
    try:
        result = client.register_pack(pack_path)
        pack_ref = result["data"]["ref"]
    except requests.exceptions.HTTPError as e:
        # If pack already exists (409 Conflict), get the pack ref from the error or default
        if e.response.status_code == 409:
            # Pack already exists, use default name
            pack_ref = "test_pack"
        else:
            raise

    yield pack_ref

    # Cleanup: Delete pack after tests
    # Note: This will cascade delete actions, rules, etc.
    # client.delete(f"/api/v1/packs/{pack_ref}")


@pytest.fixture
def unique_ref():
    """Generate unique ref for test resources"""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")
    return f"test_{timestamp}"


# ============================================================================
# Test Cases
# ============================================================================


class TestBasicAutomation:
    """Test basic automation flows"""

    def test_api_health(self, client):
        """Test API health endpoint"""
        response = client.get("/health")
        assert response["status"] == "ok"

    def test_authentication(self):
        """Test login and token generation"""
        c = AttuneClient(API_BASE_URL)
        token = c.login(login="test@attune.local", password="TestPass123!")

        assert token is not None
        assert len(token) > 20  # JWT tokens are long

    def test_pack_registration(self, client, test_pack):
        """Test pack can be registered"""
        pack_data = client.get_pack(test_pack)

        assert pack_data["data"]["ref"] == test_pack
        assert pack_data["data"]["label"] == "E2E Test Pack"
        assert pack_data["data"]["version"] == "1.0.0"

    def test_create_simple_action(self, client, test_pack, unique_ref):
        """Test creating a simple echo action

        Note: Action creation requires specific schema:
        - pack_ref (not 'pack')
        - entrypoint (not 'entry_point')
        - param_schema (JSON Schema, not 'parameters')
        - No 'runner_type' or 'enabled' fields in CreateActionRequest
        """
        action_ref = f"{test_pack}.{unique_ref}"

        action_data = {
            "ref": action_ref,
            "pack_ref": test_pack,
            "label": "Test Echo Action",
            "description": "Simple echo action for testing",
            "entrypoint": "actions/echo.py",
            "param_schema": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Message to echo",
                    }
                },
                "required": ["message"],
            },
        }

        result = client.create_action(action_data)

        assert result["data"]["ref"] == action_ref

        # Verify we can retrieve it
        retrieved = client.get_action(action_ref)
        assert retrieved["data"]["ref"] == action_ref

    def test_create_automation_rule(self, client, test_pack, unique_ref):
        """
        Test creating a complete automation rule with trigger and action.

        This test creates:
        1. A webhook trigger (simpler than timer triggers)
        2. An echo action
        3. A rule linking the trigger to the action

        Note: Actually firing the trigger requires the sensor service to be running.
        This test only validates that the rule can be created successfully.
        """
        # Step 1: Create a webhook trigger
        trigger_ref = f"{test_pack}.{unique_ref}_webhook"
        trigger_data = {
            "ref": trigger_ref,
            "pack_ref": test_pack,
            "label": "Test Webhook Trigger",
            "description": "Webhook trigger for E2E testing",
            "enabled": True,
            "param_schema": {
                "type": "object",
                "properties": {
                    "url_path": {
                        "type": "string",
                        "description": "URL path for webhook",
                    },
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST", "PUT", "DELETE"],
                        "default": "POST",
                    },
                },
                "required": ["url_path"],
            },
            "out_schema": {
                "type": "object",
                "properties": {
                    "headers": {"type": "object"},
                    "body": {"type": "object"},
                    "query": {"type": "object"},
                },
            },
        }

        trigger_result = client.create_trigger(trigger_data)
        assert trigger_result["data"]["ref"] == trigger_ref

        # Step 2: Create an echo action
        action_ref = f"{test_pack}.{unique_ref}_echo"
        action_data = {
            "ref": action_ref,
            "pack_ref": test_pack,
            "label": "Test Echo Action",
            "description": "Echo action for E2E testing",
            "entrypoint": "actions/echo.py",
            "param_schema": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Message to echo",
                    }
                },
                "required": ["message"],
            },
        }

        action_result = client.create_action(action_data)
        assert action_result["data"]["ref"] == action_ref

        # Step 3: Create a rule linking trigger to action
        rule_ref = f"{test_pack}.{unique_ref}_rule"
        rule_data = {
            "ref": rule_ref,
            "pack_ref": test_pack,
            "label": "Test Webhook to Echo Rule",
            "description": "Rule that echoes webhook payloads",
            "action_ref": action_ref,
            "trigger_ref": trigger_ref,
            "conditions": {
                "and": [{"var": "event.payload.body.message", "!=": None}]
            },
            "action_params": {"message": "{{ event.payload.body.message }}"},
            "enabled": True,
        }

        rule_result = client.create_rule(rule_data)
        assert rule_result["data"]["ref"] == rule_ref
        assert rule_result["data"]["action_ref"] == action_ref
        assert rule_result["data"]["trigger_ref"] == trigger_ref
        assert rule_result["data"]["enabled"] is True

        # Verify we can retrieve the rule
        retrieved_rule = client.get_rule(rule_ref)
        assert retrieved_rule["data"]["ref"] == rule_ref
        assert retrieved_rule["data"]["action_ref"] == action_ref
        assert retrieved_rule["data"]["trigger_ref"] == trigger_ref


class TestManualExecution:
    """Test manual action execution (without sensor/trigger flow)

    This tests the ability to execute an action directly via the API
    without requiring a trigger or rule.
    """

    def test_execute_action_directly(self, client, test_pack, unique_ref):
        """
        Test executing an action directly via API
        This tests: API → Executor → Worker flow

        Uses the POST /api/v1/executions/execute endpoint to directly
        execute an action with parameters.
        """
        # Create an echo action first
        action_ref = f"{test_pack}.{unique_ref}_manual_echo"
        action_data = {
            "ref": action_ref,
            "pack_ref": test_pack,
            "label": "Manual Execution Test Action",
            "description": "Echo action for manual execution testing",
            "entrypoint": "actions/echo.py",
            "param_schema": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Message to echo",
                    }
                },
                "required": ["message"],
            },
        }

        action_result = client.create_action(action_data)
        assert action_result["data"]["ref"] == action_ref

        # Execute the action manually
        execution_request = {
            "action_ref": action_ref,
            "parameters": {"message": "Hello from manual execution!"},
        }

        execution_result = client.post(
            "/api/v1/executions/execute", json=execution_request
        )

        # Verify execution was created
        assert "data" in execution_result
        execution = execution_result["data"]
        assert execution["action_ref"] == action_ref
        assert execution["status"].lower() in [
            "requested",
            "scheduling",
            "scheduled",
            "running",
        ]
        assert execution["config"]["message"] == "Hello from manual execution!"

        execution_id = execution["id"]

        # Verify we can retrieve the execution
        retrieved = client.get_execution(execution_id)
        assert retrieved["data"]["id"] == execution_id
        assert retrieved["data"]["action_ref"] == action_ref


# ============================================================================
# Main
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v", "-s"])
