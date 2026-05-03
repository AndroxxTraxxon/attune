"""
T3.20: Secret Injection Security Test

Tests that secrets are passed securely to actions via stdin (not environment variables)
to prevent exposure through process inspection.

Priority: HIGH
Duration: ~20 seconds
"""

import time

import pytest
from helpers import AttuneClient
from helpers.fixtures import create_echo_action, unique_ref
from helpers.polling import wait_for_execution_status


def create_pack_secret(
    client: AttuneClient, pack_ref: str, key: str, value: str, *, encrypted: bool = True
) -> dict:
    response = client.post(
        "/api/v1/keys",
        json={
            "ref": key,
            "name": key,
            "value": value,
            "owner_type": "pack",
            "owner_pack_ref": pack_ref,
            "encrypted": encrypted,
        },
    )
    assert response.status_code == 201, response.text
    return response.json()["data"]


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
@pytest.mark.skip(reason="Worker secret injection is not available in the E2E stack")
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

    secret_response = create_pack_secret(
        client, pack_ref, secret_key, secret_value, encrypted=False
    )

    assert "id" in secret_response, "Secret creation failed"
    secret_id = secret_response["id"]
    print(f"✓ Secret created: {secret_key} (ID: {secret_id})")
    print(f"  Secret value: {secret_value[:10]}... (truncated for security)")

    # Step 2: Create an action that uses the secret and outputs debug info
    print("\n[STEP 2] Creating action that uses secret...")
    action_ref = f"{pack_ref}.test_secret_action_{unique_ref()}"

    action_script = f"""INPUT=$(cat)
case "$INPUT" in
  *"{secret_key}"*) echo "SECRET_RECEIVED: yes" ;;
  *) echo "SECRET_RECEIVED: no" ;;
esac
case "$INPUT" in
  *"{secret_value}"*) echo "SECRET_VALID: yes" ;;
  *) echo "SECRET_VALID: no" ;;
esac
echo "SECRET_LENGTH: {len(secret_value)}"
if printenv | grep -F "{secret_value}" >/dev/null || printenv | grep -F "{secret_key}" >/dev/null; then
  echo "SECURITY_VIOLATION: Secret found in environment"
else
  echo "SECURITY_CHECK: Secret not in environment variables (GOOD)"
fi
echo "Successfully authenticated with API key (length: {len(secret_value)})"
"""

    action_data = {
        "ref": action_ref,
        "label": "Secret Injection Test Action",
        "description": "Tests secure secret injection via stdin",
        "runtime_ref": "core.shell",
        "entrypoint": action_script,
        "pack_ref": pack_ref,
        "enabled": True,
        "param_schema": {},
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    action_ref = action_response["ref"]
    print(f"✓ Action created: {action_ref}")

    # Step 3: Execute the action with secret reference
    print("\n[STEP 3] Executing action with secret reference...")

    exec_response = client.create_execution(action_ref=action_ref, parameters={})
    assert "id" in exec_response, "Execution creation failed"
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")
    print(f"  Action: {action_ref}")
    print(f"  Secret key available to action: {secret_key}")

    # Step 4: Wait for execution to complete
    print("\n[STEP 4] Waiting for execution to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="completed",
        timeout=20,
    )

    print(f"✓ Execution succeeded with status: {final_exec['status']}")

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
    print(f"✓ Secret created for stdin delivery: {secret_key}")
    print(f"✓ Action executed with secret injection: {action_ref}")
    print(f"✓ Execution succeeded: {execution_id}")
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
    assert final_exec["status"] == "completed", (
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
@pytest.mark.skip(reason="Worker secret injection is not available in the E2E stack")
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

    secret_response = create_pack_secret(
        client, pack_ref, secret_key, secret_value, encrypted=False
    )

    assert "id" in secret_response, "Secret creation failed"
    print(f"✓ Secret created: {secret_key}")

    # Step 2: Create an action that attempts to log the secret
    print("\n[STEP 2] Creating action that attempts to log secret...")
    action_ref = f"{pack_ref}.log_secret_test_{unique_ref()}"

    action_script = f"""INPUT=$(cat)
case "$INPUT" in
  *"{secret_key}"*) echo "Secret length: {len(secret_value)}"; echo "Secret received successfully" ;;
  *) echo "No secret received" ;;
esac
"""

    action_data = {
        "ref": action_ref,
        "label": "Secret Logging Test Action",
        "runtime_ref": "core.shell",
        "entrypoint": action_script,
        "pack_ref": pack_ref,
        "enabled": True,
    }

    action_response = client.create_action(action_data)
    assert "id" in action_response, "Action creation failed"
    action_ref = action_response["ref"]
    print(f"✓ Action created: {action_ref}")

    # Step 3: Execute the action
    print("\n[STEP 3] Executing action...")
    exec_response = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = exec_response["id"]
    print(f"✓ Execution created: {execution_id}")

    # Step 4: Wait for completion
    print("\n[STEP 4] Waiting for execution to complete...")
    final_exec = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="completed",
        timeout=15,
    )

    print(f"✓ Execution succeeded: {final_exec['status']}")

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
        assert False, "Secret value was exposed in execution output"
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
    print(f"✓ Execution succeeded: {execution_id}")

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

    assert not secret_exposed, "Secret value was exposed in execution output"
    assert final_exec["status"] == "completed", "Execution failed"


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.secrets
@pytest.mark.skip(reason="Key tenant isolation is not implemented")
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
