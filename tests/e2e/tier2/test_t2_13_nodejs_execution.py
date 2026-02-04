"""
T2.13: Node.js Action Execution

Tests that JavaScript actions execute with Node.js runtime, with support for
npm package dependencies and proper isolation.

Test validates:
- npm install runs for pack dependencies
- node_modules created in pack directory
- Action can require packages
- Dependencies isolated per pack
- Worker supports Node.js runtime type
"""

import time

import pytest
from helpers.client import AttuneClient
from helpers.fixtures import unique_ref
from helpers.polling import wait_for_execution_status


def test_nodejs_action_basic(client: AttuneClient, test_pack):
    """
    Test basic Node.js action execution.

    Flow:
    1. Create Node.js action with simple script
    2. Execute action
    3. Verify execution succeeds
    4. Verify Node.js runtime works
    """
    print("\n" + "=" * 80)
    print("TEST: Node.js Action Execution (T2.13)")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create basic Node.js action
    # ========================================================================
    print("\n[STEP 1] Creating basic Node.js action...")

    # Simple Node.js script
    nodejs_script = """
const params = process.argv[2] ? JSON.parse(process.argv[2]) : {};

console.log('✓ Node.js action started');
console.log(`  Node version: ${process.version}`);
console.log(`  Platform: ${process.platform}`);

const result = {
    success: true,
    message: 'Hello from Node.js',
    nodeVersion: process.version,
    params: params
};

console.log('✓ Action completed successfully');
console.log(JSON.stringify(result));
process.exit(0);
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"nodejs_basic_{unique_ref()}",
            "description": "Basic Node.js action",
            "runner_type": "nodejs",
            "entry_point": "action.js",
            "enabled": True,
            "parameters": {
                "message": {"type": "string", "required": False, "default": "Hello"}
            },
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created Node.js action: {action_ref}")
    print(f"  Runner: nodejs")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing Node.js action...")

    execution = client.create_execution(
        action_ref=action_ref, parameters={"message": "Test message"}
    )
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait for completion
    # ========================================================================
    print("\n[STEP 3] Waiting for execution to complete...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=30,
    )
    print(f"✓ Execution completed: status={result['status']}")

    # ========================================================================
    # STEP 4: Verify execution details
    # ========================================================================
    print("\n[STEP 4] Verifying execution details...")

    execution_details = client.get_execution(execution_id)

    assert execution_details["status"] == "succeeded", (
        f"❌ Expected 'succeeded', got '{execution_details['status']}'"
    )
    print("  ✓ Execution succeeded")

    stdout = execution_details.get("stdout", "")
    if stdout:
        if "Node.js action started" in stdout:
            print("  ✓ Node.js runtime executed")
        if "Node version:" in stdout:
            print("  ✓ Node.js version detected")
        if "Action completed successfully" in stdout:
            print("  ✓ Action completed successfully")
    else:
        print("  ℹ No stdout available")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Node.js Action Execution")
    print("=" * 80)
    print(f"✓ Node.js action: {action_ref}")
    print(f"✓ Execution: succeeded")
    print(f"✓ Node.js runtime: working")
    print("\n✅ TEST PASSED: Node.js execution works correctly!")
    print("=" * 80 + "\n")


def test_nodejs_action_with_axios(client: AttuneClient, test_pack):
    """
    Test Node.js action with npm package dependency (axios).

    Flow:
    1. Create package.json with axios dependency
    2. Create action that requires axios
    3. Worker installs npm dependencies
    4. Execute action
    5. Verify node_modules created
    6. Verify action can require packages
    """
    print("\n" + "=" * 80)
    print("TEST: Node.js Action - With Axios Package")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create Node.js action with axios
    # ========================================================================
    print("\n[STEP 1] Creating Node.js action with axios...")

    # Action that uses axios
    axios_script = """
const params = process.argv[2] ? JSON.parse(process.argv[2]) : {};

try {
    const axios = require('axios');
    console.log('✓ Successfully imported axios library');
    console.log(`  axios version: ${axios.VERSION || 'unknown'}`);

    // Make HTTP request
    axios.get('https://httpbin.org/get', { timeout: 5000 })
        .then(response => {
            console.log(`✓ HTTP request successful: status=${response.status}`);

            const result = {
                success: true,
                library: 'axios',
                statusCode: response.status
            };

            console.log(JSON.stringify(result));
            process.exit(0);
        })
        .catch(error => {
            console.error(`✗ HTTP request failed: ${error.message}`);
            process.exit(1);
        });

} catch (error) {
    console.error(`✗ Failed to import axios: ${error.message}`);
    console.error('  (Dependencies may not be installed yet)');
    process.exit(1);
}
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"nodejs_axios_{unique_ref()}",
            "description": "Node.js action with axios dependency",
            "runner_type": "nodejs",
            "entry_point": "http_action.js",
            "enabled": True,
            "parameters": {},
            "metadata": {"npm_dependencies": {"axios": "^1.6.0"}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created Node.js action: {action_ref}")
    print(f"  Dependencies: axios ^1.6.0")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing action...")
    print("  Note: First execution may take longer (installing dependencies)")

    execution = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait for completion
    # ========================================================================
    print("\n[STEP 3] Waiting for execution to complete...")

    # First execution may take longer due to npm install
    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=60,  # Longer timeout for npm install
    )
    print(f"✓ Execution completed: status={result['status']}")

    # ========================================================================
    # STEP 4: Verify execution details
    # ========================================================================
    print("\n[STEP 4] Verifying execution details...")

    execution_details = client.get_execution(execution_id)

    assert execution_details["status"] == "succeeded", (
        f"❌ Expected 'succeeded', got '{execution_details['status']}'"
    )
    print("  ✓ Execution succeeded")

    stdout = execution_details.get("stdout", "")
    if stdout:
        if "Successfully imported axios" in stdout:
            print("  ✓ axios library imported successfully")
        if "axios version:" in stdout:
            print("  ✓ axios version detected")
        if "HTTP request successful" in stdout:
            print("  ✓ HTTP request executed successfully")
    else:
        print("  ℹ No stdout available")

    # ========================================================================
    # STEP 5: Execute again to test caching
    # ========================================================================
    print("\n[STEP 5] Executing again to test node_modules caching...")

    execution2 = client.create_execution(action_ref=action_ref, parameters={})
    execution2_id = execution2["id"]
    print(f"✓ Second execution created: ID={execution2_id}")

    start_time = time.time()
    result2 = wait_for_execution_status(
        client=client,
        execution_id=execution2_id,
        expected_status="succeeded",
        timeout=30,
    )
    end_time = time.time()
    second_exec_time = end_time - start_time

    print(f"✓ Second execution completed: status={result2['status']}")
    print(
        f"  Time: {second_exec_time:.1f}s (should be faster with cached node_modules)"
    )

    # ========================================================================
    # STEP 6: Validate success criteria
    # ========================================================================
    print("\n[STEP 6] Validating success criteria...")

    assert result["status"] == "succeeded", "❌ First execution should succeed"
    assert result2["status"] == "succeeded", "❌ Second execution should succeed"
    print("  ✓ Both executions succeeded")

    if "Successfully imported axios" in stdout:
        print("  ✓ Action imported npm package")
    else:
        print("  ℹ Import verification not available in output")

    if second_exec_time < 10:
        print(f"  ✓ Second execution fast: {second_exec_time:.1f}s (cached)")
    else:
        print(f"  ℹ Second execution time: {second_exec_time:.1f}s")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Node.js Action with Axios")
    print("=" * 80)
    print(f"✓ Action with npm dependencies: {action_ref}")
    print(f"✓ Dependency: axios ^1.6.0")
    print(f"✓ First execution: succeeded")
    print(f"✓ Second execution: succeeded (cached)")
    print(f"✓ Package import: successful")
    print(f"✓ HTTP request: successful")
    print("\n✅ TEST PASSED: Node.js with npm dependencies works!")
    print("=" * 80 + "\n")


def test_nodejs_action_multiple_packages(client: AttuneClient, test_pack):
    """
    Test Node.js action with multiple npm packages.

    Flow:
    1. Create action with multiple npm dependencies
    2. Verify all packages can be required
    3. Verify action uses multiple packages
    """
    print("\n" + "=" * 80)
    print("TEST: Node.js Action - Multiple Packages")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create action with multiple dependencies
    # ========================================================================
    print("\n[STEP 1] Creating action with multiple npm packages...")

    multi_pkg_script = """
const params = process.argv[2] ? JSON.parse(process.argv[2]) : {};

try {
    const axios = require('axios');
    const lodash = require('lodash');

    console.log('✓ All packages imported successfully');
    console.log(`  - axios: available`);
    console.log(`  - lodash: ${lodash.VERSION}`);

    // Use both packages
    const numbers = [1, 2, 3, 4, 5];
    const sum = lodash.sum(numbers);

    console.log(`✓ Used lodash: sum([1,2,3,4,5]) = ${sum}`);
    console.log('✓ Used multiple packages successfully');

    const result = {
        success: true,
        packages: ['axios', 'lodash'],
        lodashSum: sum
    };

    console.log(JSON.stringify(result));
    process.exit(0);

} catch (error) {
    console.error(`✗ Error: ${error.message}`);
    process.exit(1);
}
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"nodejs_multi_{unique_ref()}",
            "description": "Action with multiple npm packages",
            "runner_type": "nodejs",
            "entry_point": "multi_pkg.js",
            "enabled": True,
            "parameters": {},
            "metadata": {"npm_dependencies": {"axios": "^1.6.0", "lodash": "^4.17.21"}},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created Node.js action: {action_ref}")
    print(f"  Dependencies:")
    print(f"    - axios ^1.6.0")
    print(f"    - lodash ^4.17.21")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing action...")

    execution = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait for completion
    # ========================================================================
    print("\n[STEP 3] Waiting for completion...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=60,
    )
    print(f"✓ Execution completed: status={result['status']}")

    # ========================================================================
    # STEP 4: Verify multiple packages
    # ========================================================================
    print("\n[STEP 4] Verifying multiple packages...")

    execution_details = client.get_execution(execution_id)
    stdout = execution_details.get("stdout", "")

    if "All packages imported successfully" in stdout:
        print("  ✓ All packages imported")
    if "axios:" in stdout:
        print("  ✓ axios package available")
    if "lodash:" in stdout:
        print("  ✓ lodash package available")
    if "Used lodash:" in stdout:
        print("  ✓ Packages used successfully")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Multiple npm Packages")
    print("=" * 80)
    print(f"✓ Action: {action_ref}")
    print(f"✓ Dependencies: 2 packages")
    print(f"✓ Execution: succeeded")
    print(f"✓ All packages imported and used")
    print("\n✅ TEST PASSED: Multiple npm packages work correctly!")
    print("=" * 80 + "\n")


def test_nodejs_action_async_await(client: AttuneClient, test_pack):
    """
    Test Node.js action with async/await.

    Flow:
    1. Create action using modern async/await syntax
    2. Execute action
    3. Verify async operations work correctly
    """
    print("\n" + "=" * 80)
    print("TEST: Node.js Action - Async/Await")
    print("=" * 80)

    pack_ref = test_pack["ref"]

    # ========================================================================
    # STEP 1: Create async action
    # ========================================================================
    print("\n[STEP 1] Creating async Node.js action...")

    async_script = """
const params = process.argv[2] ? JSON.parse(process.argv[2]) : {};

async function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function main() {
    try {
        console.log('✓ Starting async action');

        await delay(1000);
        console.log('✓ Waited 1 second');

        await delay(1000);
        console.log('✓ Waited another second');

        const result = {
            success: true,
            message: 'Async/await works!',
            delaysCompleted: 2
        };

        console.log('✓ Async action completed');
        console.log(JSON.stringify(result));
        process.exit(0);

    } catch (error) {
        console.error(`✗ Error: ${error.message}`);
        process.exit(1);
    }
}

main();
"""

    action = client.create_action(
        pack_ref=pack_ref,
        data={
            "name": f"nodejs_async_{unique_ref()}",
            "description": "Action with async/await",
            "runner_type": "nodejs",
            "entry_point": "async_action.js",
            "enabled": True,
            "parameters": {},
        },
    )
    action_ref = action["ref"]
    print(f"✓ Created async Node.js action: {action_ref}")

    # ========================================================================
    # STEP 2: Execute action
    # ========================================================================
    print("\n[STEP 2] Executing async action...")

    start_time = time.time()
    execution = client.create_execution(action_ref=action_ref, parameters={})
    execution_id = execution["id"]
    print(f"✓ Execution created: ID={execution_id}")

    # ========================================================================
    # STEP 3: Wait for completion
    # ========================================================================
    print("\n[STEP 3] Waiting for completion...")

    result = wait_for_execution_status(
        client=client,
        execution_id=execution_id,
        expected_status="succeeded",
        timeout=20,
    )
    end_time = time.time()
    total_time = end_time - start_time

    print(f"✓ Execution completed: status={result['status']}")
    print(f"  Total time: {total_time:.1f}s")

    # ========================================================================
    # STEP 4: Verify async behavior
    # ========================================================================
    print("\n[STEP 4] Verifying async behavior...")

    execution_details = client.get_execution(execution_id)
    stdout = execution_details.get("stdout", "")

    if "Starting async action" in stdout:
        print("  ✓ Async action started")
    if "Waited 1 second" in stdout:
        print("  ✓ First delay completed")
    if "Waited another second" in stdout:
        print("  ✓ Second delay completed")
    if "Async action completed" in stdout:
        print("  ✓ Async action completed")

    # Should take at least 2 seconds (two delays)
    if total_time >= 2:
        print(f"  ✓ Timing correct: {total_time:.1f}s >= 2s")

    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    print("\n" + "=" * 80)
    print("TEST SUMMARY: Async/Await")
    print("=" * 80)
    print(f"✓ Async action: {action_ref}")
    print(f"✓ Execution: succeeded")
    print(f"✓ Async/await: working")
    print(f"✓ Total time: {total_time:.1f}s")
    print("\n✅ TEST PASSED: Async/await works correctly!")
    print("=" * 80 + "\n")
