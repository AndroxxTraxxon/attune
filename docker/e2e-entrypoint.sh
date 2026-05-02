#!/usr/bin/env bash
#
# Entrypoint for the E2E test runner container.
#
# Waits for the API to be healthy, then runs pytest with any
# arguments passed to the container.
#
# Supported arguments (passed through to the run_e2e.sh wrapper or pytest):
#   --tier <N>        Run only tier N tests (1, 2, or 3)
#   --tier1 / --tier2 / --tier3   Shorthand for --tier
#   -k <EXPR>         Pytest filter expression
#   -m <MARKER>       Pytest marker filter
#   -x                Stop on first failure
#   -v                Verbose output
#   --html <path>     Generate HTML report
#   Any other pytest arguments are passed through directly.
#
set -e

API_URL="${ATTUNE_API_URL:-http://api:8080}"
MAX_WAIT="${E2E_MAX_WAIT:-120}"

# ── Wait for API ──────────────────────────────────────────────────────────
echo "⏳ Waiting for API at ${API_URL}/health (timeout: ${MAX_WAIT}s)..."
elapsed=0
while ! curl -sf "${API_URL}/health" > /dev/null 2>&1; do
  if [ "$elapsed" -ge "$MAX_WAIT" ]; then
    echo "✗ API did not become healthy within ${MAX_WAIT}s"
    exit 1
  fi
  sleep 2
  elapsed=$((elapsed + 2))
done
echo "✓ API is healthy (waited ${elapsed}s)"

# ── Parse arguments ───────────────────────────────────────────────────────
PYTEST_ARGS=()
TEST_PATH="tests/e2e/"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tier)
      TEST_PATH="tests/e2e/tier${2}/"
      shift 2
      ;;
    --tier1)
      TEST_PATH="tests/e2e/tier1/"
      shift
      ;;
    --tier2)
      TEST_PATH="tests/e2e/tier2/"
      shift
      ;;
    --tier3)
      TEST_PATH="tests/e2e/tier3/"
      shift
      ;;
    *)
      PYTEST_ARGS+=("$1")
      shift
      ;;
  esac
done

# ── Run tests ─────────────────────────────────────────────────────────────
cd /app

export PYTHONPATH="/app/tests:/app:${PYTHONPATH:-}"

echo ""
echo "╔════════════════════════════════════════════════════════╗"
echo "║  Attune E2E Integration Tests                         ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""
echo "  API:   ${API_URL}"
echo "  Path:  ${TEST_PATH}"
echo "  Args:  ${PYTEST_ARGS[*]:-<none>}"
echo ""

exec pytest "${TEST_PATH}" "${PYTEST_ARGS[@]}"
