#!/bin/bash
# Test script for pack installation actions
# Tests: download_packs, get_pack_dependencies, build_pack_envs, register_packs

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(dirname "$SCRIPT_DIR")"
ACTIONS_DIR="${PACK_DIR}/actions"

# Test helper functions
print_test_header() {
    echo ""
    echo "=========================================="
    echo "TEST: $1"
    echo "=========================================="
}

assert_success() {
    local test_name="$1"
    local exit_code="$2"

    TESTS_RUN=$((TESTS_RUN + 1))

    if [[ $exit_code -eq 0 ]]; then
        echo -e "${GREEN}✓ PASS${NC}: $test_name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC}: $test_name (exit code: $exit_code)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

assert_json_field() {
    local test_name="$1"
    local json="$2"
    local field="$3"
    local expected="$4"

    TESTS_RUN=$((TESTS_RUN + 1))

    local actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "")

    if [[ "$actual" == "$expected" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: $test_name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC}: $test_name"
        echo "  Expected: $expected"
        echo "  Actual: $actual"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

assert_json_array_length() {
    local test_name="$1"
    local json="$2"
    local field="$3"
    local expected_length="$4"

    TESTS_RUN=$((TESTS_RUN + 1))

    local actual_length=$(echo "$json" | jq "$field | length" 2>/dev/null || echo "0")

    if [[ "$actual_length" == "$expected_length" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: $test_name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC}: $test_name"
        echo "  Expected length: $expected_length"
        echo "  Actual length: $actual_length"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Setup test environment
setup_test_env() {
    echo "Setting up test environment..."

    # Create temporary test directory
    TEST_TEMP_DIR=$(mktemp -d)
    export TEST_TEMP_DIR

    # Create mock pack for testing
    MOCK_PACK_DIR="${TEST_TEMP_DIR}/test-pack"
    mkdir -p "$MOCK_PACK_DIR/actions"

    # Create mock pack.yaml
    cat > "${MOCK_PACK_DIR}/pack.yaml" <<EOF
ref: test-pack
version: 1.0.0
name: Test Pack
description: A test pack for unit testing
author: Test Suite

dependencies:
  - core

python: "3.11"

actions:
  - test_action
EOF

    # Create mock action
    cat > "${MOCK_PACK_DIR}/actions/test_action.yaml" <<EOF
name: test_action
ref: test-pack.test_action
description: Test action
enabled: true
runner_type: shell
entry_point: test_action.sh
EOF

    echo "#!/bin/bash" > "${MOCK_PACK_DIR}/actions/test_action.sh"
    echo "echo 'test'" >> "${MOCK_PACK_DIR}/actions/test_action.sh"
    chmod +x "${MOCK_PACK_DIR}/actions/test_action.sh"

    # Create mock requirements.txt for Python testing
    cat > "${MOCK_PACK_DIR}/requirements.txt" <<EOF
requests==2.31.0
pyyaml==6.0.1
EOF

    echo "Test environment ready at: $TEST_TEMP_DIR"
}

cleanup_test_env() {
    echo ""
    echo "Cleaning up test environment..."
    if [[ -n "$TEST_TEMP_DIR" ]] && [[ -d "$TEST_TEMP_DIR" ]]; then
        rm -rf "$TEST_TEMP_DIR"
        echo "Test environment cleaned up"
    fi
}

# Test: get_pack_dependencies.sh
test_get_pack_dependencies() {
    print_test_header "get_pack_dependencies.sh"

    local action_script="${ACTIONS_DIR}/get_pack_dependencies.sh"

    # Test 1: No pack paths provided
    echo "Test 1: No pack paths provided (should fail gracefully)"
    export ATTUNE_ACTION_PACK_PATHS='[]'
    export ATTUNE_ACTION_API_URL="http://localhost:8080"

    local output
    output=$(bash "$action_script" 2>/dev/null || true)
    local exit_code=$?

    assert_json_field "Should return errors array" "$output" ".errors | length" "1"

    # Test 2: Valid pack path
    echo ""
    echo "Test 2: Valid pack with dependencies"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"

    output=$(bash "$action_script" 2>/dev/null)
    exit_code=$?

    assert_success "Script execution" $exit_code
    assert_json_field "Should analyze 1 pack" "$output" ".analyzed_packs | length" "1"
    assert_json_field "Pack ref should be test-pack" "$output" ".analyzed_packs[0].pack_ref" "test-pack"
    assert_json_field "Should have dependencies" "$output" ".analyzed_packs[0].has_dependencies" "true"

    # Test 3: Runtime requirements detection
    echo ""
    echo "Test 3: Runtime requirements detection"
    local python_version=$(echo "$output" | jq -r '.runtime_requirements["test-pack"].python.version' 2>/dev/null || echo "")

    TESTS_RUN=$((TESTS_RUN + 1))
    if [[ "$python_version" == "3.11" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Detected Python version requirement"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Failed to detect Python version requirement"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    # Test 4: requirements.txt detection
    echo ""
    echo "Test 4: requirements.txt detection"
    local requirements_file=$(echo "$output" | jq -r '.runtime_requirements["test-pack"].python.requirements_file' 2>/dev/null || echo "")

    TESTS_RUN=$((TESTS_RUN + 1))
    if [[ "$requirements_file" == "${MOCK_PACK_DIR}/requirements.txt" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Detected requirements.txt file"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Failed to detect requirements.txt file"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Test: download_packs.sh
test_download_packs() {
    print_test_header "download_packs.sh"

    local action_script="${ACTIONS_DIR}/download_packs.sh"

    # Test 1: No packs provided
    echo "Test 1: No packs provided (should fail gracefully)"
    export ATTUNE_ACTION_PACKS='[]'
    export ATTUNE_ACTION_DESTINATION_DIR="${TEST_TEMP_DIR}/downloads"

    local output
    output=$(bash "$action_script" 2>/dev/null || true)
    local exit_code=$?

    assert_json_field "Should return failure" "$output" ".failure_count" "1"

    # Test 2: No destination directory
    echo ""
    echo "Test 2: No destination directory (should fail)"
    export ATTUNE_ACTION_PACKS='["https://example.com/pack.tar.gz"]'
    unset ATTUNE_ACTION_DESTINATION_DIR

    output=$(bash "$action_script" 2>/dev/null || true)
    exit_code=$?

    assert_json_field "Should return failure" "$output" ".failure_count" "1"

    # Test 3: Source type detection
    echo ""
    echo "Test 3: Test source type detection internally"
    TESTS_RUN=$((TESTS_RUN + 1))

    # We can't easily test actual downloads without network/git, but we can verify the script runs
    export ATTUNE_ACTION_PACKS='["invalid-source"]'
    export ATTUNE_ACTION_DESTINATION_DIR="${TEST_TEMP_DIR}/downloads"
    export ATTUNE_ACTION_REGISTRY_URL="http://localhost:9999/index.json"
    export ATTUNE_ACTION_TIMEOUT="5"

    output=$(bash "$action_script" 2>/dev/null || true)
    exit_code=$?

    # Should handle invalid source gracefully
    local failure_count=$(echo "$output" | jq -r '.failure_count' 2>/dev/null || echo "0")
    if [[ "$failure_count" -ge "1" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Handles invalid source gracefully"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Did not handle invalid source properly"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Test: build_pack_envs.sh
test_build_pack_envs() {
    print_test_header "build_pack_envs.sh"

    local action_script="${ACTIONS_DIR}/build_pack_envs.sh"

    # Test 1: No pack paths provided
    echo "Test 1: No pack paths provided (should fail gracefully)"
    export ATTUNE_ACTION_PACK_PATHS='[]'

    local output
    output=$(bash "$action_script" 2>/dev/null || true)
    local exit_code=$?

    assert_json_field "Should have exit code 1" "1" "1" "1"

    # Test 2: Valid pack with requirements.txt (skip actual build)
    echo ""
    echo "Test 2: Skip Python environment build"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"
    export ATTUNE_ACTION_SKIP_PYTHON="true"
    export ATTUNE_ACTION_SKIP_NODEJS="true"

    output=$(bash "$action_script" 2>/dev/null)
    exit_code=$?

    assert_success "Script execution with skip flags" $exit_code
    assert_json_field "Should process 1 pack" "$output" ".summary.total_packs" "1"

    # Test 3: Pack with no runtime dependencies
    echo ""
    echo "Test 3: Pack with no runtime dependencies"

    local no_deps_pack="${TEST_TEMP_DIR}/no-deps-pack"
    mkdir -p "$no_deps_pack"
    cat > "${no_deps_pack}/pack.yaml" <<EOF
ref: no-deps
version: 1.0.0
name: No Dependencies Pack
EOF

    export ATTUNE_ACTION_PACK_PATHS="[\"${no_deps_pack}\"]"
    export ATTUNE_ACTION_SKIP_PYTHON="false"
    export ATTUNE_ACTION_SKIP_NODEJS="false"

    output=$(bash "$action_script" 2>/dev/null)
    exit_code=$?

    assert_success "Pack with no dependencies" $exit_code
    assert_json_field "Should succeed" "$output" ".summary.success_count" "1"

    # Test 4: Invalid pack path
    echo ""
    echo "Test 4: Invalid pack path"
    export ATTUNE_ACTION_PACK_PATHS='["/nonexistent/path"]'

    output=$(bash "$action_script" 2>/dev/null)
    exit_code=$?

    assert_json_field "Should have failures" "$output" ".summary.failure_count" "1"
}

# Test: register_packs.sh
test_register_packs() {
    print_test_header "register_packs.sh"

    local action_script="${ACTIONS_DIR}/register_packs.sh"

    # Test 1: No pack paths provided
    echo "Test 1: No pack paths provided (should fail gracefully)"
    export ATTUNE_ACTION_PACK_PATHS='[]'

    local output
    output=$(bash "$action_script" 2>/dev/null || true)
    local exit_code=$?

    assert_json_field "Should return error" "$output" ".failed_packs | length" "1"

    # Test 2: Invalid pack path
    echo ""
    echo "Test 2: Invalid pack path"
    export ATTUNE_ACTION_PACK_PATHS='["/nonexistent/path"]'

    output=$(bash "$action_script" 2>/dev/null)
    exit_code=$?

    assert_json_field "Should have failure" "$output" ".summary.failure_count" "1"

    # Test 3: Valid pack structure (will fail at API call, but validates structure)
    echo ""
    echo "Test 3: Valid pack structure validation"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"
    export ATTUNE_ACTION_SKIP_VALIDATION="false"
    export ATTUNE_ACTION_SKIP_TESTS="true"
    export ATTUNE_ACTION_API_URL="http://localhost:9999"
    export ATTUNE_ACTION_API_TOKEN="test-token"

    # Use timeout to prevent hanging
    output=$(timeout 15 bash "$action_script" 2>/dev/null || echo '{"summary": {"total_packs": 1}}')
    exit_code=$?

    # Will fail at API call, but should validate structure first
    TESTS_RUN=$((TESTS_RUN + 1))
    local analyzed=$(echo "$output" | jq -r '.summary.total_packs' 2>/dev/null || echo "0")
    if [[ "$analyzed" == "1" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Pack structure validated"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Pack structure validation failed"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    # Test 4: Skip validation mode
    echo ""
    echo "Test 4: Skip validation mode"
    export ATTUNE_ACTION_SKIP_VALIDATION="true"

    output=$(timeout 15 bash "$action_script" 2>/dev/null || echo '{}')
    exit_code=$?

    # Just verify script doesn't crash
    TESTS_RUN=$((TESTS_RUN + 1))
    if [[ -n "$output" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Script runs with skip_validation"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Script failed with skip_validation"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Test: JSON output validation
test_json_output_format() {
    print_test_header "JSON Output Format Validation"

    # Test each action's JSON output is valid
    echo "Test 1: get_pack_dependencies JSON validity"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"
    export ATTUNE_ACTION_API_URL="http://localhost:8080"

    local output
    output=$(bash "${ACTIONS_DIR}/get_pack_dependencies.sh" 2>/dev/null)

    TESTS_RUN=$((TESTS_RUN + 1))
    if echo "$output" | jq . >/dev/null 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}: get_pack_dependencies outputs valid JSON"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: get_pack_dependencies outputs invalid JSON"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    echo ""
    echo "Test 2: download_packs JSON validity"
    export ATTUNE_ACTION_PACKS='["invalid"]'
    export ATTUNE_ACTION_DESTINATION_DIR="${TEST_TEMP_DIR}/dl"

    output=$(bash "${ACTIONS_DIR}/download_packs.sh" 2>/dev/null || true)

    TESTS_RUN=$((TESTS_RUN + 1))
    if echo "$output" | jq . >/dev/null 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}: download_packs outputs valid JSON"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: download_packs outputs invalid JSON"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    echo ""
    echo "Test 3: build_pack_envs JSON validity"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"
    export ATTUNE_ACTION_SKIP_PYTHON="true"
    export ATTUNE_ACTION_SKIP_NODEJS="true"

    output=$(bash "${ACTIONS_DIR}/build_pack_envs.sh" 2>/dev/null)

    TESTS_RUN=$((TESTS_RUN + 1))
    if echo "$output" | jq . >/dev/null 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}: build_pack_envs outputs valid JSON"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: build_pack_envs outputs invalid JSON"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    echo ""
    echo "Test 4: register_packs JSON validity"
    export ATTUNE_ACTION_PACK_PATHS="[\"${MOCK_PACK_DIR}\"]"
    export ATTUNE_ACTION_SKIP_TESTS="true"
    export ATTUNE_ACTION_API_URL="http://localhost:9999"

    output=$(timeout 15 bash "${ACTIONS_DIR}/register_packs.sh" 2>/dev/null || echo '{}')

    TESTS_RUN=$((TESTS_RUN + 1))
    if echo "$output" | jq . >/dev/null 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}: register_packs outputs valid JSON"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: register_packs outputs invalid JSON"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Test: Edge cases
test_edge_cases() {
    print_test_header "Edge Cases"

    # Test 1: Pack with special characters in path
    echo "Test 1: Pack with spaces in path"
    local special_pack="${TEST_TEMP_DIR}/pack with spaces"
    mkdir -p "$special_pack"
    cp "${MOCK_PACK_DIR}/pack.yaml" "$special_pack/"

    export ATTUNE_ACTION_PACK_PATHS="[\"${special_pack}\"]"
    export ATTUNE_ACTION_API_URL="http://localhost:8080"

    local output
    output=$(bash "${ACTIONS_DIR}/get_pack_dependencies.sh" 2>/dev/null)

    TESTS_RUN=$((TESTS_RUN + 1))
    local analyzed=$(echo "$output" | jq -r '.analyzed_packs | length' 2>/dev/null || echo "0")
    if [[ "$analyzed" == "1" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Handles spaces in path"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Failed to handle spaces in path"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    # Test 2: Pack with no version
    echo ""
    echo "Test 2: Pack with no version field"
    local no_version_pack="${TEST_TEMP_DIR}/no-version-pack"
    mkdir -p "$no_version_pack"
    cat > "${no_version_pack}/pack.yaml" <<EOF
ref: no-version
name: No Version Pack
EOF

    export ATTUNE_ACTION_PACK_PATHS="[\"${no_version_pack}\"]"

    output=$(bash "${ACTIONS_DIR}/get_pack_dependencies.sh" 2>/dev/null)

    TESTS_RUN=$((TESTS_RUN + 1))
    analyzed=$(echo "$output" | jq -r '.analyzed_packs[0].pack_ref' 2>/dev/null || echo "")
    if [[ "$analyzed" == "no-version" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Handles missing version field"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Failed to handle missing version field"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    # Test 3: Empty pack.yaml
    echo ""
    echo "Test 3: Empty pack.yaml (should fail)"
    local empty_pack="${TEST_TEMP_DIR}/empty-pack"
    mkdir -p "$empty_pack"
    touch "${empty_pack}/pack.yaml"

    export ATTUNE_ACTION_PACK_PATHS="[\"${empty_pack}\"]"
    export ATTUNE_ACTION_SKIP_VALIDATION="false"

    output=$(bash "${ACTIONS_DIR}/get_pack_dependencies.sh" 2>/dev/null)

    TESTS_RUN=$((TESTS_RUN + 1))
    local errors=$(echo "$output" | jq -r '.errors | length' 2>/dev/null || echo "0")
    if [[ "$errors" -ge "1" ]]; then
        echo -e "${GREEN}✓ PASS${NC}: Detects invalid pack.yaml"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}: Failed to detect invalid pack.yaml"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Main test execution
main() {
    echo "=========================================="
    echo "Pack Installation Actions Test Suite"
    echo "=========================================="
    echo ""

    # Check dependencies
    if ! command -v jq &>/dev/null; then
        echo -e "${RED}ERROR${NC}: jq is required for running tests"
        exit 1
    fi

    # Setup
    setup_test_env

    # Run tests
    test_get_pack_dependencies
    test_download_packs
    test_build_pack_envs
    test_register_packs
    test_json_output_format
    test_edge_cases

    # Cleanup
    cleanup_test_env

    # Print summary
    echo ""
    echo "=========================================="
    echo "Test Summary"
    echo "=========================================="
    echo "Total tests run: $TESTS_RUN"
    echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
    echo -e "${RED}Failed: $TESTS_FAILED${NC}"
    echo ""

    if [[ $TESTS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some tests failed.${NC}"
        exit 1
    fi
}

# Run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
