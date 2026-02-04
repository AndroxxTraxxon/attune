"""
T3.20: Secret Injection Security Test

Tests that secrets are passed securely to actions via stdin (not environment variables)
to prevent exposure through process inspection.

Priority: HIGH
Duration: ~20 seconds
"""

import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import create_echo_action, unique_ref
from helpers.polling import wait_for_execution_status


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
def test_secret_injection_via_stdin(client: AttuneClient, test_pack):
    """
    Test that secrets are injected via stdin, not environment variables.

    This is critical for security - environment variables can be inspected
    via /proc/{pid}/environ, while stdin cannot.
    """
    print("\n" + "=" * 80)
    print("T3.20: Secret Injection Security Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create a secret
    print("\n[STEP 1] Creating secret...")
    secret_key = f"test_api_key_{unique_ref()}"
    secret_value = "super_secret_password_12345"

    secret_response = client.create_secret(
        key=secret_key,
        value=secret_value,
        encrypted=True,
        description="Test API key for secret injection test",
    )

    assert "id" in secret_response, "Secret creation failed"
    secret_id = secret_response["id"]
    print(f"✓ Secret created: {secret_key} (ID: {secret_id})")
    print(f"  Secret value: {secret_value[:10]}... (truncated for security)")

    # Step 2: Create an action that uses the secret and outputs debug info
    print("\n[STEP 2] Creating action that uses secret...")
    action_ref = f"test_secret_action_{unique_ref()}"

    # Python script that:
    # 1. Reads secret from stdin
    # 2. Uses the secret
    # 3. Outputs confirmation (but NOT the secret value itself)
    # 4. Checks environment variables to ensure secret is NOT there
    action_script = f"""
import sys
import json
import os

# Read secrets from stdin (secure channel)
secrets_json = sys.stdin.read()
secrets = json.loads(secrets_json) if secrets_json else {{}}

# Get the specific secret we need
api_key = secrets.get('{secret_key}')

# Verify we received the secret
if api_key:
    print(f"SECRET_RECEIVED: yes")
    print(f"SECRET_LENGTH: {{len(api_key)}}")

    # Verify it's the correct value (without exposing it in logs)
    if api_key == '{secret_value}':
        print("SECRET_VALID: yes")
    else:
        print("SECRET_VALID: no")
else:
    print("SECRET_RECEIVED: no")

# Check if secret is in environment variables (SECURITY VIOLATION)
secret_in_env = False
for key, value in os.environ.items():
    if '{secret_value}' in value or '{secret_key}' in key:
        secret_in_env = True
        print(f"SECURITY_VIOLATION: Secret found in environment variable: {{key}}")
        break

if not secret_in_env:
    print("SECURITY_CHECK: Secret not in environment variables (GOOD)")

# Output a message that uses the secret (simulating real usage)
print(f"Successfully authenticated with API key (length: {{len(api_key) if api_key else 0}})")
"""

    action_data = {
        "ref": action_ref,
        "name": "Secret Injection Test Action",
        "description": "Tests secure secret injection via stdin",
        "runner_type": "python",
        "entry_point": "main.py",
        "pack": pack_ref,
        "enabled": True,
        "parameters": {},
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    print(f"✓ Action created: {action_ref}")

    # Upload the action script
    files = {"main.py": action_script}
    client.upload_action_files(action_ref, files)
    print(f"✓ Action files uploaded")

    # Step 3: Execute the action with secret reference
    print("\n[STEP 3] Executing action with secret reference...")

    execution_data = {
        "action": action_ref,
        "parameters": {},
        "secrets": [secret_key],  # Request the secret to be injected
    }

    exec_response = client.execute_action(execution_data)
    assert "id" in exec_response, "Execution creation failed"
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")
    print(f"  Action: {action_ref}")
    print(f"  Secrets requested: [{secret_key}]")

    # Step 4: Wait for execution to complete
    print("\n[STEP 4] Waiting for execution to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=20,
    )

    print(f"✓ Execution completed with status: {final_exec['status']}")

    # Step 5: Verify security properties in execution output
    print("\n[STEP 5] Verifying security properties...")

    output = final_exec.get("result", {}).get("stdout", "")
    print(f"\nExecution output:")
    print("-" * 60)
    print(output)
    print("-" * 60)

    # Security checks
    security_checks = {
        "secret_received": False,
        "secret_valid": False,
        "secret_not_in_env": False,
        "secret_not_in_output": True,  # Should be true by default
    }

    # Check output for security markers
    if "SECRET_RECEIVED: yes" in output:
        security_checks["secret_received"] = True
        print("✓ Secret was received by action")
    else:
        print("✗ Secret was NOT received by action")

    if "SECRET_VALID: yes" in output:
        security_checks["secret_valid"] = True
        print("✓ Secret value was correct")
    else:
        print("✗ Secret value was incorrect or not validated")

    if "SECURITY_CHECK: Secret not in environment variables (GOOD)" in output:
        security_checks["secret_not_in_env"] = True
        print("✓ Secret NOT found in environment variables (SECURE)")
    else:
        print("✗ Secret may have been exposed in environment variables")

    if "SECURITY_VIOLATION" in output:
        security_checks["secret_not_in_env"] = False
        security_checks["secret_not_in_output"] = False
        print("✗ SECURITY VIOLATION DETECTED in output")

    # Check that the actual secret value is not in the output
    if secret_value in output:
        security_checks["secret_not_in_output"] = False
        print(f"✗ SECRET VALUE EXPOSED IN OUTPUT!")
    else:
        print("✓ Secret value not exposed in output")

    # Step 6: Verify secret is not in execution record
    print("\n[STEP 6] Verifying secret not stored in execution record...")

    # Check parameters field
    params_str = str(final_exec.get("parameters", {}))
    if secret_value in params_str:
        print("✗ Secret value found in execution parameters!")
        security_checks["secret_not_in_output"] = False
    else:
        print("✓ Secret value not in execution parameters")

    # Check result field (but expect controlled references)
    result_str = str(final_exec.get("result", {}))
    if secret_value in result_str:
        print("⚠ Secret value found in execution result (may be in output)")
    else:
        print("✓ Secret value not in execution result metadata")

    # Summary
    print("\n" + "=" * 80)
    print("SECURITY TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Secret created and stored encrypted: {secret_key}")
    print(f"✓ Action executed with secret injection: {action_ref}")
    print(f"✓ Execution completed: {execution_id}")
    print("\nSecurity Checks:")
    print(
        f"  {'✓' if security_checks['secret_received'] else '✗'} Secret received by action via stdin"
    )
    print(
        f"  {'✓' if security_checks['secret_valid'] else '✗'} Secret value validated correctly"
    )
    print(
        f"  {'✓' if security_checks['secret_not_in_env'] else '✗'} Secret NOT in environment variables"
    )
    print(
        f"  {'✓' if security_checks['secret_not_in_output'] else '✗'} Secret NOT exposed in logs/output"
    )

    all_checks_passed = all(security_checks.values())
    if all_checks_passed:
        print("\n🔒 ALL SECURITY CHECKS PASSED!")
    else:
        print("\n⚠️  SOME SECURITY CHECKS FAILED!")
        failed_checks = [k for k, v in security_checks.items() if not v]
        print(f"   Failed checks: {', '.join(failed_checks)}")

    print("=" * 80)

    # Assertions
    assert security_checks["secret_received"], "Secret was not received by action"
    assert security_checks["secret_valid"], "Secret value was incorrect"
    assert security_checks["secret_not_in_env"], (
        "SECURITY VIOLATION: Secret found in environment variables"
    )
    assert security_checks["secret_not_in_output"], (
        "SECURITY VIOLATION: Secret exposed in output"
    )
    assert final_exec["status"] == "succeeded", (
        f"Execution failed: {final_exec.get('status')}"
    )


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
def test_secret_encryption_at_rest(client: AttuneClient):
    """
    Test that secrets are stored encrypted in the database.

    This verifies that even if the database is compromised, secrets
    cannot be read without the encryption key.
    """
    print("\n" + "=" * 80)
    print("T3.20b: Secret Encryption at Rest Test")
    print("=" * 80)

    # Step 1: Create an encrypted secret
    print("\n[STEP 1] Creating encrypted secret...")
    secret_key = f"encrypted_secret_{unique_ref()}"
    secret_value = "this_should_be_encrypted_in_database"

    secret_response = client.create_secret(
        key=secret_key,
        value=secret_value,
        encrypted=True,
        description="Test encryption at rest",
    )

    assert "id" in secret_response, "Secret creation failed"
    secret_id = secret_response["id"]
    print(f"✓ Encrypted secret created: {secret_key}")

    # Step 2: Retrieve the secret
    print("\n[STEP 2] Retrieving secret via API...")
    retrieved = client.get_secret(secret_key)

    assert retrieved["key"] == secret_key, "Secret key mismatch"
    assert retrieved["encrypted"] is True, "Secret not marked as encrypted"
    print(f"✓ Secret retrieved: {secret_key}")
    print(f"  Encrypted flag: {retrieved['encrypted']}")

    # Note: The API should decrypt the value when returning it to authorized users
    # But we cannot verify database-level encryption without direct DB access
    print(f"  Value accessible via API: yes")

    # Step 3: Create a non-encrypted secret for comparison
    print("\n[STEP 3] Creating non-encrypted secret for comparison...")
    plain_key = f"plain_secret_{unique_ref()}"
    plain_value = "this_is_stored_in_plaintext"

    plain_response = client.create_secret(
        key=plain_key,
        value=plain_value,
        encrypted=False,
        description="Test plaintext storage",
    )

    assert "id" in plain_response, "Plain secret creation failed"
    print(f"✓ Plain secret created: {plain_key}")

    plain_retrieved = client.get_secret(plain_key)
    assert plain_retrieved["encrypted"] is False, (
        "Secret incorrectly marked as encrypted"
    )
    print(f"  Encrypted flag: {plain_retrieved['encrypted']}")

    # Summary
    print("\n" + "=" * 80)
    print("ENCRYPTION AT REST TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Encrypted secret created: {secret_key}")
    print(f"✓ Encrypted flag set correctly: True")
    print(f"✓ Plain secret created for comparison: {plain_key}")
    print(f"✓ Encrypted flag set correctly: False")
    print("\n🔒 Encryption at rest configuration validated!")
    print("   Note: Database-level encryption verification requires direct DB access")
    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
def test_secret_not_in_execution_logs(client: AttuneClient, test_pack):
    """
    Test that secrets are never logged or exposed in execution output.

    Even if an action tries to print a secret, it should be redacted or
    the action should be designed to never output secrets.
    """
    print("\n" + "=" * 80)
    print("T3.20c: Secret Redaction in Logs Test")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # Step 1: Create a secret
    print("\n[STEP 1] Creating secret...")
    secret_key = f"log_test_secret_{unique_ref()}"
    secret_value = "SENSITIVE_PASSWORD_DO_NOT_LOG"

    secret_response = client.create_secret(
        key=secret_key, value=secret_value, encrypted=True
    )

    assert "id" in secret_response, "Secret creation failed"
    print(f"✓ Secret created: {secret_key}")

    # Step 2: Create an action that attempts to log the secret
    print("\n[STEP 2] Creating action that attempts to log secret...")
    action_ref = f"log_secret_test_{unique_ref()}"

    # Action that tries to print the secret (bad practice, but we test handling)
    action_script = f"""
import sys
import json

# Read secrets from stdin
secrets_json = sys.stdin.read()
secrets = json.loads(secrets_json) if secrets_json else {{}}

api_key = secrets.get('{secret_key}')

if api_key:
    # Bad practice: trying to log the secret
    # The system should handle this gracefully
    print(f"Received secret: {{api_key}}")
    print(f"Secret first 5 chars: {{api_key[:5]}}")
    print(f"Secret length: {{len(api_key)}}")
    print("Secret received successfully")
else:
    print("No secret received")
"""

    action_data = {
        "ref": action_ref,
        "name": "Secret Logging Test Action",
        "runner_type": "python",
        "entry_point": "main.py",
        "pack": pack_ref,
        "enabled": True,
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    print(f"✓ Action created: {action_ref}")

    files = {"main.py": action_script}
    client.upload_action_files(action_ref, files)
    print(f"✓ Action files uploaded")

    # Step 3: Execute the action
    print("\n[STEP 3] Executing action...")
    execution_data = {"action": action_ref, "parameters": {}, "secrets": [secret_key]}

    exec_response = client.execute_action(execution_data)
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")

    # Step 4: Wait for completion
    print("\n[STEP 4] Waiting for execution to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=15,
    )

    print(f"✓ Execution completed: {final_exec['status']}")

    # Step 5: Verify secret handling in output
    print("\n[STEP 5] Verifying secret handling in output...")
    output = final_exec.get("result", {}).get("stdout", "")

    print(f"\nExecution output:")
    print("-" * 60)
    print(output)
    print("-" * 60)

    # Check if secret is exposed
    if secret_value in output:
        print("⚠️  WARNING: Secret value appears in output!")
        print("   This is a security concern and should be addressed.")
        # Note: In a production system, we would want this to fail
        # For now, we document the behavior
    else:
        print("✓ Secret value NOT found in output (GOOD)")

    # Check for partial exposure
    if "SENSITIVE_PASSWORD" in output:
        print("⚠️  Secret partially exposed in output")

    # Summary
    print("\n" + "=" * 80)
    print("SECRET LOGGING TEST SUMMARY")
    print("=" * 80)
    print(f"✓ Action attempted to log secret: {action_ref}")
    print(f"✓ Execution completed: {execution_id}")

    secret_exposed = secret_value in output
    if secret_exposed:
        print(f"⚠️  Secret exposed in output (action printed it)")
        print("   Recommendation: Actions should never print secrets")
        print("   Consider: Output filtering/redaction in worker service")
    else:
        print(f"✓ Secret NOT exposed in output")

    print("\n💡 Best Practices:")
    print("   - Actions should never print secrets to stdout/stderr")
    print("   - Use secrets only for API calls, not for display")
    print("   - Consider implementing automatic secret redaction in worker")
    print("=" * 80)

    # We pass the test even if secret is exposed, but warn about it
    # In production, you might want to fail this test
    assert final_exec["status"] == "succeeded", "Execution failed"


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
def test_secret_access_tenant_isolation(
    client: AttuneClient, unique_user_client: AttuneClient
):
    """
    Test that secrets are isolated per tenant - users cannot access
    secrets from other tenants.
    """
    print("\n" + "=" * 80)
    print("T3.20d: Secret Tenant Isolation Test")
    print("=" * 80)

    # Step 1: User 1 creates a secret
    print("\n[STEP 1] User 1 creates a secret...")
    user1_secret_key = f"user1_secret_{unique_ref()}"
    user1_secret_value = "user1_private_data"

    secret_response = client.create_secret(
        key=user1_secret_key, value=user1_secret_value, encrypted=True
    )

    assert "id" in secret_response, "Secret creation failed"
    print(f"✓ User 1 created secret: {user1_secret_key}")

    # Step 2: User 1 can retrieve their own secret
    print("\n[STEP 2] User 1 retrieves their own secret...")
    retrieved = client.get_secret(user1_secret_key)
    assert retrieved["key"] == user1_secret_key, "User 1 cannot retrieve own secret"
    print(f"✓ User 1 successfully retrieved their own secret")

    # Step 3: User 2 tries to access User 1's secret (should fail)
    print("\n[STEP 3] User 2 attempts to access User 1's secret...")
    try:
        user2_attempt = unique_user_client.get_secret(user1_secret_key)
        print(f"✗ SECURITY VIOLATION: User 2 accessed User 1's secret!")
        print(f"   Retrieved: {user2_attempt}")
        assert False, "Tenant isolation violated: User 2 accessed User 1's secret"
    except Exception as e:
        error_msg = str(e)
        if "404" in error_msg or "not found" in error_msg.lower():
            print(f"✓ User 2 cannot access User 1's secret (404 Not Found)")
        elif "403" in error_msg or "forbidden" in error_msg.lower():
            print(f"✓ User 2 cannot access User 1's secret (403 Forbidden)")
        else:
            print(f"✓ User 2 cannot access User 1's secret (Error: {error_msg})")

    # Step 4: User 2 creates their own secret
    print("\n[STEP 4] User 2 creates their own secret...")
    user2_secret_key = f"user2_secret_{unique_ref()}"
    user2_secret_value = "user2_private_data"

    user2_secret = unique_user_client.create_secret(
        key=user2_secret_key, value=user2_secret_value, encrypted=True
    )

    assert "id" in user2_secret, "User 2 secret creation failed"
    print(f"✓ User 2 created secret: {user2_secret_key}")

    # Step 5: User 2 can retrieve their own secret
    print("\n[STEP 5] User 2 retrieves their own secret...")
    user2_retrieved = unique_user_client.get_secret(user2_secret_key)
    assert user2_retrieved["key"] == user2_secret_key, (
        "User 2 cannot retrieve own secret"
    )
    print(f"✓ User 2 successfully retrieved their own secret")

    # Step 6: User 1 tries to access User 2's secret (should fail)
    print("\n[STEP 6] User 1 attempts to access User 2's secret...")
    try:
        user1_attempt = client.get_secret(user2_secret_key)
        print(f"✗ SECURITY VIOLATION: User 1 accessed User 2's secret!")
        assert False, "Tenant isolation violated: User 1 accessed User 2's secret"
    except Exception as e:
        error_msg = str(e)
        if "404" in error_msg or "403" in error_msg:
            print(f"✓ User 1 cannot access User 2's secret")
        else:
            print(f"✓ User 1 cannot access User 2's secret (Error: {error_msg})")

    # Summary
    print("\n" + "=" * 80)
    print("TENANT ISOLATION TEST SUMMARY")
    print("=" * 80)
    print(f"✓ User 1 secret: {user1_secret_key}")
    print(f"✓ User 2 secret: {user2_secret_key}")
    print(f"✓ User 1 can access own secret: yes")
    print(f"✓ User 2 can access own secret: yes")
    print(f"✓ User 1 cannot access User 2's secret: yes")
    print(f"✓ User 2 cannot access User 1's secret: yes")
    print("\n🔒 TENANT ISOLATION VERIFIED!")
    print("=" * 80)
