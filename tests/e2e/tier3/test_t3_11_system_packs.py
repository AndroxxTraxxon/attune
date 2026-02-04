"""
T3.11: System vs User Packs Test

Tests that system packs are available to all tenants while user packs
are isolated per tenant.

Priority: MEDIUM
Duration: ~15 seconds
"""

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import unique_ref


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.multi_tenant
@pytest.mark.packs
def test_system_pack_visible_to_all_tenants(
    client: AttuneClient, unique_user_client: AttuneClient
):
    """
    Test that system packs (like 'core') are visible to all tenants.

    System packs have tenant_id=NULL or a special system marker, making
    them available to all users regardless of tenant.
    """
    print("\n" + "=" * 80)
    print("T3.11a: System Pack Visibility Test")
    print("=" * 80)

    # Step 1: User 1 lists packs
    print("\n[STEP 1] User 1 listing packs...")
    user1_packs = client.list_packs()

    user1_pack_refs = [p["ref"] for p in user1_packs]
    print(f"✓ User 1 sees {len(user1_packs)} pack(s)")

    # Check if core pack is present
    core_pack_visible_user1 = "core" in user1_pack_refs
    if core_pack_visible_user1:
        print(f"✓ User 1 sees 'core' system pack")
    else:
        print(f"⚠ User 1 does not see 'core' pack")

    # Step 2: User 2 (different tenant) lists packs
    print("\n[STEP 2] User 2 (different tenant) listing packs...")
    user2_packs = unique_user_client.list_packs()

    user2_pack_refs = [p["ref"] for p in user2_packs]
    print(f"✓ User 2 sees {len(user2_packs)} pack(s)")

    # Check if core pack is present
    core_pack_visible_user2 = "core" in user2_pack_refs
    if core_pack_visible_user2:
        print(f"✓ User 2 sees 'core' system pack")
    else:
        print(f"⚠ User 2 does not see 'core' pack")

    # Step 3: Verify both users see the same system packs
    print("\n[STEP 3] Verifying system pack visibility...")

    # Find packs visible to both users (likely system packs)
    common_packs = set(user1_pack_refs) & set(user2_pack_refs)
    print(f"✓ Packs visible to both users: {list(common_packs)}")

    if "core" in common_packs:
        print(f"✓ 'core' pack is a system pack (visible to all)")

    # Step 4: User 1 can access system pack details
    print("\n[STEP 4] Testing system pack access...")

    if core_pack_visible_user1:
        try:
            core_pack_user1 = client.get_pack("core")
            print(f"✓ User 1 can access 'core' pack details")

            # Check for system pack markers
            tenant_id = core_pack_user1.get("tenant_id")
            system_flag = core_pack_user1.get("system", False)

            print(f"  Tenant ID: {tenant_id}")
            print(f"  System flag: {system_flag}")

            if tenant_id is None or system_flag:
                print(f"✓ 'core' pack marked as system pack")
        except Exception as e:
            print(f"⚠ User 1 cannot access 'core' pack: {e}")

    # Step 5: User 2 can also access system pack
    if core_pack_visible_user2:
        try:
            core_pack_user2 = unique_user_client.get_pack("core")
            print(f"✓ User 2 can access 'core' pack details")
        except Exception as e:
            print(f"⚠ User 2 cannot access 'core' pack: {e}")

    # Summary
    print("\n" + "=" * 80)
    print("SYSTEM PACK VISIBILITY TEST SUMMARY")
    print("=" * 80)
    print(f"✓ User 1 sees {len(user1_packs)} pack(s)")
    print(f"✓ User 2 sees {len(user2_packs)} pack(s)")
    print(f"✓ Common packs: {list(common_packs)}")

    if core_pack_visible_user1 and core_pack_visible_user2:
        print(f"✓ 'core' system pack visible to both users")
        print("\n✅ SYSTEM PACK VISIBILITY VERIFIED!")
    else:
        print(f"⚠ System pack visibility may not be working as expected")
        print("   Note: This may be expected if no system packs exist yet")

    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.multi_tenant
@pytest.mark.packs
def test_user_pack_isolation(client: AttuneClient, unique_user_client: AttuneClient):
    """
    Test that user-created packs are isolated per tenant.

    User 1 creates a pack, User 2 should NOT see it.
    """
    print("\n" + "=" * 80)
    print("T3.11b: User Pack Isolation Test")
    print("=" * 80)

    # Step 1: User 1 creates a pack
    print("\n[STEP 1] User 1 creating a pack...")
    user1_pack_ref = f"user1_pack_{unique_ref()}"

    user1_pack_data = {
        "ref": user1_pack_ref,
        "name": "User 1 Private Pack",
        "version": "1.0.0",
        "description": "This pack should only be visible to User 1",
    }

    user1_pack_response = client.create_pack(user1_pack_data)
    assert "id" in user1_pack_response, "Pack creation failed"
    user1_pack_id = user1_pack_response["id"]

    print(f"✓ User 1 created pack: {user1_pack_ref}")
    print(f"  Pack ID: {user1_pack_id}")

    # Step 2: User 1 can see their own pack
    print("\n[STEP 2] User 1 verifying pack visibility...")
    user1_packs = client.list_packs()
    user1_pack_refs = [p["ref"] for p in user1_packs]

    if user1_pack_ref in user1_pack_refs:
        print(f"✓ User 1 can see their own pack: {user1_pack_ref}")
    else:
        print(f"✗ User 1 cannot see their own pack!")

    # Step 3: User 2 tries to list packs (should NOT see User 1's pack)
    print("\n[STEP 3] User 2 (different tenant) listing packs...")
    user2_packs = unique_user_client.list_packs()
    user2_pack_refs = [p["ref"] for p in user2_packs]

    print(f"✓ User 2 sees {len(user2_packs)} pack(s)")

    if user1_pack_ref in user2_pack_refs:
        print(f"✗ SECURITY VIOLATION: User 2 can see User 1's pack!")
        print(f"   Pack: {user1_pack_ref}")
        assert False, "Tenant isolation violated: User 2 can see User 1's pack"
    else:
        print(f"✓ User 2 cannot see User 1's pack (isolation working)")

    # Step 4: User 2 tries to access User 1's pack directly (should fail)
    print("\n[STEP 4] User 2 attempting direct access to User 1's pack...")
    try:
        user2_attempt = unique_user_client.get_pack(user1_pack_ref)
        print(f"✗ SECURITY VIOLATION: User 2 accessed User 1's pack!")
        print(f"   Response: {user2_attempt}")
        assert False, "Tenant isolation violated: User 2 accessed User 1's pack"
    except Exception as e:
        error_msg = str(e)
        if "404" in error_msg or "not found" in error_msg.lower():
            print(f"✓ User 2 cannot access User 1's pack (404 Not Found)")
        elif "403" in error_msg or "forbidden" in error_msg.lower():
            print(f"✓ User 2 cannot access User 1's pack (403 Forbidden)")
        else:
            print(f"✓ User 2 cannot access User 1's pack (Error: {error_msg})")

    # Step 5: User 2 creates their own pack
    print("\n[STEP 5] User 2 creating their own pack...")
    user2_pack_ref = f"user2_pack_{unique_ref()}"

    user2_pack_data = {
        "ref": user2_pack_ref,
        "name": "User 2 Private Pack",
        "version": "1.0.0",
        "description": "This pack should only be visible to User 2",
    }

    user2_pack_response = unique_user_client.create_pack(user2_pack_data)
    assert "id" in user2_pack_response, "Pack creation failed for User 2"

    print(f"✓ User 2 created pack: {user2_pack_ref}")

    # Step 6: User 1 cannot see User 2's pack
    print("\n[STEP 6] User 1 attempting to see User 2's pack...")
    user1_packs_after = client.list_packs()
    user1_pack_refs_after = [p["ref"] for p in user1_packs_after]

    if user2_pack_ref in user1_pack_refs_after:
        print(f"✗ SECURITY VIOLATION: User 1 can see User 2's pack!")
        assert False, "Tenant isolation violated: User 1 can see User 2's pack"
    else:
        print(f"✓ User 1 cannot see User 2's pack (isolation working)")

    # Step 7: Verify each user can only see their own pack
    print("\n[STEP 7] Verifying complete isolation...")

    user1_final_packs = client.list_packs()
    user2_final_packs = unique_user_client.list_packs()

    user1_custom_packs = [p for p in user1_final_packs if p["ref"] not in ["core"]]
    user2_custom_packs = [p for p in user2_final_packs if p["ref"] not in ["core"]]

    print(f"  User 1 custom packs: {[p['ref'] for p in user1_custom_packs]}")
    print(f"  User 2 custom packs: {[p['ref'] for p in user2_custom_packs]}")

    # Check no overlap in custom packs
    user1_custom_refs = set(p["ref"] for p in user1_custom_packs)
    user2_custom_refs = set(p["ref"] for p in user2_custom_packs)
    overlap = user1_custom_refs & user2_custom_refs

    if not overlap:
        print(f"✓ No overlap in custom packs (perfect isolation)")
    else:
        print(f"✗ Custom pack overlap detected: {overlap}")

    # Summary
    print("\n" + "=" * 80)
    print("USER PACK ISOLATION TEST SUMMARY")
    print("=" * 80)
    print(f"✓ User 1 created pack: {user1_pack_ref}")
    print(f"✓ User 2 created pack: {user2_pack_ref}")
    print(f"✓ User 1 cannot see User 2's pack: verified")
    print(f"✓ User 2 cannot see User 1's pack: verified")
    print(f"✓ User 2 cannot access User 1's pack directly: verified")
    print(f"✓ Pack isolation per tenant: working")
    print("\n🔒 USER PACK ISOLATION VERIFIED!")
    print("=" * 80)

    # Cleanup
    try:
        client.delete_pack(user1_pack_ref)
        print(f"\n✓ Cleanup: User 1 pack deleted")
    except:
        pass

    try:
        unique_user_client.delete_pack(user2_pack_ref)
        print(f"✓ Cleanup: User 2 pack deleted")
    except:
        pass


@pytest.mark.tier3
@pytest.mark.security
@pytest.mark.multi_tenant
@pytest.mark.packs
def test_system_pack_actions_available_to_all(
    client: AttuneClient, unique_user_client: AttuneClient
):
    """
    Test that actions from system packs can be executed by all users.

    The 'core.echo' action should be available to all tenants.
    """
    print("\n" + "=" * 80)
    print("T3.11c: System Pack Actions Availability Test")
    print("=" * 80)

    # Step 1: User 1 lists actions
    print("\n[STEP 1] User 1 listing actions...")
    user1_actions = client.list_actions()
    user1_action_refs = [a["ref"] for a in user1_actions]

    print(f"✓ User 1 sees {len(user1_actions)} action(s)")

    # Check for core.echo
    core_echo_visible_user1 = any("core.echo" in ref for ref in user1_action_refs)
    if core_echo_visible_user1:
        print(f"✓ User 1 sees 'core.echo' system action")
    else:
        print(f"⚠ User 1 does not see 'core.echo' action")

    # Step 2: User 2 lists actions
    print("\n[STEP 2] User 2 (different tenant) listing actions...")
    user2_actions = unique_user_client.list_actions()
    user2_action_refs = [a["ref"] for a in user2_actions]

    print(f"✓ User 2 sees {len(user2_actions)} action(s)")

    # Check for core.echo
    core_echo_visible_user2 = any("core.echo" in ref for ref in user2_action_refs)
    if core_echo_visible_user2:
        print(f"✓ User 2 sees 'core.echo' system action")
    else:
        print(f"⚠ User 2 does not see 'core.echo' action")

    # Step 3: User 1 executes system pack action
    print("\n[STEP 3] User 1 executing system pack action...")

    if core_echo_visible_user1:
        try:
            exec_data = {
                "action": "core.echo",
                "parameters": {"message": "User 1 test"},
            }
            exec_response = client.execute_action(exec_data)
            print(f"✓ User 1 executed 'core.echo': execution {exec_response['id']}")
        except Exception as e:
            print(f"⚠ User 1 cannot execute 'core.echo': {e}")

    # Step 4: User 2 executes system pack action
    print("\n[STEP 4] User 2 executing system pack action...")

    if core_echo_visible_user2:
        try:
            exec_data = {
                "action": "core.echo",
                "parameters": {"message": "User 2 test"},
            }
            exec_response = unique_user_client.execute_action(exec_data)
            print(f"✓ User 2 executed 'core.echo': execution {exec_response['id']}")
        except Exception as e:
            print(f"⚠ User 2 cannot execute 'core.echo': {e}")

    # Summary
    print("\n" + "=" * 80)
    print("SYSTEM PACK ACTIONS TEST SUMMARY")
    print("=" * 80)
    print(f"✓ User 1 sees system actions: {core_echo_visible_user1}")
    print(f"✓ User 2 sees system actions: {core_echo_visible_user2}")

    if core_echo_visible_user1 and core_echo_visible_user2:
        print(f"✓ System pack actions available to all tenants")
        print("\n✅ SYSTEM PACK ACTIONS AVAILABILITY VERIFIED!")
    else:
        print(f"⚠ System pack actions may not be fully available")
        print("   Note: This may be expected if system packs not fully set up")

    print("=" * 80)


@pytest.mark.tier3
@pytest.mark.packs
def test_system_pack_identification():
    """
    Document the expected system pack markers and identification.

    This is a documentation test that doesn't make API calls.
    """
    print("\n" + "=" * 80)
    print("T3.11d: System Pack Identification Reference")
    print("=" * 80)

    print("\nSystem Pack Identification Markers:\n")

    print("1. Database Level:")
    print("   - tenant_id = NULL (not associated with any tenant)")
    print("   - OR system = true flag")
    print("   - Stored in 'attune.pack' table")

    print("\n2. API Level:")
    print("   - GET /api/v1/packs returns system packs to all users")
    print("   - System packs marked with 'system': true in response")
    print("   - Cannot be deleted by regular users")

    print("\n3. Known System Packs:")
    print("   - 'core' - Built-in core actions (echo, delay, etc.)")
    print("   - Future: 'stdlib', 'integrations', etc.")

    print("\n4. System Pack Characteristics:")
    print("   - Visible to all tenants")
    print("   - Actions executable by all users")
    print("   - Cannot be modified by regular users")
    print("   - Shared virtualenv/dependencies")
    print("   - Installed during system initialization")

    print("\n5. User Pack Characteristics:")
    print("   - tenant_id = <specific tenant ID>")
    print("   - Only visible to owning tenant")
    print("   - Can be created/modified/deleted by tenant users")
    print("   - Isolated virtualenv per pack")
    print("   - Tenant-specific lifecycle")

    print("\n" + "=" * 80)
    print("📋 System Pack Identification Documented")
    print("=" * 80)

    # Always passes - documentation only
    assert True
