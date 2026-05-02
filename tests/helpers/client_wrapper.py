"""
Wrapper for Generated API Client

This module provides a backward-compatible wrapper around the auto-generated
OpenAPI client, maintaining the same interface as the original AttuneClient
while using the generated client internally.

This allows tests to gradually migrate to the generated client without
requiring immediate changes to all test code.
"""

import os
from typing import Any, Optional

from generated_client import AuthenticatedClient, Client
from generated_client.api.actions import (
    create_action as gen_create_action,
)
from generated_client.api.actions import (
    delete_action as gen_delete_action,
)
from generated_client.api.actions import (
    get_action as gen_get_action,
)
from generated_client.api.actions import (
    list_actions as gen_list_actions,
)
from generated_client.api.auth import (
    login as gen_login,
)
from generated_client.api.auth import (
    register as gen_register,
)
from generated_client.api.enforcements import list_enforcements as gen_list_enforcements
from generated_client.api.events import get_event as gen_get_event
from generated_client.api.events import list_events as gen_list_events
from generated_client.api.executions import (
    get_execution as gen_get_execution,
)
from generated_client.api.executions import (
    list_executions as gen_list_executions,
)
from generated_client.api.health import health as gen_health
from generated_client.api.inquiries import (
    list_inquiries as gen_list_inquiries,
)
from generated_client.api.inquiries import (
    respond_to_inquiry as gen_respond_inquiry,
)
from generated_client.api.packs import (
    create_pack as gen_create_pack,
)
from generated_client.api.packs import (
    delete_pack as gen_delete_pack,
)
from generated_client.api.packs import (
    get_pack as gen_get_pack,
)
from generated_client.api.packs import (
    list_packs as gen_list_packs,
)
from generated_client.api.packs import (
    register_pack as gen_register_pack,
)
from generated_client.api.rules import (
    create_rule as gen_create_rule,
)
from generated_client.api.rules import (
    delete_rule as gen_delete_rule,
)
from generated_client.api.rules import (
    disable_rule as gen_disable_rule,
)
from generated_client.api.rules import (
    enable_rule as gen_enable_rule,
)
from generated_client.api.rules import (
    get_rule as gen_get_rule,
)
from generated_client.api.rules import (
    list_rules as gen_list_rules,
)
from generated_client.api.secrets import (
    create_key as gen_create_key,
)
from generated_client.api.secrets import (
    delete_key as gen_delete_key,
)
from generated_client.api.secrets import (
    get_key as gen_get_key,
)
from generated_client.api.secrets import (
    list_keys as gen_list_keys,
)
from generated_client.api.secrets import (
    update_key as gen_update_key,
)
from generated_client.api.sensors import (
    create_sensor as gen_create_sensor,
)
from generated_client.api.sensors import (
    delete_sensor as gen_delete_sensor,
)
from generated_client.api.sensors import (
    get_sensor as gen_get_sensor,
)
from generated_client.api.sensors import (
    list_sensors as gen_list_sensors,
)
from generated_client.api.triggers import (
    create_trigger as gen_create_trigger,
)
from generated_client.api.triggers import (
    delete_trigger as gen_delete_trigger,
)
from generated_client.api.triggers import (
    get_trigger as gen_get_trigger,
)
from generated_client.api.triggers import (
    list_triggers as gen_list_triggers,
)
from generated_client.api.webhooks import receive_webhook as gen_receive_webhook
from generated_client.models.create_action_request import CreateActionRequest
from generated_client.models.create_key_request import CreateKeyRequest
from generated_client.models.create_pack_request import CreatePackRequest
from generated_client.models.create_rule_request import CreateRuleRequest
from generated_client.models.create_sensor_request import CreateSensorRequest
from generated_client.models.create_trigger_request import CreateTriggerRequest
from generated_client.models.inquiry_respond_request import InquiryRespondRequest
from generated_client.models.login_request import LoginRequest
from generated_client.models.register_pack_request import RegisterPackRequest
from generated_client.models.register_request import RegisterRequest
from generated_client.models.update_key_request import UpdateKeyRequest
from generated_client.models.owner_type import OwnerType


def to_dict(obj: Any) -> Any:
    """Convert Pydantic model to dict recursively"""
    if obj is None:
        return None
    if hasattr(obj, "to_dict"):
        return obj.to_dict()
    if isinstance(obj, dict):
        return obj
    if isinstance(obj, list):
        return [to_dict(item) for item in obj]
    return obj


def unwrap_list(response: Any) -> list[dict]:
    """Unwrap a paginated list response into a list of dicts.

    The generated client returns PaginatedResponse* objects with 'items' field.
    """
    if response is None:
        return []
    result = to_dict(response)
    if isinstance(result, dict):
        if "items" in result:
            return result["items"]
        if "data" in result:
            return result["data"]
    if isinstance(result, list):
        return result
    return []


def unwrap_item(response: Any) -> Optional[dict]:
    """Unwrap a single-item API response into a dict.

    The generated client returns ApiResponse* objects with 'data' field.
    """
    if response is None:
        return None
    result = to_dict(response)
    if isinstance(result, dict):
        if "data" in result:
            return result["data"]
        return result
    return result


class AttuneClient:
    """
    Backward-compatible wrapper for generated Attune API client

    This class wraps the auto-generated OpenAPI client to maintain
    compatibility with existing test code while using type-safe
    generated API calls internally.
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        timeout: int = 30,
        auto_login: bool = True,
    ):
        """
        Initialize Attune client wrapper

        Args:
            base_url: API base URL (defaults to ATTUNE_API_URL env var)
            timeout: Request timeout in seconds
            auto_login: Whether to auto-login with default credentials
        """
        self.base_url = base_url or os.getenv("ATTUNE_API_URL", "http://localhost:8080")
        self.timeout = timeout
        self.auto_login_flag = auto_login

        # Initialize unauthenticated clients
        # Note: Generated API functions include full paths like "/api/v1/packs"
        # so base_url should just be the host
        self.client = Client(
            base_url=self.base_url,
            timeout=float(timeout),
            verify_ssl=False,
        )

        # Auth client (same as regular client since paths are in generated code)
        self.auth_base_client = Client(
            base_url=self.base_url,
            timeout=float(timeout),
            verify_ssl=False,
        )

        # Will be set after login
        self.auth_client: Optional[AuthenticatedClient] = None
        self.access_token: Optional[str] = None
        self.refresh_token: Optional[str] = None
        self.user_info: Optional[dict] = None

        # Default credentials
        self.default_login = os.getenv("TEST_USER_LOGIN", "test@attune.local")
        self.default_password = os.getenv("TEST_USER_PASSWORD", "TestPass123!")

        # Auto-login if requested
        if self.auto_login_flag:
            self.login()

    def _get_client(self) -> AuthenticatedClient:
        """Get authenticated client (raises if not logged in)"""
        if not self.auth_client:
            raise Exception("Not authenticated. Please login first.")
        return self.auth_client

    def register(
        self,
        login: str,
        password: str,
        display_name: Optional[str] = None,
    ) -> dict:
        """
        Register a new user

        Args:
            login: User login/email
            password: User password
            display_name: User display name

        Returns:
            dict: User information
        """
        request = RegisterRequest(
            login=login,
            password=password,
            display_name=display_name or login,
        )

        response = gen_register.sync(client=self.auth_base_client, body=request)

        return unwrap_item(response)

        raise Exception("Registration failed")

    def login(
        self,
        login: Optional[str] = None,
        password: Optional[str] = None,
    ) -> dict:
        """
        Login and obtain access token

        Args:
            login: User login (defaults to TEST_USER_LOGIN)
            password: User password (defaults to TEST_USER_PASSWORD)

        Returns:
            dict: Login response with tokens
        """
        login_email = login or self.default_login
        login_password = password or self.default_password

        request = LoginRequest(
            login=login_email,
            password=login_password,
        )

        response = gen_login.sync(client=self.auth_base_client, body=request)

        if response:
            result = to_dict(response)
            if isinstance(result, dict) and "data" in result:
                data = result["data"]
                self.access_token = data.get("access_token")
                self.refresh_token = data.get("refresh_token")
                self.user_info = data.get("user")

                # Create authenticated client
                # Note: base_url should just be host since generated API includes full paths
                self.auth_client = AuthenticatedClient(
                    base_url=self.base_url,
                    token=self.access_token,
                    timeout=float(self.timeout),
                    verify_ssl=False,
                )

                return data

        raise Exception("Login failed")

    def logout(self):
        """Logout and clear tokens"""
        self.auth_client = None
        self.access_token = None
        self.refresh_token = None
        self.user_info = None

    def health(self) -> dict:
        """Check API health"""
        response = gen_health.sync(client=self.auth_base_client)
        return to_dict(response) if response else {}

    # ========================================================================
    # Packs
    # ========================================================================

    def list_packs(self, **params) -> list[dict]:
        """List all packs"""
        response = gen_list_packs.sync(client=self._get_client())
        return unwrap_list(response)

    def get_pack(self, pack_id: int) -> dict:
        """Get pack by ID - Note: API uses ref, so need to lookup by ref"""
        # The generated API uses ref, not ID
        # This is a compatibility shim - we need to list and find by ID
        packs = self.list_packs()
        for pack in packs:
            if pack.get("id") == pack_id:
                return self.get_pack_by_ref(pack["ref"])
        raise Exception(f"Pack {pack_id} not found")

    def get_pack_by_ref(self, ref: str) -> Optional[dict]:
        """Get pack by reference"""
        response = gen_get_pack.sync(ref=ref, client=self._get_client())
        return unwrap_item(response)

    def create_pack(
        self,
        ref: str,
        label: str,
        description: Optional[str] = None,
        version: str = "1.0.0",
        author: Optional[str] = None,
        **kwargs,
    ) -> dict:
        """Create a new pack"""
        request = CreatePackRequest(
            ref=ref,
            label=label,
            description=description,
            version=version,
            author=author,
        )

        response = gen_create_pack.sync(client=self._get_client(), body=request)

        return unwrap_item(response)

        raise Exception("Failed to create pack")

    def register_pack(
        self, path: str, skip_tests: bool = True, force: bool = False
    ) -> dict:
        """Register a pack from filesystem path (API-server-side path)

        Args:
            path: Path to pack directory (must be accessible by the API server)
            skip_tests: Skip running pack tests during registration (default: True)
            force: Force registration even if tests fail (default: False)
        """
        # Use direct HTTP request to avoid generated client schema mismatch
        payload = {"path": path, "skip_tests": skip_tests, "force": force}
        response = self.post("/api/v1/packs/register", json=payload)

        if response.status_code in (200, 201):
            data = response.json()
            if isinstance(data, dict) and "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to register pack: {response.status_code} {response.text}"
        )

    def upload_pack(
        self, pack_dir: str, force: bool = False, skip_tests: bool = True
    ) -> dict:
        """Upload a pack directory as a tarball to the API

        This works across container boundaries (test container → API container)
        by creating a .tar.gz archive and POSTing it to /api/v1/packs/upload.

        Args:
            pack_dir: Local path to pack directory
            force: Overwrite existing pack (default: False)
            skip_tests: Skip pack tests (default: True)
        """
        import io
        import tarfile

        # Create in-memory tarball
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w:gz") as tar:
            tar.add(pack_dir, arcname=".")
        buf.seek(0)

        # Upload via multipart form
        client = self._get_client()
        files = {"pack": ("pack.tar.gz", buf, "application/gzip")}
        data = {}
        if force:
            data["force"] = "true"
        if skip_tests:
            data["skip_tests"] = "true"

        response = client.get_httpx_client().post(
            "/api/v1/packs/upload", files=files, data=data
        )

        if response.status_code in (200, 201):
            resp_data = response.json()
            # Upload response wraps pack data in {"data": {"pack": {...}, ...}}
            if isinstance(resp_data, dict) and "data" in resp_data:
                resp_data = resp_data["data"]
            # Extract just the pack object if wrapped in {"pack": {...}}
            if isinstance(resp_data, dict) and "pack" in resp_data:
                return resp_data["pack"]
            return resp_data
        raise Exception(
            f"Failed to upload pack: {response.status_code} {response.text}"
        )

    def reload_pack(self, pack_id: int) -> dict:
        """Reload a pack"""
        # Not implemented in wrapper yet
        raise NotImplementedError("reload_pack not yet implemented")

    def delete_pack(self, pack_id: int):
        """Delete a pack"""
        # Need to get pack ref first
        pack = self.get_pack(pack_id)
        if pack:
            gen_delete_pack.sync(ref=pack["ref"], client=self._get_client())

    # ========================================================================
    # Actions
    # ========================================================================

    def list_actions(self, **params) -> list[dict]:
        """List all actions"""
        response = gen_list_actions.sync(
            client=self._get_client(),
        )
        items = unwrap_list(response)
        # Client-side pack filter (API no longer supports server-side filtering)
        pack = params.get("pack")
        if pack:
            items = [a for a in items if a.get("pack_ref") == pack or a.get("ref", "").startswith(f"{pack}.")]
        return items

    def get_action(self, action_id: int) -> dict:
        """Get action by ID - needs ref lookup"""
        actions = self.list_actions()
        for action in actions:
            if action.get("id") == action_id:
                return self.get_action_by_ref(action["ref"])
        raise Exception(f"Action {action_id} not found")

    def get_action_by_ref(self, ref: str) -> Optional[dict]:
        """Get action by reference"""
        response = gen_get_action.sync(ref=ref, client=self._get_client())
        return unwrap_item(response)

    def create_action(
        self,
        ref: Optional[str] = None,
        label: Optional[str] = None,
        pack_ref: Optional[str] = None,
        entrypoint: Optional[str] = None,
        description: Optional[str] = None,
        param_schema: Optional[dict] = None,
        out_schema: Optional[dict] = None,
        runtime_ref: Optional[str] = None,
        # Legacy arguments for backward compatibility
        name: Optional[str] = None,
        runner_type: Optional[str] = None,
        data: Optional[dict] = None,
        **kwargs,
    ) -> dict:
        """Create a new action

        Supports both new-style (ref, label, pack_ref, entrypoint)
        and legacy-style (pack_ref, name, runner_type, entrypoint) arguments,
        plus dict-style (pack_ref, data={...}) for tier 2/3 tests.
        """
        # Handle dict-style argument (tier 2/3 tests pass data={...})
        if data is not None:
            name = name or data.get("name")
            description = description or data.get("description")
            runner_type = runner_type or data.get("runner_type")
            entrypoint = entrypoint or data.get("entry_point") or data.get("entrypoint")
            param_schema = param_schema or data.get("parameters") or data.get("param_schema")
            out_schema = out_schema or data.get("out_schema")

        # Handle legacy-style arguments
        if pack_ref and name and not ref:
            ref = f"{pack_ref}.{name}"
            label = label or name.replace("_", " ").title()

            # Map legacy runner_type to runtime_ref
            if runner_type and not runtime_ref:
                runtime_map = {
                    "python3": "core.python",
                    "python": "core.python",
                    "shell": "core.shell",
                    "node": "core.nodejs",
                    "nodejs": "core.nodejs",
                }
                runtime_ref = runtime_map.get(runner_type, f"core.{runner_type}")

        # Default entrypoint if not provided
        if not entrypoint:
            entrypoint = "action.sh"

        # Validate required fields
        if not ref or not label or not pack_ref:
            raise ValueError(
                "Missing required arguments: ref, label, pack_ref "
                "(or pack_ref, name)"
            )

        # Use plain POST request instead of generated client to handle API schema changes
        payload = {
            "ref": ref,
            "label": label,
            "pack_ref": pack_ref,
            "entrypoint": entrypoint,
            "description": description or f"Action: {label}",
        }
        if param_schema:
            payload["param_schema"] = param_schema
        if out_schema:
            payload["out_schema"] = out_schema
        if runtime_ref:
            payload["runtime_ref"] = runtime_ref

        response = self._request("POST", "/api/v1/actions", json=payload)
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to create action: {response.status_code} {response.text}"
        )

    def delete_action(self, action_id: int):
        """Delete an action"""
        action = self.get_action(action_id)
        if action:
            gen_delete_action.sync(ref=action["ref"], client=self._get_client())

    # ========================================================================
    # Triggers
    # ========================================================================

    def list_triggers(self, **params) -> list[dict]:
        """List all triggers"""
        response = gen_list_triggers.sync(client=self._get_client(), page_size=1000)
        return unwrap_list(response)

    def get_trigger(self, trigger_id: int) -> dict:
        """Get trigger by ID"""
        # Use paginated listing with max page_size to handle many triggers
        response = self.get("/api/v1/triggers", params={"page_size": 100})
        if response.status_code == 200:
            data = response.json()
            triggers = data.get("items", data.get("data", []))
            if isinstance(triggers, list):
                for trigger in triggers:
                    if trigger.get("id") == trigger_id:
                        return self.get_trigger_by_ref(trigger["ref"])
            # Check additional pages
            pagination = data.get("pagination", {})
            page = 2
            while pagination.get("has_next"):
                response = self.get("/api/v1/triggers", params={"page_size": 100, "page": page})
                if response.status_code != 200:
                    break
                data = response.json()
                triggers = data.get("items", data.get("data", []))
                if isinstance(triggers, list):
                    for trigger in triggers:
                        if trigger.get("id") == trigger_id:
                            return self.get_trigger_by_ref(trigger["ref"])
                pagination = data.get("pagination", {})
                page += 1
        raise Exception(f"Trigger {trigger_id} not found")

    def get_trigger_by_ref(self, ref: str) -> Optional[dict]:
        """Get trigger by reference"""
        response = gen_get_trigger.sync(ref=ref, client=self._get_client())
        return unwrap_item(response)

    def create_trigger(
        self,
        ref: Optional[str] = None,
        label: Optional[str] = None,
        pack_ref: Optional[str] = None,
        description: Optional[str] = None,
        param_schema: Optional[dict] = None,
        out_schema: Optional[dict] = None,
        # Legacy arguments for backward compatibility
        name: Optional[str] = None,
        trigger_type: Optional[str] = None,
        parameters: Optional[dict] = None,
        **kwargs,
    ) -> dict:
        """Create a new trigger

        Supports both new-style (ref, label, pack_ref) and legacy-style (pack_ref, name) arguments.
        """
        # Handle legacy-style arguments
        if pack_ref and name:
            # Build ref from pack_ref and name
            ref = f"{pack_ref}.{name}"
            label = name.replace("_", " ").title()

            if parameters:
                param_schema = parameters

        # Validate required fields
        if not ref or not label or not pack_ref:
            raise ValueError(
                "Missing required arguments: ref, label, and pack_ref (or pack_ref and name)"
            )

        # Use plain POST request instead of generated client to handle API schema changes
        payload = {
            "ref": ref,
            "label": label,
            "pack_ref": pack_ref,
        }

        if description:
            payload["description"] = description
        if param_schema:
            payload["param_schema"] = param_schema
        if out_schema:
            payload["out_schema"] = out_schema

        response = self._request("POST", "/api/v1/triggers", json=payload)
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to create trigger: {response.status_code} {response.text}"
        )

    def delete_trigger(self, trigger_id: int):
        """Delete a trigger"""
        trigger = self.get_trigger(trigger_id)
        if trigger:
            gen_delete_trigger.sync(ref=trigger["ref"], client=self._get_client())

    def enable_webhook(
        self,
        trigger_ref: Optional[str] = None,
        trigger_id: Optional[int] = None,
    ) -> dict:
        """Enable webhooks for a trigger

        Supports both trigger_ref and trigger_id arguments for backward compatibility.
        """
        # Handle legacy trigger_id argument
        if trigger_id and not trigger_ref:
            trigger = self.get_trigger(trigger_id)
            if trigger:
                trigger_ref = trigger["ref"]
            else:
                raise Exception(f"Trigger {trigger_id} not found")

        if not trigger_ref:
            raise ValueError("Either trigger_ref or trigger_id must be provided")

        response = self._request(
            "POST", f"/api/v1/triggers/{trigger_ref}/webhooks/enable"
        )
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to enable webhook: {response.status_code} {response.text}"
        )

    def disable_webhook(
        self,
        trigger_ref: Optional[str] = None,
        trigger_id: Optional[int] = None,
    ) -> dict:
        """Disable webhooks for a trigger

        Supports both trigger_ref and trigger_id arguments for backward compatibility.
        """
        # Handle legacy trigger_id argument
        if trigger_id and not trigger_ref:
            trigger = self.get_trigger(trigger_id)
            if trigger:
                trigger_ref = trigger["ref"]
            else:
                raise Exception(f"Trigger {trigger_id} not found")

        if not trigger_ref:
            raise ValueError("Either trigger_ref or trigger_id must be provided")

        response = self._request(
            "POST", f"/api/v1/triggers/{trigger_ref}/webhooks/disable"
        )
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to disable webhook: {response.status_code} {response.text}"
        )

    def fire_webhook(
        self,
        trigger_ref: Optional[str] = None,
        payload: Optional[dict] = None,
        trigger_id: Optional[int] = None,
        auto_enable: bool = True,
    ) -> dict:
        """Fire a webhook trigger

        Supports both trigger_ref and trigger_id arguments for backward compatibility.

        Args:
            trigger_ref: Trigger reference
            payload: Webhook payload
            trigger_id: Trigger ID (legacy)
            auto_enable: Automatically enable webhooks if not enabled (default: True)
        """
        # Handle legacy trigger_id argument
        if trigger_id and not trigger_ref:
            trigger = self.get_trigger(trigger_id)
            if trigger:
                trigger_ref = trigger["ref"]
            else:
                raise Exception(f"Trigger {trigger_id} not found")

        if not trigger_ref:
            raise ValueError("Either trigger_ref or trigger_id must be provided")

        # Get the trigger to check if webhooks are enabled
        trigger = self.get_trigger_by_ref(trigger_ref)
        if not trigger:
            raise Exception(f"Trigger {trigger_ref} not found")

        # Enable webhooks if not enabled and auto_enable is True
        if not trigger.get("webhook_enabled") and auto_enable:
            trigger = self.enable_webhook(trigger_ref=trigger_ref)

        # Check if we have a webhook_key now
        if not trigger.get("webhook_key"):
            raise Exception(f"Trigger {trigger_ref} does not have a webhook_key")

        # Use plain POST request instead of generated client to handle API response structure
        webhook_key = trigger["webhook_key"]
        response = self._request(
            "POST",
            f"/api/v1/webhooks/{webhook_key}",
            json={"payload": payload},
        )
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to fire webhook: {response.status_code} {response.text}"
        )

    # ========================================================================
    # Sensors
    # ========================================================================

    def list_sensors(self, **params) -> list[dict]:
        """List all sensors"""
        response = gen_list_sensors.sync(client=self._get_client())
        return unwrap_list(response)

    def get_sensor(self, sensor_id: int) -> dict:
        """Get sensor by ID"""
        sensors = self.list_sensors()
        for sensor in sensors:
            if sensor.get("id") == sensor_id:
                response = gen_get_sensor.sync(
                    ref=sensor.get("ref", str(sensor_id)), client=self._get_client()
                )
                if response:
                    result = to_dict(response)
                    if isinstance(result, dict) and "data" in result:
                        return result["data"]
        raise Exception(f"Sensor {sensor_id} not found")

    def create_sensor(
        self,
        trigger_id: int,
        enabled: bool = True,
        parameters: Optional[dict] = None,
        **kwargs,
    ) -> dict:
        """Create a new sensor"""
        # Get trigger to obtain trigger_ref
        trigger = self.get_trigger(trigger_id)
        trigger_ref = trigger.get("ref")

        # Extract required fields from kwargs or use defaults
        ref = kwargs.get("ref", f"sensor_{trigger_id}")
        pack_ref = kwargs.get("pack_ref", "core")
        runtime_ref = kwargs.get("runtime_ref", "python3")
        label = kwargs.get("label", f"Sensor for {trigger_ref}")
        entrypoint = kwargs.get("entrypoint", "internal://sensor")
        description = kwargs.get("description", f"Sensor for trigger {trigger_ref}")
        param_schema = kwargs.get("param_schema")
        config = kwargs.get("config")

        request = CreateSensorRequest(
            ref=ref,
            trigger_ref=trigger_ref,
            pack_ref=pack_ref,
            runtime_ref=runtime_ref,
            label=label,
            entrypoint=entrypoint,
            description=description,
            param_schema=param_schema,
            config=config,
            enabled=enabled,
        )

        response = gen_create_sensor.sync(client=self._get_client(), body=request)

        return unwrap_item(response)

        # Get more detailed error information
        try:
            error_response = self._request(
                "POST", "/api/v1/sensors", json=request.to_dict()
            )
            error_msg = f"Failed to create sensor: {error_response.status_code} - {error_response.text}"
        except Exception as e:
            error_msg = f"Failed to create sensor: {str(e)}"

        raise Exception(error_msg)

    def delete_sensor(self, sensor_id: int):
        """Delete a sensor"""
        sensor = self.get_sensor(sensor_id)
        if sensor:
            gen_delete_sensor.sync(
                ref=sensor.get("ref", str(sensor_id)), client=self._get_client()
            )

    # ========================================================================
    # Rules
    # ========================================================================

    def list_rules(self, **params) -> list[dict]:
        """List all rules"""
        response = gen_list_rules.sync(client=self._get_client())
        return unwrap_list(response)

    def get_rule(self, rule_id: int) -> dict:
        """Get rule by ID"""
        rules = self.list_rules()
        for rule in rules:
            if rule.get("id") == rule_id:
                response = gen_get_rule.sync(
                    ref=rule.get("ref", str(rule_id)), client=self._get_client()
                )
                if response:
                    result = to_dict(response)
                    if isinstance(result, dict) and "data" in result:
                        return result["data"]
        raise Exception(f"Rule {rule_id} not found")

    def create_rule(
        self,
        ref: Optional[str] = None,
        label: Optional[str] = None,
        pack_ref: Optional[str] = None,
        trigger_ref: Optional[str] = None,
        action_ref: Optional[str] = None,
        enabled: bool = True,
        description: Optional[str] = None,
        criteria: Optional[str] = None,
        action_parameters: Optional[dict] = None,
        # Legacy arguments for backward compatibility
        name: Optional[str] = None,
        trigger_id: Optional[int] = None,
        data: Optional[dict] = None,
        **kwargs,
    ) -> dict:
        """Create a new rule

        Supports new-style keyword args, legacy-style, and data={...} dict.
        """
        # Handle dict-style argument
        if data is not None:
            name = name or data.get("name")
            description = description or data.get("description")
            trigger_ref = trigger_ref or data.get("trigger_ref")
            action_ref = action_ref or data.get("action_ref")
            enabled = data.get("enabled", enabled)
            criteria = criteria or data.get("criteria")
            action_parameters = action_parameters or data.get(
                "action_parameters"
            ) or data.get("action_params")

        # Handle legacy-style arguments
        if pack_ref and name and not ref:
            ref = f"{pack_ref}.{name}"
            label = label or name.replace("_", " ").title()

            # If trigger_id is provided, get the trigger to find trigger_ref
            if trigger_id and not trigger_ref:
                trigger = self.get_trigger(trigger_id)
                if trigger:
                    trigger_ref = trigger["ref"]
                else:
                    raise Exception(f"Trigger {trigger_id} not found")

        # Validate required fields
        if not ref or not label or not pack_ref or not trigger_ref or not action_ref:
            raise ValueError(
                "Missing required arguments: ref, label, pack_ref, trigger_ref, and action_ref "
                "(or pack_ref, name, trigger_id, and action_ref)"
            )

        # Use plain POST request instead of generated client to handle API schema changes
        payload = {
            "ref": ref,
            "label": label,
            "pack_ref": pack_ref,
            "trigger_ref": trigger_ref,
            "action_ref": action_ref,
            "enabled": enabled,
            "description": description or f"Rule: {label}",
        }

        if criteria:
            payload["criteria"] = criteria
        if action_parameters:
            payload["action_params"] = action_parameters

        # Include trigger_parameters if provided (required for triggers with param_schema)
        trigger_parameters = kwargs.get("trigger_parameters")
        if trigger_parameters:
            payload["trigger_params"] = trigger_parameters

        response = self._request("POST", "/api/v1/rules", json=payload)
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to create rule: {response.status_code} {response.text}"
        )

    def update_rule(self, rule_id: int, **kwargs) -> dict:
        """Update a rule"""
        raise NotImplementedError("update_rule not yet implemented")

    def enable_rule(self, rule_id: int) -> dict:
        """Enable a rule"""
        rule = self.get_rule(rule_id)
        response = gen_enable_rule.sync(
            ref=rule.get("ref", str(rule_id)), client=self._get_client()
        )
        return unwrap_item(response) or {}

    def disable_rule(self, rule_id: int) -> dict:
        """Disable a rule"""
        rule = self.get_rule(rule_id)
        response = gen_disable_rule.sync(
            ref=rule.get("ref", str(rule_id)), client=self._get_client()
        )
        return unwrap_item(response) or {}

    def delete_rule(self, rule_id: int):
        """Delete a rule"""
        rule = self.get_rule(rule_id)
        if rule:
            gen_delete_rule.sync(
                ref=rule.get("ref", str(rule_id)), client=self._get_client()
            )

    # ========================================================================
    # Events
    # ========================================================================

    def list_events(self, enrich: bool = True, **params) -> list[dict]:
        """List all events (optionally enriched with full payload from individual fetches)

        Args:
            enrich: If True, fetch full payload for each event (slow for large lists).
                    If False, return list response as-is (has rule, created, trigger fields).
        """
        # Map trigger_id to trigger for backward compatibility
        trigger = params.get("trigger_id") or params.get("trigger")
        response = gen_list_events.sync(
            client=self._get_client(),
            trigger=trigger,
            trigger_ref=params.get("trigger_ref"),
            source=params.get("source"),
            page=params.get("page"),
            per_page=params.get("limit"),
        )
        items = unwrap_list(response)
        if not enrich:
            return items
        # Enrich events with full payload (list endpoint only returns summaries)
        enriched = []
        for event in items:
            if event.get("has_payload") and "payload" not in event:
                try:
                    full = self.get_event(event["id"])
                    enriched.append(full)
                except Exception:
                    enriched.append(event)
            else:
                enriched.append(event)
        return enriched

    def get_event(self, event_id: int) -> dict:
        """Get event by ID (full response with payload)"""
        response = gen_get_event.sync(id=event_id, client=self._get_client())
        result = unwrap_item(response)
        if result:
            return result
        raise Exception(f"Event {event_id} not found")

    # ========================================================================
    # Enforcements
    # ========================================================================

    def list_enforcements(self, **params) -> list[dict]:
        """List all enforcements"""
        response = gen_list_enforcements.sync(
            client=self._get_client(),
            rule=params.get("rule_id") or params.get("rule"),
            rule_ref=params.get("rule_ref"),
            status=params.get("status"),
            page=params.get("page"),
            per_page=params.get("limit") or params.get("per_page"),
        )
        return unwrap_list(response)

    def get_enforcement(self, enforcement_id: int) -> dict:
        """Get enforcement by ID"""
        enforcements = self.list_enforcements(limit=1000)
        for enforcement in enforcements:
            if enforcement.get("id") == enforcement_id:
                return enforcement
        raise Exception(f"Enforcement {enforcement_id} not found")

    # ========================================================================
    # Executions
    # ========================================================================

    def list_executions(self, **params) -> list[dict]:
        """List all executions"""
        query_params = {}
        if params.get("action_ref"):
            query_params["action_ref"] = params["action_ref"]
        if params.get("status"):
            query_params["status"] = params["status"]
        if params.get("page"):
            query_params["page"] = params["page"]
        per_page = params.get("limit") or params.get("per_page")
        if per_page:
            query_params["per_page"] = per_page
        response = self._request("GET", "/api/v1/executions", params=query_params)
        if response.status_code == 200:
            data = response.json()
            if "items" in data:
                return data["items"]
            if "data" in data:
                return data["data"]
            if isinstance(data, list):
                return data
        return []

    def get_execution(self, execution_id: int) -> dict:
        """Get execution by ID"""
        response = self._request("GET", f"/api/v1/executions/{execution_id}")
        if response.status_code == 200:
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(f"Execution {execution_id} not found")

    def create_execution(
        self,
        action_ref: str,
        parameters: Optional[dict] = None,
        env_vars: Optional[dict] = None,
    ) -> dict:
        """Create and queue an execution"""
        payload: dict = {"action_ref": action_ref}
        if parameters:
            payload["parameters"] = parameters
        if env_vars:
            payload["env_vars"] = env_vars
        response = self._request("POST", "/api/v1/executions/execute", json=payload)
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to create execution: {response.status_code} {response.text}"
        )

    def cancel_execution(self, execution_id: int) -> dict:
        """Cancel an execution"""
        response = self._request(
            "POST", f"/api/v1/executions/{execution_id}/cancel", json={}
        )
        if response.status_code in (200, 201):
            data = response.json()
            if "data" in data:
                return data["data"]
            return data
        raise Exception(
            f"Failed to cancel execution: {response.status_code} {response.text}"
        )

    # ========================================================================
    # Webhooks
    # ========================================================================

    def post_webhook(self, webhook_url: str, payload: Optional[dict] = None) -> dict:
        """Post data to a webhook URL.

        Args:
            webhook_url: Full webhook URL or just the key path
            payload: JSON payload to send
        """
        # webhook_url may be full URL or just path like /api/v1/webhooks/{key}
        if webhook_url.startswith("http"):
            # Extract path from full URL
            from urllib.parse import urlparse

            parsed = urlparse(webhook_url)
            path = parsed.path
        else:
            path = webhook_url

        response = self._request("POST", path, json={"payload": payload or {}})
        if response.status_code in (200, 201, 202):
            return response.json() if response.text else {}
        raise Exception(
            f"Webhook POST failed: {response.status_code} {response.text}"
        )

    # ========================================================================
    # Inquiries
    # ========================================================================

    def create_inquiry(
        self,
        execution_id: Optional[int] = None,
        data: Optional[dict] = None,
        **kwargs,
    ) -> dict:
        """Create an inquiry.

        Supports both keyword args and data={...} dict style.
        """
        if data is not None:
            execution_id = execution_id or data.get("execution_id")
            prompt = data.get("prompt", data.get("message", "Please respond"))
            response_schema = data.get("schema") or data.get("response_schema")
            assigned_to = data.get("assigned_to")
            timeout_at = data.get("timeout_at") or data.get("ttl")
        else:
            prompt = kwargs.get("prompt", "Please respond")
            response_schema = kwargs.get("schema") or kwargs.get("response_schema")
            assigned_to = kwargs.get("assigned_to")
            timeout_at = kwargs.get("timeout_at")

        payload: dict = {
            "execution": execution_id or 0,
            "prompt": prompt,
        }
        if response_schema:
            payload["response_schema"] = response_schema
        if assigned_to:
            payload["assigned_to"] = assigned_to
        if timeout_at:
            # Convert TTL (integer seconds) to ISO timestamp if needed
            if isinstance(timeout_at, (int, float)):
                from datetime import datetime, timezone, timedelta

                timeout_at = (
                    datetime.now(timezone.utc) + timedelta(seconds=timeout_at)
                ).isoformat()
            payload["timeout_at"] = timeout_at

        response = self._request("POST", "/api/v1/inquiries", json=payload)
        if response.status_code in (200, 201):
            resp_data = response.json()
            if "data" in resp_data:
                return resp_data["data"]
            return resp_data
        raise Exception(
            f"Failed to create inquiry: {response.status_code} {response.text}"
        )

    def list_inquiries(self, **params) -> list[dict]:
        """List all inquiries"""
        response = gen_list_inquiries.sync(
            client=self._get_client(),
            status=params.get("status"),
            limit=params.get("limit"),
            offset=params.get("offset"),
        )
        return unwrap_list(response)

    def get_inquiry(self, inquiry_id: int) -> dict:
        """Get inquiry by ID"""
        inquiries = self.list_inquiries(limit=1000)
        for inquiry in inquiries:
            if inquiry.get("id") == inquiry_id:
                return inquiry
        raise Exception(f"Inquiry {inquiry_id} not found")

    def respond_to_inquiry(
        self, inquiry_id: int, response_data: dict = None, response: dict = None
    ) -> dict:
        """Respond to an inquiry"""
        data = response_data or response or {}
        resp = self._request(
            "POST", f"/api/v1/inquiries/{inquiry_id}/respond", json={"response": data}
        )
        if resp.status_code in (200, 201):
            resp_data = resp.json()
            if "data" in resp_data:
                return resp_data["data"]
            return resp_data
        raise Exception(
            f"Failed to respond to inquiry: {resp.status_code} {resp.text}"
        )

    # ========================================================================
    # Secrets (Datastore/Keys)
    # ========================================================================

    def list_secrets(self, **params) -> list[dict]:
        """List all secrets/keys"""
        response = gen_list_keys.sync(client=self._get_client())
        return unwrap_list(response)

    def get_secret(self, key_name: str, **params) -> Optional[dict]:
        """Get secret by key name"""
        response = gen_get_key.sync(ref=key_name, client=self._get_client())
        return unwrap_item(response)

    def datastore_get(self, key: str, **params) -> Optional[str]:
        """Get value from datastore"""
        secret = self.get_secret(key)
        return secret.get("value") if secret else None

    def datastore_set(self, key: str, value: str, **params) -> dict:
        """Set value in datastore"""
        # Check if key exists
        existing = self.get_secret(key)
        if existing:
            # Update existing
            request = UpdateKeyRequest(value=value)
            response = gen_update_key.sync(
                ref=key, client=self._get_client(), body=request
            )
        else:
            # Create new
            encrypted = params.get("encrypted", False)
            request = CreateKeyRequest(
                ref=key,
                name=key,
                value=value,
                owner_type=OwnerType.SYSTEM,
                encrypted=encrypted,
            )
            response = gen_create_key.sync(client=self._get_client(), body=request)

        return unwrap_item(response) or {}

    def datastore_delete(self, key: str, **params):
        """Delete value from datastore"""
        gen_delete_key.sync(ref=key, client=self._get_client())

    def create_secret(self, key: str, value: str, **params) -> dict:
        """Create a new secret"""
        encrypted = params.get("encrypted", True)
        request = CreateKeyRequest(
            ref=key,
            name=key,
            value=value,
            owner_type=OwnerType.SYSTEM,
            encrypted=encrypted,
        )
        response = gen_create_key.sync(client=self._get_client(), body=request)
        return unwrap_item(response) or {}

    def update_secret(self, key_id: int, **params) -> dict:
        """Update a secret"""
        # Need to get key ref first
        secrets = self.list_secrets()
        for secret in secrets:
            if secret.get("id") == key_id:
                request = UpdateKeyRequest(value=params.get("value"))
                response = gen_update_key.sync(
                    ref=secret["key"], client=self._get_client(), body=request
                )
                if response:
                    result = to_dict(response)
                    if isinstance(result, dict) and "data" in result:
                        return result["data"]
        return {}

    def delete_secret(self, key_id: int):
        """Delete a secret"""
        secrets = self.list_secrets()
        for secret in secrets:
            if secret.get("id") == key_id:
                gen_delete_key.sync(ref=secret["key"], client=self._get_client())
                return

    # Aliases for tier 2 test compatibility
    def get_datastore_item(self, key: str, **params) -> Optional[dict]:
        """Get datastore item (alias for get_secret)"""
        return self.get_secret(key)

    def set_datastore_item(self, key: str, value: str, **params) -> dict:
        """Set datastore item (alias for datastore_set)"""
        return self.datastore_set(key, value, **params)

    # ========================================================================
    # Compatibility helpers
    # ========================================================================

    def _request(self, method: str, path: str, **kwargs):
        """Raw request method for backward compatibility"""
        # This is for any edge cases that need raw access
        client = self._get_client()
        url = f"{path}"
        response = client.get_httpx_client().request(method, url, **kwargs)
        return response

    def get(self, path: str, **kwargs):
        """GET request"""
        return self._request("GET", path, **kwargs)

    def post(self, path: str, **kwargs):
        """POST request"""
        return self._request("POST", path, **kwargs)

    def put(self, path: str, **kwargs):
        """PUT request"""
        return self._request("PUT", path, **kwargs)

    def patch(self, path: str, **kwargs):
        """PATCH request"""
        return self._request("PATCH", path, **kwargs)

    def delete(self, path: str, **kwargs):
        """DELETE request"""
        return self._request("DELETE", path, **kwargs)
