#!/usr/bin/env bash
#
# End-to-End Integration Test Runner
#
# This script runs the E2E integration tests for Attune.
# It assumes all services are already running.
#
# Usage:
#   ./tests/run_e2e_tests.sh [options]
#
# Options:
#   -v, --verbose       Verbose test output
#   -s, --stop-on-fail  Stop on first test failure
#   -k EXPRESSION       Only run tests matching expression
#   -m MARKER           Only run tests with given marker
#   --tier TIER         Run specific tier (1, 2, or 3)
#   --setup             Set up test environment (install deps, create venv)
#   --teardown          Clean up after tests
#   --coverage          Run with coverage reporting
#   -h, --help          Show this help message
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$SCRIPT_DIR/venvs/e2e"

# Default values
VERBOSE=""
STOP_ON_FAIL=""
TEST_PATTERN=""
MARKER=""
TIER=""
COVERAGE=""
SETUP=false
TEARDOWN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE="-v -s"
            shift
            ;;
        -s|--stop-on-fail)
            STOP_ON_FAIL="-x"
            shift
            ;;
        -k)
            TEST_PATTERN="-k $2"
            shift 2
            ;;
        -m)
            MARKER="-m $2"
            shift 2
            ;;
        --tier)
            TIER="$2"
            shift 2
            ;;
        --coverage)
            COVERAGE="--cov=../crates --cov-report=html --cov-report=term"
            shift
            ;;
        --setup)
            SETUP=true
            shift
            ;;
        --teardown)
            TEARDOWN=true
            shift
            ;;
        -h|--help)
            head -n 22 "$0" | tail -n +3 | sed 's/^# //'
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            exit 1
            ;;
    esac
done

# Functions
log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1"
}

log_header() {
    echo -e "${CYAN}═══${NC} $1"
}

setup_environment() {
    log_info "Setting up E2E test environment..."

    # Create virtual environment if it doesn't exist
    if [[ ! -d "$VENV_DIR" ]]; then
        log_info "Creating virtual environment at $VENV_DIR"
        python3 -m venv "$VENV_DIR"
    fi

    # Activate virtual environment
    source "$VENV_DIR/bin/activate"

    # Install dependencies
    log_info "Installing test dependencies..."
    pip install -q --upgrade pip
    pip install -q -r "$SCRIPT_DIR/requirements.txt"

    log_success "Test environment ready"
}

check_services() {
    log_info "Checking if Attune services are running..."

    local api_url="${ATTUNE_API_URL:-http://localhost:8080}"

    # Check API health
    if curl -s -f "${api_url}/health" > /dev/null 2>&1; then
        log_success "API service is running at ${api_url}"
    else
        log_error "API service is not reachable at ${api_url}"
        log_warning "Make sure all services are running before running E2E tests"
        log_info "Start services with:"
        echo "  Terminal 1: cd crates/api && cargo run"
        echo "  Terminal 2: cd crates/executor && cargo run"
        echo "  Terminal 3: cd crates/worker && cargo run"
        echo "  Terminal 4: cd crates/sensor && cargo run"
        echo "  Terminal 5: cd crates/notifier && cargo run"
        exit 1
    fi
}

run_tests() {
    log_info "Running E2E integration tests..."

    cd "$SCRIPT_DIR"

    # Activate virtual environment
    source "$VENV_DIR/bin/activate"

    # Set environment variables
    export ATTUNE_API_URL="${ATTUNE_API_URL:-http://localhost:8080}"
    export TEST_TIMEOUT="${TEST_TIMEOUT:-60}"
    export PYTHONPATH="$SCRIPT_DIR:$PROJECT_ROOT:$PYTHONPATH"

    # Determine test path
    local test_path="e2e/"
    if [[ -n "$TIER" ]]; then
        test_path="e2e/tier${TIER}/"
        MARKER="-m tier${TIER}"
        log_header "Running Tier ${TIER} Tests"
    else
        log_header "Running All E2E Tests"
    fi

    # Build pytest command
    local pytest_args=()
    pytest_args+=("$test_path")

    if [[ -n "$VERBOSE" ]]; then
        pytest_args+=($VERBOSE)
    else
        pytest_args+=("-v")
    fi

    if [[ -n "$STOP_ON_FAIL" ]]; then
        pytest_args+=("$STOP_ON_FAIL")
    fi

    if [[ -n "$TEST_PATTERN" ]]; then
        pytest_args+=($TEST_PATTERN)
    fi

    if [[ -n "$MARKER" ]]; then
        pytest_args+=($MARKER)
    fi

    if [[ -n "$COVERAGE" ]]; then
        pytest_args+=($COVERAGE)
    fi

    log_info "Command: pytest ${pytest_args[*]}"
    echo ""

    # Run tests
    if pytest "${pytest_args[@]}"; then
        echo ""
        log_success "All tests passed!"
        return 0
    else
        echo ""
        log_error "Some tests failed"
        return 1
    fi
}

teardown_environment() {
    log_info "Cleaning up test artifacts..."

    # Remove test artifacts
    rm -rf "$SCRIPT_DIR/artifacts/*"
    rm -rf "$SCRIPT_DIR/logs/*"
    rm -rf "$SCRIPT_DIR/.pytest_cache"
    rm -rf "$SCRIPT_DIR/__pycache__"
    rm -rf "$SCRIPT_DIR/e2e/__pycache__"
    rm -rf "$SCRIPT_DIR/e2e/*/__pycache__"
    rm -rf "$SCRIPT_DIR/helpers/__pycache__"
    rm -rf "$SCRIPT_DIR/htmlcov"
    rm -rf "$SCRIPT_DIR/.coverage"

    log_success "Cleanup complete"
}

print_banner() {
    echo ""
    echo "╔════════════════════════════════════════════════════════╗"
    echo "║  Attune E2E Integration Test Suite                    ║"
    echo "╚════════════════════════════════════════════════════════╝"
    echo ""
}

print_tier_info() {
    if [[ -n "$TIER" ]]; then
        case "$TIER" in
            1)
                echo "  Tier 1: Core Automation Flows (MVP Essential)"
                echo "  Tests: Timers, Webhooks, Basic Workflows"
                ;;
            2)
                echo "  Tier 2: Orchestration & Data Flow"
                echo "  Tests: Workflows, Inquiries, Error Handling"
                ;;
            3)
                echo "  Tier 3: Advanced Features & Edge Cases"
                echo "  Tests: Performance, Security, Edge Cases"
                ;;
        esac
    else
        echo "  Running all test tiers (1, 2, 3)"
    fi
    echo ""
}

# Main execution
main() {
    print_banner
    print_tier_info

    # Setup if requested
    if [[ "$SETUP" == true ]]; then
        setup_environment
        echo ""
    fi

    # Ensure environment is set up
    if [[ ! -d "$VENV_DIR" ]]; then
        log_warning "Virtual environment not found. Setting up..."
        setup_environment
        echo ""
    fi

    # Check services
    check_services
    echo ""

    # Run tests
    if run_tests; then
        TEST_EXIT_CODE=0
    else
        TEST_EXIT_CODE=1
    fi

    echo ""

    # Teardown if requested
    if [[ "$TEARDOWN" == true ]]; then
        teardown_environment
        echo ""
    fi

    # Final summary
    echo "╔════════════════════════════════════════════════════════╗"
    if [[ $TEST_EXIT_CODE -eq 0 ]]; then
        echo "║  ${GREEN}✓ All E2E tests passed successfully${NC}                 ║"
    else
        echo "║  ${RED}✗ Some E2E tests failed${NC}                             ║"
        echo "║                                                        ║"
        echo "║  Check test output above for details                  ║"
        echo "║  Logs may be available in tests/logs/                 ║"
    fi
    echo "╚════════════════════════════════════════════════════════╝"
    echo ""

    exit $TEST_EXIT_CODE
}

# Run main function
main
