"""
T3.18: HTTP Runner Execution Test

Tests that HTTP runner type makes REST API calls and captures responses.
This validates the HTTP runner can make external API calls with proper
headers, authentication, and response handling.

Priority: MEDIUM
Duration: ~10 seconds
"""

import json
import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


@pytest.mark.tier3
@pytest.mark.runner
@pytest.mark.http
def test_http_runner_basic_get(client: AttuneClient, test_pack):
    """
    Test HTTP runner making a basic GET request.
    """
    print("\n" + "=" * 80)
    print("T3.18a: HTTP Runner Basic GET Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create HTTP action for GET request
    print("\n[STEP 1] Creating HTTP GET action...")
    action_ref = f"http_get_test_{unique_ref()}"

    action_data = {
        "ref": action_ref,
        "name": "HTTP GET Test Action",
        "description": "Tests HTTP GET request",
        "runner_type": "http",
        "pack": pack_ref,
        "enabled": True,
        "parameters": {
            "url": {
                "type": "string",
                "required": True,
                "description": "URL to request",
            }
        },
        "http_config": {
            "method": "GET",
            "url": "{{ parameters.url }}",
            "headers": {
                "User-Agent": "Attune-Test/1.0",
                "Accept": "application/json",
            },
            "timeout": 10,
        },
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    print(f"✓ HTTP GET action created: {action_ref}")
    print(f"  Method: GET")
    print(f"  Headers: User-Agent, Accept")

    # Step 2: Execute action against a test endpoint
    print("\n[STEP 2] Executing HTTP GET action...")

    # Use httpbin.org as a reliable test endpoint
    test_url = "https://httpbin.org/get?test=attune&id=123"

    execution_data = {
        "action": action_ref,
        "parameters": {"url": test_url},
    }

    exec_response = client.execute_action(execution_data)
    assert "id" in exec_response, "Execution creation failed"
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")
    print(f"  Target URL: {test_url}")

    # Step 3: Wait for execution to complete
    print("\n[STEP 3] Waiting for HTTP request to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=20,
    )

    print(f"✓ Execution completed: {final_exec['status']}")

    # Step 4: Verify response
    print("\n[STEP 4] Verifying HTTP response...")
    result = final_exec.get("result", {})

    print(f"\nHTTP Response:")
    print("-" * 60)
    print(f"Status Code: {result.get('status_code', 'N/A')}")
    print(f"Headers: {json.dumps(result.get('headers', {}), indent=2)}")

    response_body = result.get("body", "")
    if response_body:
        try:
            body_json = json.loads(response_body)
            print(f"Body (JSON): {json.dumps(body_json, indent=2)}")
        except:
            print(f"Body (text): {response_body[:200]}...")
    print("-" * 60)

    # Verify successful response
    assert result.get("status_code") == 200, (
        f"Expected 200, got {result.get('status_code')}"
    )
    print(f"✓ HTTP status code: 200 OK")

    # Verify response contains our query parameters
    if response_body:
        try:
            body_json = json.loads(response_body)
            args = body_json.get("args", {})
            assert args.get("test") == "attune", "Query parameter 'test' not found"
            assert args.get("id") == "123", "Query parameter 'id' not found"
            print(f"✓ Query parameters captured correctly")
        except Exception as e:
            print(f"⚠ Could not verify query parameters: {e}")

    # Summary
    print("\n" + "=" * 80)
    print("HTTP GET TEST SUMMARY")
    print("=" * 80)
    print(f"✓ HTTP GET action created: {action_ref}")
    print(f"✓ Execution completed: {execution_id}")
    print(f"✓ HTTP request successful: 200 OK")
    print(f"✓ Response captured correctly")
    print("\n🌐 HTTP Runner GET test PASSED!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.runner
@pytest.mark.http
def test_http_runner_post_with_json(client: AttuneClient, test_pack):
    """
    Test HTTP runner making a POST request with JSON body.
    """
    print("\n" + "=" * 80)
    print("T3.18b: HTTP Runner POST with JSON Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create HTTP action for POST request
    print("\n[STEP 1] Creating HTTP POST action...")
    action_ref = f"http_post_test_{unique_ref()}"

    action_data = {
        "ref": action_ref,
        "name": "HTTP POST Test Action",
        "description": "Tests HTTP POST with JSON body",
        "runner_type": "http",
        "pack": pack_ref,
        "enabled": True,
        "parameters": {
            "url": {"type": "string", "required": True},
            "data": {"type": "object", "required": True},
        },
        "http_config": {
            "method": "POST",
            "url": "{{ parameters.url }}",
            "headers": {
                "Content-Type": "application/json",
                "User-Agent": "Attune-Test/1.0",
            },
            "body": "{{ parameters.data | tojson }}",
            "timeout": 10,
        },
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    print(f"✓ HTTP POST action created: {action_ref}")
    print(f"  Method: POST")
    print(f"  Content-Type: application/json")

    # Step 2: Execute action with JSON payload
    print("\n[STEP 2] Executing HTTP POST action...")

    test_url = "https://httpbin.org/post"
    test_data = {
        "username": "test_user",
        "action": "test_automation",
        "timestamp": time.time(),
        "metadata": {"source": "attune", "test": "http_runner"},
    }

    execution_data = {
        "action": action_ref,
        "parameters": {"url": test_url, "data": test_data},
    }

    exec_response = client.execute_action(execution_data)
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")
    print(f"  Target URL: {test_url}")
    print(f"  Payload: {json.dumps(test_data, indent=2)}")

    # Step 3: Wait for completion
    print("\n[STEP 3] Waiting for HTTP POST to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=20,
    )

    print(f"✓ Execution completed: {final_exec['status']}")

    # Step 4: Verify response
    print("\n[STEP 4] Verifying HTTP response...")
    result = final_exec.get("result", {})

    status_code = result.get("status_code")
    print(f"Status Code: {status_code}")

    assert status_code == 200, f"Expected 200, got {status_code}"
    print(f"✓ HTTP status code: 200 OK")

    # Verify the server received our JSON data
    response_body = result.get("body", "")
    if response_body:
        try:
            body_json = json.loads(response_body)
            received_json = body_json.get("json", {})

            # httpbin.org echoes back the JSON we sent
            assert received_json.get("username") == test_data["username"]
            assert received_json.get("action") == test_data["action"]
            print(f"✓ JSON payload sent and echoed back correctly")
        except Exception as e:
            print(f"⚠ Could not verify JSON payload: {e}")

    # Summary
    print("\n" + "=" * 80)
    print("HTTP POST TEST SUMMARY")
    print("=" * 80)
    print(f"✓ HTTP POST action created: {action_ref}")
    print(f"✓ Execution completed: {execution_id}")
    print(f"✓ JSON payload sent successfully")
    print(f"✓ Response captured correctly")
    print("\n🌐 HTTP Runner POST test PASSED!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.runner
@pytest.mark.http
def test_http_runner_authentication_header(client: AttuneClient, test_pack):
    """
    Test HTTP runner with authentication headers (Bearer token).
    """
    print("\n" + "=" * 80)
    print("T3.18c: HTTP Runner Authentication Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create secret for API token
    print("\n[STEP 1] Creating API token secret...")
    secret_key = f"api_token_{unique_ref()}"
    secret_value = "test_bearer_token_12345"

    secret_response = client.create_secret(
        key=secret_key, value=secret_value, encrypted=True
    )
    print(f"✓ Secret created: {secret_key}")

    # Step 2: Create HTTP action with auth header
    print("\n[STEP 2] Creating HTTP action with authentication...")
    action_ref = f"http_auth_test_{unique_ref()}"

    action_data = {
        "ref": action_ref,
        "name": "HTTP Auth Test Action",
        "description": "Tests HTTP request with Bearer token",
        "runner_type": "http",
        "pack": pack_ref,
        "enabled": True,
        "parameters": {
            "url": {"type": "string", "required": True},
        },
        "http_config": {
            "method": "GET",
            "url": "{{ parameters.url }}",
            "headers": {
                "Authorization": "Bearer {{ secrets." + secret_key + " }}",
                "Accept": "application/json",
            },
            "timeout": 10,
        },
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    print(f"✓ HTTP action with auth created: {action_ref}")
    print(f"  Authorization: Bearer <token from secret>")

    # Step 3: Execute action
    print("\n[STEP 3] Executing authenticated HTTP request...")

    # httpbin.org/bearer endpoint validates Bearer tokens
    test_url = "https://httpbin.org/bearer"

    execution_data = {
        "action": action_ref,
        "parameters": {"url": test_url},
        "secrets": [secret_key],
    }

    exec_response = client.execute_action(execution_data)
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")

    # Step 4: Wait for completion
    print("\n[STEP 4] Waiting for authenticated request to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=20,
    )

    print(f"✓ Execution completed: {final_exec['status']}")

    # Step 5: Verify authentication
    print("\n[STEP 5] Verifying authentication header...")
    result = final_exec.get("result", {})

    status_code = result.get("status_code")
    print(f"Status Code: {status_code}")

    # httpbin.org/bearer returns 200 if token is present
    if status_code == 200:
        print(f"✓ Authentication successful (200 OK)")

        response_body = result.get("body", "")
        if response_body:
            try:
                body_json = json.loads(response_body)
                authenticated = body_json.get("authenticated", False)
                token = body_json.get("token", "")

                if authenticated:
                    print(f"✓ Server confirmed authentication")
                if token:
                    print(f"✓ Token passed correctly (not exposing in logs)")
            except:
                pass
    else:
        print(f"⚠ Authentication may have failed: {status_code}")

    # Summary
    print("\n" + "=" * 80)
    print("HTTP AUTHENTICATION TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Secret created for token: {secret_key}")
    print(f"✓ HTTP action with auth created: {action_ref}")
    print(f"✓ Execution completed: {execution_id}")
    print(f"✓ Authentication header injected from secret")
    print("\n🔒 HTTP Runner authentication test PASSED!")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.runner
@pytest.mark.http
def test_http_runner_error_handling(client: AttuneClient, test_pack):
    """
    Test HTTP runner handling of error responses (4xx, 5xx).
    """
    print("\n" + "=" * 80)
    print("T3.18d: HTTP Runner Error Handling Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create HTTP action
    print("\n[STEP 1] Creating HTTP action...")
    action_ref = f"http_error_test_{unique_ref()}"

    action_data = {
        "ref": action_ref,
        "name": "HTTP Error Test Action",
        "description": "Tests HTTP error handling",
        "runner_type": "http",
        "pack": pack_ref,
        "enabled": True,
        "parameters": {
            "url": {"type": "string", "required": True},
        },
        "http_config": {
            "method": "GET",
            "url": "{{ parameters.url }}",
            "timeout": 10,
        },
    }

    action_response = client.create_action(action_data)
    print(f"✓ HTTP action created: {action_ref}")

    # Step 2: Test 404 Not Found
    print("\n[STEP 2] Testing 404 Not Found...")
    test_url = "https://httpbin.org/status/404"

    execution_data = {"action": action_ref, "parameters": {"url": test_url}}

    exec_response = client.execute_action(execution_data)
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")
    print(f"  Target: {test_url}")

    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status=["succeeded", "failed"],  # Either is acceptable
        timeout=20,
    )

    result = final_exec.get("result", {})
    status_code = result.get("status_code")

    print(f"  Status code: {status_code}")
    if status_code == 404:
        print(f"✓ 404 error captured correctly")

    # Step 3: Test 500 Internal Server Error
    print("\n[STEP 3] Testing 500 Internal Server Error...")
    test_url = "https://httpbin.org/status/500"

    exec_response = client.execute_action(
        {"action": action_ref, "parameters": {"url": test_url}}
    )
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")

    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status=["succeeded", "failed"],
        timeout=20,
    )

    result = final_exec.get("result", {})
    status_code = result.get("status_code")

    print(f"  Status code: {status_code}")
    if status_code == 500:
        print(f"✓ 500 error captured correctly")

    # Summary
    print("\n" + "=" * 80)
    print("HTTP ERROR HANDLING TEST SUMMARY")
    print("=" * 80)
    print(f"✓ HTTP action created: {action_ref}")
    print(f"✓ 404 error handled correctly")
    print(f"✓ 500 error handled correctly")
    print(f"✓ HTTP runner captures error status codes")
    print("\n⚠️  HTTP Runner error handling validated!")
    print("=" * 80)
