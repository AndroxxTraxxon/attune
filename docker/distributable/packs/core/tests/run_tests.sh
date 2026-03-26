#!/bin/bash
# Core Pack Unit Test Runner
# Runs all unit tests for core pack actions and reports results

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(dirname "$SCRIPT_DIR")"
ACTIONS_DIR="$PACK_DIR/actions"

# Test results array
declare -a FAILED_TEST_NAMES

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Core Pack Unit Tests${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    echo -n "  [$TOTAL_TESTS] $test_name ... "

    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        FAILED_TEST_NAMES+=("$test_name")
        return 1
    fi
}

# Function to run a test expecting failure
run_test_expect_fail() {
    local test_name="$1"
    local test_command="$2"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    echo -n "  [$TOTAL_TESTS] $test_name ... "

    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${RED}FAIL${NC} (expected failure but passed)"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        FAILED_TEST_NAMES+=("$test_name")
        return 1
    else
        echo -e "${GREEN}PASS${NC} (failed as expected)"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    fi
}

# Function to check output contains text
check_output() {
    local test_name="$1"
    local command="$2"
    local expected="$3"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    echo -n "  [$TOTAL_TESTS] $test_name ... "

    local output=$(eval "$command" 2>&1)

    if echo "$output" | grep -q "$expected"; then
        echo -e "${GREEN}PASS${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "    Expected output to contain: '$expected'"
        echo "    Got: '$output'"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        FAILED_TEST_NAMES+=("$test_name")
        return 1
    fi
}

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if [ ! -f "$ACTIONS_DIR/echo.sh" ]; then
    echo -e "${RED}ERROR: Actions directory not found at $ACTIONS_DIR${NC}"
    exit 1
fi

# Check Python for http_request tests
if ! command -v python3 &> /dev/null; then
    echo -e "${YELLOW}WARNING: python3 not found, skipping Python tests${NC}"
    SKIP_PYTHON=true
else
    echo "  ✓ python3 found"
fi

# Check Python requests library
if [ "$SKIP_PYTHON" != "true" ]; then
    if ! python3 -c "import requests" 2>/dev/null; then
        echo -e "${YELLOW}WARNING: requests library not installed, skipping HTTP tests${NC}"
        SKIP_HTTP=true
    else
        echo "  ✓ requests library found"
    fi
fi

echo ""

# ========================================
# Test: core.echo
# ========================================
echo -e "${BLUE}Testing core.echo${NC}"

# Test 1: Basic echo
check_output \
    "echo: basic message" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_MESSAGE='Hello, Attune!' ./echo.sh" \
    "Hello, Attune!"

# Test 2: Default message
check_output \
    "echo: default message" \
    "cd '$ACTIONS_DIR' && unset ATTUNE_ACTION_MESSAGE && ./echo.sh" \
    "Hello, World!"

# Test 3: Uppercase conversion
check_output \
    "echo: uppercase conversion" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_MESSAGE='test message' ATTUNE_ACTION_UPPERCASE=true ./echo.sh" \
    "TEST MESSAGE"

# Test 4: Uppercase false
check_output \
    "echo: uppercase false" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_MESSAGE='Mixed Case' ATTUNE_ACTION_UPPERCASE=false ./echo.sh" \
    "Mixed Case"

# Test 5: Exit code success
run_test \
    "echo: exit code 0" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_MESSAGE='test' ./echo.sh && [ \$? -eq 0 ]"

echo ""

# ========================================
# Test: core.noop
# ========================================
echo -e "${BLUE}Testing core.noop${NC}"

# Test 1: Basic noop
check_output \
    "noop: basic execution" \
    "cd '$ACTIONS_DIR' && ./noop.sh" \
    "No operation completed successfully"

# Test 2: With message
check_output \
    "noop: with message" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_MESSAGE='Test noop' ./noop.sh" \
    "Test noop"

# Test 3: Exit code 0
run_test \
    "noop: exit code 0" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_EXIT_CODE=0 ./noop.sh && [ \$? -eq 0 ]"

# Test 4: Custom exit code
run_test \
    "noop: custom exit code 5" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_EXIT_CODE=5 ./noop.sh; [ \$? -eq 5 ]"

# Test 5: Invalid exit code (negative)
run_test_expect_fail \
    "noop: invalid negative exit code" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_EXIT_CODE=-1 ./noop.sh"

# Test 6: Invalid exit code (too large)
run_test_expect_fail \
    "noop: invalid large exit code" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_EXIT_CODE=999 ./noop.sh"

# Test 7: Invalid exit code (non-numeric)
run_test_expect_fail \
    "noop: invalid non-numeric exit code" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_EXIT_CODE=abc ./noop.sh"

echo ""

# ========================================
# Test: core.sleep
# ========================================
echo -e "${BLUE}Testing core.sleep${NC}"

# Test 1: Basic sleep
check_output \
    "sleep: basic execution (1s)" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=1 ./sleep.sh" \
    "Slept for 1 seconds"

# Test 2: Zero seconds
check_output \
    "sleep: zero seconds" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=0 ./sleep.sh" \
    "Slept for 0 seconds"

# Test 3: With message
check_output \
    "sleep: with message" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=1 ATTUNE_ACTION_MESSAGE='Sleeping now...' ./sleep.sh" \
    "Sleeping now..."

# Test 4: Verify timing (should take at least 2 seconds)
run_test \
    "sleep: timing verification (2s)" \
    "cd '$ACTIONS_DIR' && start=\$(date +%s) && ATTUNE_ACTION_SECONDS=2 ./sleep.sh > /dev/null && end=\$(date +%s) && [ \$((end - start)) -ge 2 ]"

# Test 5: Invalid negative seconds
run_test_expect_fail \
    "sleep: invalid negative seconds" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=-1 ./sleep.sh"

# Test 6: Invalid too large seconds
run_test_expect_fail \
    "sleep: invalid large seconds (>3600)" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=9999 ./sleep.sh"

# Test 7: Invalid non-numeric seconds
run_test_expect_fail \
    "sleep: invalid non-numeric seconds" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_SECONDS=abc ./sleep.sh"

# Test 8: Default value
check_output \
    "sleep: default value (1s)" \
    "cd '$ACTIONS_DIR' && unset ATTUNE_ACTION_SECONDS && ./sleep.sh" \
    "Slept for 1 seconds"

echo ""

# ========================================
# Test: core.http_request
# ========================================
if [ "$SKIP_HTTP" != "true" ]; then
    echo -e "${BLUE}Testing core.http_request${NC}"

    # Test 1: Simple GET request
    run_test \
        "http_request: GET request" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/get' ATTUNE_ACTION_METHOD='GET' python3 ./http_request.py | grep -q '\"success\": true'"

    # Test 2: Missing required URL
    run_test_expect_fail \
        "http_request: missing URL parameter" \
        "cd '$ACTIONS_DIR' && unset ATTUNE_ACTION_URL && python3 ./http_request.py"

    # Test 3: POST with JSON body
    run_test \
        "http_request: POST with JSON" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/post' ATTUNE_ACTION_METHOD='POST' ATTUNE_ACTION_JSON_BODY='{\"test\": \"value\"}' python3 ./http_request.py | grep -q '\"success\": true'"

    # Test 4: Custom headers
    run_test \
        "http_request: custom headers" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/headers' ATTUNE_ACTION_METHOD='GET' ATTUNE_ACTION_HEADERS='{\"X-Custom-Header\": \"test\"}' python3 ./http_request.py | grep -q 'X-Custom-Header'"

    # Test 5: Query parameters
    run_test \
        "http_request: query parameters" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/get' ATTUNE_ACTION_METHOD='GET' ATTUNE_ACTION_QUERY_PARAMS='{\"foo\": \"bar\", \"page\": \"1\"}' python3 ./http_request.py | grep -q '\"foo\": \"bar\"'"

    # Test 6: Timeout (expect failure/timeout)
    run_test \
        "http_request: timeout handling" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/delay/10' ATTUNE_ACTION_METHOD='GET' ATTUNE_ACTION_TIMEOUT=2 python3 ./http_request.py; [ \$? -ne 0 ]"

    # Test 7: 404 Not Found
    run_test \
        "http_request: 404 handling" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/status/404' ATTUNE_ACTION_METHOD='GET' python3 ./http_request.py | grep -q '\"status_code\": 404'"

    # Test 8: Different methods (PUT, PATCH, DELETE)
    for method in PUT PATCH DELETE; do
        run_test \
            "http_request: $method method" \
            "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/${method,,}' ATTUNE_ACTION_METHOD='$method' python3 ./http_request.py | grep -q '\"success\": true'"
    done

    # Test 9: HEAD method (no body expected)
    run_test \
        "http_request: HEAD method" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/get' ATTUNE_ACTION_METHOD='HEAD' python3 ./http_request.py | grep -q '\"status_code\": 200'"

    # Test 10: OPTIONS method
    run_test \
        "http_request: OPTIONS method" \
        "cd '$ACTIONS_DIR' && ATTUNE_ACTION_URL='https://httpbin.org/get' ATTUNE_ACTION_METHOD='OPTIONS' python3 ./http_request.py | grep -q '\"status_code\"'"

    echo ""
else
    echo -e "${YELLOW}Skipping core.http_request tests (Python/requests not available)${NC}"
    echo ""
fi

# ========================================
# Test: File Permissions
# ========================================
echo -e "${BLUE}Testing file permissions${NC}"

run_test \
    "permissions: echo.sh is executable" \
    "[ -x '$ACTIONS_DIR/echo.sh' ]"

run_test \
    "permissions: noop.sh is executable" \
    "[ -x '$ACTIONS_DIR/noop.sh' ]"

run_test \
    "permissions: sleep.sh is executable" \
    "[ -x '$ACTIONS_DIR/sleep.sh' ]"

if [ "$SKIP_PYTHON" != "true" ]; then
    run_test \
        "permissions: http_request.py is executable" \
        "[ -x '$ACTIONS_DIR/http_request.py' ]"
fi

echo ""

# ========================================
# Test: YAML Schema Validation
# ========================================
echo -e "${BLUE}Testing YAML schemas${NC}"

# Check if PyYAML is installed
if python3 -c "import yaml" 2>/dev/null; then
    # Check YAML files are valid
    for yaml_file in "$PACK_DIR"/*.yaml "$PACK_DIR"/actions/*.yaml "$PACK_DIR"/triggers/*.yaml; do
        if [ -f "$yaml_file" ]; then
            filename=$(basename "$yaml_file")
            run_test \
                "yaml: $filename is valid" \
                "python3 -c 'import yaml; yaml.safe_load(open(\"$yaml_file\"))'"
        fi
    done
else
    echo -e "  ${YELLOW}Skipping YAML validation tests (PyYAML not installed)${NC}"
    echo -e "  ${YELLOW}Install with: pip install pyyaml${NC}"
fi

echo ""

# ========================================
# Results Summary
# ========================================
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Results${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Total Tests:  $TOTAL_TESTS"
echo -e "Passed:       ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed:       ${RED}$FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed:${NC}"
    for test_name in "${FAILED_TEST_NAMES[@]}"; do
        echo -e "  ${RED}✗${NC} $test_name"
    done
    echo ""
    exit 1
fi
