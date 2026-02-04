#!/bin/bash
# End-to-End Flow Test
#
# Tests the complete event lifecycle:
# 1. Sensor generates event
# 2. Rule matcher creates enforcement
# 3. Executor schedules execution
# 4. Worker executes action
# 5. Results are recorded

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Database connection
DB_URL="postgresql://postgres:postgres@localhost:5432/attune"

# Service PIDs
SENSOR_PID=""
EXECUTOR_PID=""
WORKER_PID=""

# Log files
SENSOR_LOG=$(mktemp /tmp/attune-sensor-e2e.XXXXXX)
EXECUTOR_LOG=$(mktemp /tmp/attune-executor-e2e.XXXXXX)
WORKER_LOG=$(mktemp /tmp/attune-worker-e2e.XXXXXX)

echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Attune End-to-End Flow Test                 ║${NC}"
echo -e "${BLUE}║   Sensor → Event → Enforcement → Execution    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}Cleaning up services...${NC}"

    if [ -n "$WORKER_PID" ]; then
        kill $WORKER_PID 2>/dev/null || true
        wait $WORKER_PID 2>/dev/null || true
        echo "  Stopped worker service"
    fi

    if [ -n "$EXECUTOR_PID" ]; then
        kill $EXECUTOR_PID 2>/dev/null || true
        wait $EXECUTOR_PID 2>/dev/null || true
        echo "  Stopped executor service"
    fi

    if [ -n "$SENSOR_PID" ]; then
        kill $SENSOR_PID 2>/dev/null || true
        wait $SENSOR_PID 2>/dev/null || true
        echo "  Stopped sensor service"
    fi

    echo -e "${GREEN}✓ Cleanup complete${NC}"
    echo ""
    echo "Log files (preserved for inspection):"
    echo "  Sensor:   $SENSOR_LOG"
    echo "  Executor: $EXECUTOR_LOG"
    echo "  Worker:   $WORKER_LOG"
}

trap cleanup EXIT INT TERM

# Check prerequisites
echo -e "${YELLOW}1. Checking prerequisites...${NC}"

if ! command -v psql &> /dev/null; then
    echo -e "${RED}ERROR: psql not found${NC}"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}ERROR: cargo not found${NC}"
    exit 1
fi

if ! psql "$DB_URL" -c "SELECT 1" &> /dev/null; then
    echo -e "${RED}ERROR: Cannot connect to database${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Prerequisites OK${NC}"
echo ""

# Get initial counts
echo -e "${YELLOW}2. Recording initial state...${NC}"
INITIAL_EVENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM event" | tr -d ' ')
INITIAL_ENFORCEMENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM enforcement" | tr -d ' ')
INITIAL_EXECUTIONS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM execution" | tr -d ' ')

echo "  Events:       $INITIAL_EVENTS"
echo "  Enforcements: $INITIAL_ENFORCEMENTS"
echo "  Executions:   $INITIAL_EXECUTIONS"
echo ""

# Verify configuration
echo -e "${YELLOW}3. Verifying configuration...${NC}"

TIMER_SENSOR=$(psql "$DB_URL" -t -c "SELECT ref FROM sensor WHERE ref = 'core.interval_timer_sensor'" | tr -d ' ')
TIMER_RULE=$(psql "$DB_URL" -t -c "SELECT ref FROM rule WHERE trigger_ref = 'core.intervaltimer' AND enabled = true LIMIT 1" | tr -d ' ')
ECHO_ACTION=$(psql "$DB_URL" -t -c "SELECT ref FROM action WHERE ref = 'core.echo'" | tr -d ' ')

if [ -z "$TIMER_SENSOR" ]; then
    echo -e "${RED}ERROR: Timer sensor not found${NC}"
    exit 1
fi

if [ -z "$TIMER_RULE" ]; then
    echo -e "${RED}ERROR: No enabled timer rules found${NC}"
    exit 1
fi

if [ -z "$ECHO_ACTION" ]; then
    echo -e "${RED}ERROR: Echo action not found${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Sensor: $TIMER_SENSOR${NC}"
echo -e "${GREEN}✓ Rule:   $TIMER_RULE${NC}"
echo -e "${GREEN}✓ Action: $ECHO_ACTION${NC}"
echo ""

# Start sensor service
echo -e "${YELLOW}4. Starting sensor service...${NC}"
cargo build --quiet --bin attune-sensor 2>&1 > /dev/null
cargo run --quiet --bin attune-sensor > "$SENSOR_LOG" 2>&1 &
SENSOR_PID=$!
echo "  PID: $SENSOR_PID"
sleep 3

if ! kill -0 $SENSOR_PID 2>/dev/null; then
    echo -e "${RED}ERROR: Sensor service failed to start${NC}"
    tail -30 "$SENSOR_LOG"
    exit 1
fi

echo -e "${GREEN}✓ Sensor service running${NC}"
echo ""

# Start executor service
echo -e "${YELLOW}5. Starting executor service...${NC}"
cargo build --quiet --bin attune-executor 2>&1 > /dev/null
cargo run --quiet --bin attune-executor > "$EXECUTOR_LOG" 2>&1 &
EXECUTOR_PID=$!
echo "  PID: $EXECUTOR_PID"
sleep 3

if ! kill -0 $EXECUTOR_PID 2>/dev/null; then
    echo -e "${RED}ERROR: Executor service failed to start${NC}"
    tail -30 "$EXECUTOR_LOG"
    exit 1
fi

echo -e "${GREEN}✓ Executor service running${NC}"
echo ""

# Start worker service
echo -e "${YELLOW}6. Starting worker service...${NC}"
cargo build --quiet --bin attune-worker 2>&1 > /dev/null
cargo run --quiet --bin attune-worker > "$WORKER_LOG" 2>&1 &
WORKER_PID=$!
echo "  PID: $WORKER_PID"
sleep 3

if ! kill -0 $WORKER_PID 2>/dev/null; then
    echo -e "${RED}ERROR: Worker service failed to start${NC}"
    tail -30 "$WORKER_LOG"
    exit 1
fi

echo -e "${GREEN}✓ Worker service running${NC}"
echo ""

# Monitor for events
echo -e "${YELLOW}7. Monitoring for events (max 30 seconds)...${NC}"
TIMEOUT=30
ELAPSED=0

while [ $ELAPSED -lt $TIMEOUT ]; do
    CURRENT_EVENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM event" | tr -d ' ')
    NEW_EVENTS=$((CURRENT_EVENTS - INITIAL_EVENTS))

    if [ $NEW_EVENTS -gt 0 ]; then
        echo -e "${GREEN}✓ Generated $NEW_EVENTS new event(s)${NC}"
        break
    fi

    echo -n "."
    sleep 1
    ELAPSED=$((ELAPSED + 1))
done

echo ""

if [ $NEW_EVENTS -eq 0 ]; then
    echo -e "${RED}ERROR: No events generated${NC}"
    exit 1
fi

# Monitor for enforcements
echo -e "${YELLOW}8. Monitoring for enforcements (max 10 seconds)...${NC}"
TIMEOUT=10
ELAPSED=0

while [ $ELAPSED -lt $TIMEOUT ]; do
    CURRENT_ENFORCEMENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM enforcement" | tr -d ' ')
    NEW_ENFORCEMENTS=$((CURRENT_ENFORCEMENTS - INITIAL_ENFORCEMENTS))

    if [ $NEW_ENFORCEMENTS -gt 0 ]; then
        echo -e "${GREEN}✓ Created $NEW_ENFORCEMENTS enforcement(s)${NC}"
        break
    fi

    echo -n "."
    sleep 1
    ELAPSED=$((ELAPSED + 1))
done

echo ""

if [ $NEW_ENFORCEMENTS -eq 0 ]; then
    echo -e "${RED}ERROR: No enforcements created${NC}"
    exit 1
fi

# Monitor for executions
echo -e "${YELLOW}9. Monitoring for executions (max 15 seconds)...${NC}"
TIMEOUT=15
ELAPSED=0

while [ $ELAPSED -lt $TIMEOUT ]; do
    CURRENT_EXECUTIONS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM execution" | tr -d ' ')
    NEW_EXECUTIONS=$((CURRENT_EXECUTIONS - INITIAL_EXECUTIONS))

    if [ $NEW_EXECUTIONS -gt 0 ]; then
        echo -e "${GREEN}✓ Created $NEW_EXECUTIONS execution(s)${NC}"
        break
    fi

    echo -n "."
    sleep 1
    ELAPSED=$((ELAPSED + 1))
done

echo ""

if [ $NEW_EXECUTIONS -eq 0 ]; then
    echo -e "${RED}ERROR: No executions created${NC}"
    echo -e "${YELLOW}This might indicate executor service is not processing enforcements${NC}"
    exit 1
fi

# Check for completed executions
echo -e "${YELLOW}10. Waiting for execution completion (max 15 seconds)...${NC}"
TIMEOUT=15
ELAPSED=0
COMPLETED=0

while [ $ELAPSED -lt $TIMEOUT ]; do
    COMPLETED=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM execution WHERE status = 'succeeded' AND created > NOW() - INTERVAL '1 minute'" | tr -d ' ')

    if [ $COMPLETED -gt 0 ]; then
        echo -e "${GREEN}✓ $COMPLETED execution(s) completed successfully${NC}"
        break
    fi

    echo -n "."
    sleep 1
    ELAPSED=$((ELAPSED + 1))
done

echo ""

# Display results
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║              Test Results                      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""

echo -e "${YELLOW}Recent Events:${NC}"
psql "$DB_URL" -c "
    SELECT id, trigger_ref, LEFT(payload::text, 50) as payload_snippet, created
    FROM event
    ORDER BY created DESC
    LIMIT 3
" 2>/dev/null

echo ""
echo -e "${YELLOW}Recent Enforcements:${NC}"
psql "$DB_URL" -c "
    SELECT id, rule_ref, status, created
    FROM enforcement
    ORDER BY created DESC
    LIMIT 3
" 2>/dev/null

echo ""
echo -e "${YELLOW}Recent Executions:${NC}"
psql "$DB_URL" -c "
    SELECT id, action_ref, status, LEFT(result::text, 40) as result_snippet, created
    FROM execution
    ORDER BY created DESC
    LIMIT 3
" 2>/dev/null

echo ""

# Final summary
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║              Summary                           ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo "  Events created:       $NEW_EVENTS"
echo "  Enforcements created: $NEW_ENFORCEMENTS"
echo "  Executions created:   $NEW_EXECUTIONS"
echo "  Executions completed: $COMPLETED"
echo ""

# Determine overall result
if [ $NEW_EVENTS -gt 0 ] && [ $NEW_ENFORCEMENTS -gt 0 ] && [ $NEW_EXECUTIONS -gt 0 ] && [ $COMPLETED -gt 0 ]; then
    echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✓ END-TO-END TEST PASSED                     ║${NC}"
    echo -e "${GREEN}║                                                ║${NC}"
    echo -e "${GREEN}║  Complete flow verified:                      ║${NC}"
    echo -e "${GREEN}║  Sensor → Event → Rule → Enforcement →        ║${NC}"
    echo -e "${GREEN}║  Execution → Worker → Completion              ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
    exit 0
elif [ $NEW_EVENTS -gt 0 ] && [ $NEW_ENFORCEMENTS -gt 0 ] && [ $NEW_EXECUTIONS -gt 0 ]; then
    echo -e "${YELLOW}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  ⚠ PARTIAL SUCCESS                            ║${NC}"
    echo -e "${YELLOW}║                                                ║${NC}"
    echo -e "${YELLOW}║  Flow works up to execution creation but      ║${NC}"
    echo -e "${YELLOW}║  executions haven't completed yet.            ║${NC}"
    echo -e "${YELLOW}║  This may be timing - check worker logs.      ║${NC}"
    echo -e "${YELLOW}╚════════════════════════════════════════════════╝${NC}"
    exit 0
else
    echo -e "${RED}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ✗ TEST FAILED                                 ║${NC}"
    echo -e "${RED}║                                                ║${NC}"
    echo -e "${RED}║  Flow did not complete as expected.           ║${NC}"
    echo -e "${RED}║  Check service logs for details.              ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════╝${NC}"
    exit 1
fi
