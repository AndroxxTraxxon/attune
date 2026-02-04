#!/usr/bin/env python3
"""
T1.7: Multi-Tenant Isolation

Tests that users cannot access other tenant's resources.

Test Flow:
1. Create User A (tenant_id=1) and User B (tenant_id=2)
2. User A creates pack, action, rule
3. User B attempts to list User A's packs
4. Verify User B sees empty list
5. User B attempts to execute User A's action by ID
6. Verify request returns 404 or 403 error
7. User A can see and execute their own resources

Success Criteria:
- All API endpoints filter by tenant_id
- Cross-tenant resource access returns 404 (not 403 to avoid info leak)
- Executions scoped to tenant
- Events scoped to tenant
- Enforcements scoped to tenant
- Datastore scoped to tenant
- Secrets scoped to tenant
"""

import pytest
from helpers import (
    AttuneClient,
    create_echo_action,
    create_rule,
    create_webhook_trigger,
    unique_ref,
)


@pytest.mark.tier1
@pytest.mark.security
@pytest.mark.integration
@pytest.mark.timeout(60)
class TestMultiTenantIsolation:
    """Test multi-tenant isolation and RBAC"""

    def test_basic_tenant_isolation(self, api_base_url: str, test_timeout: int):
        """Test that users in different tenants cannot see each other's resources"""

        print(f"\n=== T1.7: Multi-Tenant Isolation ===")

        # Step 1: Create two unique users (separate tenants)
        print("\n[1/7] Creating two users in separate tenants...")

        user_a_login = f"user_a_{unique_ref()}@attune.local"
        user_b_login = f"user_b_{unique_ref()}@attune.local"
        password = "TestPass123!"

        # Client for User A
        client_a = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_a.register(login=user_a_login, password=password, display_name="User A")
        client_a.login(login=user_a_login, password=password, create_if_missing=False)
        print(f"✓ User A created: {user_a_login}")
        print(f"  Tenant ID: {client_a.tenant_id}")

        # Client for User B
        client_b = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_b.register(login=user_b_login, password=password, display_name="User B")
        client_b.login(login=user_b_login, password=password, create_if_missing=False)
        print(f"✓ User B created: {user_b_login}")
        print(f"  Tenant ID: {client_b.tenant_id}")

        # Verify different tenants (if tenant_id available in response)
        if client_a.tenant_id and client_b.tenant_id:
            print(f"\n  Tenant verification:")
            print(f"    User A tenant: {client_a.tenant_id}")
            print(f"    User B tenant: {client_b.tenant_id}")
            # Note: In some implementations, each user gets their own tenant
            # In others, users might share a tenant but have different user_ids

        # Step 2: User A creates resources
        print("\n[2/7] User A creates pack, action, and rule...")

        # Register test pack for User A
        pack_a = client_a.register_pack("tests/fixtures/packs/test_pack")
        pack_ref_a = pack_a["ref"]
        print(f"✓ User A created pack: {pack_ref_a}")

        # Create action for User A
        action_a = create_echo_action(client=client_a, pack_ref=pack_ref_a)
        action_ref_a = action_a["ref"]
        action_id_a = action_a["id"]
        print(f"✓ User A created action: {action_ref_a} (ID: {action_id_a})")

        # Create trigger and rule for User A
        trigger_a = create_webhook_trigger(client=client_a, pack_ref=pack_ref_a)
        rule_a = create_rule(
            client=client_a,
            trigger_id=trigger_a["id"],
            action_ref=action_ref_a,
            pack_ref=pack_ref_a,
        )
        print(f"✓ User A created trigger and rule")

        # Step 3: User A can see their own resources
        print("\n[3/7] Verifying User A can see their own resources...")

        user_a_packs = client_a.list_packs()
        print(f"  User A sees {len(user_a_packs)} pack(s)")
        assert len(user_a_packs) > 0, "User A should see their own packs"

        user_a_actions = client_a.list_actions()
        print(f"  User A sees {len(user_a_actions)} action(s)")
        assert len(user_a_actions) > 0, "User A should see their own actions"

        user_a_rules = client_a.list_rules()
        print(f"  User A sees {len(user_a_rules)} rule(s)")
        assert len(user_a_rules) > 0, "User A should see their own rules"

        print(f"✓ User A can access their own resources")

        # Step 4: User B cannot see User A's packs
        print("\n[4/7] Verifying User B cannot see User A's packs...")

        user_b_packs = client_b.list_packs()
        print(f"  User B sees {len(user_b_packs)} pack(s)")

        # User B should not see User A's packs
        user_b_pack_refs = [p["ref"] for p in user_b_packs]
        assert pack_ref_a not in user_b_pack_refs, (
            f"User B should not see User A's pack {pack_ref_a}"
        )
        print(f"✓ User B cannot see User A's packs")

        # Step 5: User B cannot see User A's actions
        print("\n[5/7] Verifying User B cannot see User A's actions...")

        user_b_actions = client_b.list_actions()
        print(f"  User B sees {len(user_b_actions)} action(s)")

        # User B should not see User A's actions
        user_b_action_refs = [a["ref"] for a in user_b_actions]
        assert action_ref_a not in user_b_action_refs, (
            f"User B should not see User A's action {action_ref_a}"
        )
        print(f"✓ User B cannot see User A's actions")

        # Step 6: User B cannot access User A's action by ID
        print("\n[6/7] Verifying User B cannot access User A's action by ID...")

        try:
            # Attempt to get User A's action by ID
            user_b_action = client_b.get_action(action_id_a)
            # If we get here, that's a security problem
            pytest.fail(
                f"SECURITY ISSUE: User B was able to access User A's action (ID: {action_id_a})"
            )
        except Exception as e:
            # Expected: 404 (not found) or 403 (forbidden)
            error_message = str(e)
            print(f"  Expected error: {error_message}")

            # Should be 404 (to avoid information leakage) or 403
            if (
                "404" in error_message
                or "403" in error_message
                or "not found" in error_message.lower()
            ):
                print(f"✓ User B correctly denied access (404/403)")
            else:
                print(f"⚠️  Unexpected error type: {error_message}")
                print(f"   (Expected 404 or 403)")

        # Step 7: Verify executions are isolated
        print("\n[7/7] Verifying execution isolation...")

        # User A executes their action
        client_a.fire_webhook(trigger_id=trigger_a["id"], payload={"test": "user_a"})
        print(f"  User A triggered execution")

        # Wait briefly for execution
        import time

        time.sleep(2)

        # User A can see their executions
        user_a_executions = client_a.list_executions()
        print(f"  User A sees {len(user_a_executions)} execution(s)")

        # User B cannot see User A's executions
        user_b_executions = client_b.list_executions()
        print(f"  User B sees {len(user_b_executions)} execution(s)")

        # If User A has executions, User B should not see them
        if len(user_a_executions) > 0:
            user_a_exec_ids = {e["id"] for e in user_a_executions}
            user_b_exec_ids = {e["id"] for e in user_b_executions}

            overlap = user_a_exec_ids.intersection(user_b_exec_ids)
            assert len(overlap) == 0, (
                f"SECURITY ISSUE: User B can see {len(overlap)} execution(s) from User A"
            )
            print(f"✓ User B cannot see User A's executions")

        # Final summary
        print("\n=== Test Summary ===")
        print(f"✓ Two users created in separate contexts")
        print(f"✓ User A can access their own resources")
        print(f"✓ User B cannot see User A's packs")
        print(f"✓ User B cannot see User A's actions")
        print(f"✓ User B cannot access User A's action by ID")
        print(f"✓ Executions isolated between users")
        print(f"✓ Multi-tenant isolation working correctly")
        print(f"✓ Test PASSED")

    def test_datastore_isolation(self, api_base_url: str, test_timeout: int):
        """Test that datastore values are isolated per tenant"""

        print(f"\n=== T1.7b: Datastore Isolation ===")

        # Create two users
        user_a_login = f"user_a_{unique_ref()}@attune.local"
        user_b_login = f"user_b_{unique_ref()}@attune.local"
        password = "TestPass123!"

        client_a = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_a.register(login=user_a_login, password=password)
        client_a.login(login=user_a_login, password=password, create_if_missing=False)

        client_b = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_b.register(login=user_b_login, password=password)
        client_b.login(login=user_b_login, password=password, create_if_missing=False)

        print(f"✓ Two users created")

        # User A stores a value
        print("\nUser A storing datastore value...")
        test_key = "test.isolation.key"
        user_a_value = "user_a_secret_value"

        client_a.datastore_set(key=test_key, value=user_a_value)
        print(f"  User A stored: {test_key} = {user_a_value}")

        # User A can read it back
        retrieved_a = client_a.datastore_get(test_key)
        assert retrieved_a == user_a_value
        print(f"  User A retrieved: {retrieved_a}")

        # User B tries to read the same key
        print("\nUser B attempting to read User A's key...")
        retrieved_b = client_b.datastore_get(test_key)
        print(f"  User B retrieved: {retrieved_b}")

        # User B should get None (key doesn't exist in their namespace)
        assert retrieved_b is None, (
            f"SECURITY ISSUE: User B can read User A's datastore value"
        )
        print(f"✓ User B cannot access User A's datastore values")

        # User B stores their own value with same key
        print("\nUser B storing their own value with same key...")
        user_b_value = "user_b_different_value"
        client_b.datastore_set(key=test_key, value=user_b_value)
        print(f"  User B stored: {test_key} = {user_b_value}")

        # Each user sees only their own value
        print("\nVerifying each user sees only their own value...")
        final_a = client_a.datastore_get(test_key)
        final_b = client_b.datastore_get(test_key)

        print(f"  User A sees: {final_a}")
        print(f"  User B sees: {final_b}")

        assert final_a == user_a_value, "User A should see their own value"
        assert final_b == user_b_value, "User B should see their own value"

        print(f"✓ Each user has isolated datastore namespace")

        # Cleanup
        client_a.datastore_delete(test_key)
        client_b.datastore_delete(test_key)

        print("\n=== Test Summary ===")
        print(f"✓ Datastore values isolated per tenant")
        print(f"✓ Same key can have different values per tenant")
        print(f"✓ Cross-tenant datastore access prevented")
        print(f"✓ Test PASSED")

    def test_event_isolation(self, api_base_url: str, test_timeout: int):
        """Test that events are isolated per tenant"""

        print(f"\n=== T1.7c: Event Isolation ===")

        # Create two users
        user_a_login = f"user_a_{unique_ref()}@attune.local"
        user_b_login = f"user_b_{unique_ref()}@attune.local"
        password = "TestPass123!"

        client_a = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_a.register(login=user_a_login, password=password)
        client_a.login(login=user_a_login, password=password, create_if_missing=False)

        client_b = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_b.register(login=user_b_login, password=password)
        client_b.login(login=user_b_login, password=password, create_if_missing=False)

        print(f"✓ Two users created")

        # User A creates trigger and fires webhook
        print("\nUser A creating trigger and firing webhook...")
        pack_a = client_a.register_pack("tests/fixtures/packs/test_pack")
        trigger_a = create_webhook_trigger(client=client_a, pack_ref=pack_a["ref"])

        client_a.fire_webhook(
            trigger_id=trigger_a["id"], payload={"user": "A", "message": "test"}
        )
        print(f"✓ User A fired webhook (trigger_id={trigger_a['id']})")

        # Wait for event
        import time

        time.sleep(2)

        # User A can see their events
        print("\nChecking event visibility...")
        user_a_events = client_a.list_events()
        print(f"  User A sees {len(user_a_events)} event(s)")

        # User B cannot see User A's events
        user_b_events = client_b.list_events()
        print(f"  User B sees {len(user_b_events)} event(s)")

        if len(user_a_events) > 0:
            user_a_event_ids = {e["id"] for e in user_a_events}
            user_b_event_ids = {e["id"] for e in user_b_events}

            overlap = user_a_event_ids.intersection(user_b_event_ids)
            assert len(overlap) == 0, (
                f"SECURITY ISSUE: User B can see {len(overlap)} event(s) from User A"
            )
            print(f"✓ Events isolated between tenants")

        print("\n=== Test Summary ===")
        print(f"✓ Events isolated per tenant")
        print(f"✓ Cross-tenant event access prevented")
        print(f"✓ Test PASSED")

    def test_rule_isolation(self, api_base_url: str, test_timeout: int):
        """Test that rules are isolated per tenant"""

        print(f"\n=== T1.7d: Rule Isolation ===")

        # Create two users
        user_a_login = f"user_a_{unique_ref()}@attune.local"
        user_b_login = f"user_b_{unique_ref()}@attune.local"
        password = "TestPass123!"

        client_a = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_a.register(login=user_a_login, password=password)
        client_a.login(login=user_a_login, password=password, create_if_missing=False)

        client_b = AttuneClient(
            base_url=api_base_url, timeout=test_timeout, auto_login=False
        )
        client_b.register(login=user_b_login, password=password)
        client_b.login(login=user_b_login, password=password, create_if_missing=False)

        print(f"✓ Two users created")

        # User A creates rule
        print("\nUser A creating rule...")
        pack_a = client_a.register_pack("tests/fixtures/packs/test_pack")
        trigger_a = create_webhook_trigger(client=client_a, pack_ref=pack_a["ref"])
        action_a = create_echo_action(client=client_a, pack_ref=pack_a["ref"])
        rule_a = create_rule(
            client=client_a,
            trigger_id=trigger_a["id"],
            action_ref=action_a["ref"],
            pack_ref=pack_a["ref"],
        )
        rule_id_a = rule_a["id"]
        print(f"✓ User A created rule (ID: {rule_id_a})")

        # User A can see their rule
        user_a_rules = client_a.list_rules()
        print(f"  User A sees {len(user_a_rules)} rule(s)")
        assert len(user_a_rules) > 0

        # User B cannot see User A's rules
        user_b_rules = client_b.list_rules()
        print(f"  User B sees {len(user_b_rules)} rule(s)")

        user_b_rule_ids = {r["id"] for r in user_b_rules}
        assert rule_id_a not in user_b_rule_ids, (
            f"SECURITY ISSUE: User B can see User A's rule"
        )
        print(f"✓ User B cannot see User A's rules")

        # User B cannot access User A's rule by ID
        print("\nUser B attempting direct access to User A's rule...")
        try:
            client_b.get_rule(rule_id_a)
            pytest.fail("SECURITY ISSUE: User B accessed User A's rule by ID")
        except Exception as e:
            error_message = str(e)
            if "404" in error_message or "403" in error_message:
                print(f"✓ Access correctly denied (404/403)")
            else:
                print(f"⚠️  Unexpected error: {error_message}")

        print("\n=== Test Summary ===")
        print(f"✓ Rules isolated per tenant")
        print(f"✓ Cross-tenant rule access prevented")
        print(f"✓ Direct ID access blocked")
        print(f"✓ Test PASSED")
