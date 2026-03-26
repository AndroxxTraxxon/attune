#!/bin/bash
# Automated test script for Core Pack
# Tests all actions to ensure they work correctly

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACTIONS_DIR="$SCRIPT_DIR/actions"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Function to print test result
test_result() {
    TESTS_RUN=$((TESTS_RUN + 1))
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $1"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗${NC} $1"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Function to run a test
run_test() {
    local test_name="$1"
    shift
    echo -n "  Testing: $test_name... "
    if "$@" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    TESTS_RUN=$((TESTS_RUN + 1))
}

echo "========================================="
echo "Core Pack Test Suite"
echo "========================================="
echo ""

# Check if actions directory exists
if [ ! -d "$ACTIONS_DIR" ]; then
    echo -e "${RED}ERROR:${NC} Actions directory not found at $ACTIONS_DIR"
    exit 1
fi

# Check if scripts are executable
echo "→ Checking script permissions..."
for script in "$ACTIONS_DIR"/*.sh "$ACTIONS_DIR"/*.py; do
    if [ -f "$script" ] && [ ! -x "$script" ]; then
        echo -e "${YELLOW}WARNING:${NC} $script is not executable, fixing..."
        chmod +x "$script"
    fi
done
echo -e "${GREEN}✓${NC} All scripts have correct permissions"
echo ""

# Test core.echo
echo "→ Testing core.echo..."
export ATTUNE_ACTION_MESSAGE="Test message"
export ATTUNE_ACTION_UPPERCASE=false
run_test "basic echo" "$ACTIONS_DIR/echo.sh"

export ATTUNE_ACTION_MESSAGE="test uppercase"
export ATTUNE_ACTION_UPPERCASE=true
OUTPUT=$("$ACTIONS_DIR/echo.sh")
if [ "$OUTPUT" = "TEST UPPERCASE" ]; then
    echo -e "  Testing: uppercase conversion... ${GREEN}✓${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "  Testing: uppercase conversion... ${RED}✗${NC} (expected 'TEST UPPERCASE', got '$OUTPUT')"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_RUN=$((TESTS_RUN + 1))

unset ATTUNE_ACTION_MESSAGE ATTUNE_ACTION_UPPERCASE
echo ""

# Test core.sleep
echo "→ Testing core.sleep..."
export ATTUNE_ACTION_SECONDS=1
export ATTUNE_ACTION_MESSAGE="Sleeping..."
run_test "basic sleep (1 second)" "$ACTIONS_DIR/sleep.sh"

# Test invalid seconds
export ATTUNE_ACTION_SECONDS=-1
if "$ACTIONS_DIR/sleep.sh" > /dev/null 2>&1; then
    echo -e "  Testing: invalid seconds validation... ${RED}✗${NC} (should have failed)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
else
    echo -e "  Testing: invalid seconds validation... ${GREEN}✓${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_RUN=$((TESTS_RUN + 1))

unset ATTUNE_ACTION_SECONDS ATTUNE_ACTION_MESSAGE
echo ""

# Test core.noop
echo "→ Testing core.noop..."
export ATTUNE_ACTION_MESSAGE="Test noop"
export ATTUNE_ACTION_EXIT_CODE=0
run_test "basic noop with exit 0" "$ACTIONS_DIR/noop.sh"

export ATTUNE_ACTION_EXIT_CODE=1
if "$ACTIONS_DIR/noop.sh" > /dev/null 2>&1; then
    echo -e "  Testing: custom exit code (1)... ${RED}✗${NC} (should have exited with 1)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
else
    EXIT_CODE=$?
    if [ $EXIT_CODE -eq 1 ]; then
        echo -e "  Testing: custom exit code (1)... ${GREEN}✓${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "  Testing: custom exit code (1)... ${RED}✗${NC} (exit code was $EXIT_CODE, expected 1)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
fi
TESTS_RUN=$((TESTS_RUN + 1))

unset ATTUNE_ACTION_MESSAGE ATTUNE_ACTION_EXIT_CODE
echo ""

# Test core.http_request (requires Python and requests library)
echo "→ Testing core.http_request..."

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo -e "${YELLOW}WARNING:${NC} Python 3 not found, skipping HTTP request tests"
else
    # Check if requests library is installed
    if python3 -c "import requests" 2>/dev/null; then
        export ATTUNE_ACTION_URL="https://httpbin.org/get"
        export ATTUNE_ACTION_METHOD="GET"
        export ATTUNE_ACTION_TIMEOUT=10
        run_test "basic GET request" python3 "$ACTIONS_DIR/http_request.py"

        export ATTUNE_ACTION_URL="https://httpbin.org/post"
        export ATTUNE_ACTION_METHOD="POST"
        export ATTUNE_ACTION_JSON_BODY='{"test": "data"}'
        run_test "POST with JSON body" python3 "$ACTIONS_DIR/http_request.py"

        # Test missing required parameter
        unset ATTUNE_ACTION_URL
        if python3 "$ACTIONS_DIR/http_request.py" > /dev/null 2>&1; then
            echo -e "  Testing: missing URL validation... ${RED}✗${NC} (should have failed)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        else
            echo -e "  Testing: missing URL validation... ${GREEN}✓${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        fi
        TESTS_RUN=$((TESTS_RUN + 1))

        unset ATTUNE_ACTION_URL ATTUNE_ACTION_METHOD ATTUNE_ACTION_JSON_BODY ATTUNE_ACTION_TIMEOUT
    else
        echo -e "${YELLOW}WARNING:${NC} Python requests library not found, skipping HTTP tests"
        echo "  Install with: pip install requests>=2.28.0"
    fi
fi
echo ""

# Summary
echo "========================================="
echo "Test Results"
echo "========================================="
echo "Total tests run:    $TESTS_RUN"
echo -e "Tests passed:       ${GREEN}$TESTS_PASSED${NC}"
if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "Tests failed:       ${RED}$TESTS_FAILED${NC}"
else
    echo -e "Tests failed:       ${GREEN}$TESTS_FAILED${NC}"
fi
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
