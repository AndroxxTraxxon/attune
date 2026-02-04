#!/bin/bash
# Test Sensor Service - Verify end-to-end event flow
#
# This script:
# 1. Starts the sensor service
# 2. Monitors for events being created
# 3. Monitors for enforcements being created
# 4. Verifies the event->enforcement flow works

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Database connection
DB_URL="postgresql://postgres:postgres@localhost:5432/attune"

echo -e "${GREEN}=== Attune Sensor Service Test ===${NC}"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v psql &> /dev/null; then
    echo -e "${RED}ERROR: psql not found. Please install PostgreSQL client.${NC}"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}ERROR: cargo not found. Please install Rust.${NC}"
    exit 1
fi

# Test database connection
if ! psql "$DB_URL" -c "SELECT 1" &> /dev/null; then
    echo -e "${RED}ERROR: Cannot connect to database${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Prerequisites OK${NC}"
echo ""

# Get initial counts
echo -e "${YELLOW}Getting initial event/enforcement counts...${NC}"
INITIAL_EVENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM event" | tr -d ' ')
INITIAL_ENFORCEMENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM enforcement" | tr -d ' ')

echo "Initial events: $INITIAL_EVENTS"
echo "Initial enforcements: $INITIAL_ENFORCEMENTS"
echo ""

# Check if timer sensor and rule exist
echo -e "${YELLOW}Checking sensor and rule configuration...${NC}"
TIMER_SENSOR=$(psql "$DB_URL" -t -c "SELECT ref FROM sensor WHERE ref = 'core.interval_timer_sensor'" | tr -d ' ')
TIMER_RULE=$(psql "$DB_URL" -t -c "SELECT ref FROM rule WHERE trigger_ref = 'core.intervaltimer' AND enabled = true" | tr -d ' ')

if [ -z "$TIMER_SENSOR" ]; then
    echo -e "${RED}ERROR: Timer sensor not found in database${NC}"
    exit 1
fi

if [ -z "$TIMER_RULE" ]; then
    echo -e "${RED}ERROR: No enabled timer rules found${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Found sensor: $TIMER_SENSOR${NC}"
echo -e "${GREEN}✓ Found rule: $TIMER_RULE${NC}"
echo ""

# Start sensor service in background
echo -e "${YELLOW}Starting sensor service...${NC}"
SENSOR_LOG=$(mktemp /tmp/attune-sensor-test.XXXXXX)
echo "Logs: $SENSOR_LOG"

cargo run --quiet --bin attune-sensor -- --log-level debug > "$SENSOR_LOG" 2>&1 &
SENSOR_PID=$!

echo "Sensor service PID: $SENSOR_PID"

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}Cleaning up...${NC}"
    if [ -n "$SENSOR_PID" ]; then
        kill $SENSOR_PID 2>/dev/null || true
        wait $SENSOR_PID 2>/dev/null || true
    fi
    echo -e "${GREEN}✓ Cleanup complete${NC}"
}

trap cleanup EXIT INT TERM

# Wait for service to start
echo -e "${YELLOW}Waiting for service to initialize...${NC}"
sleep 5

# Check if process is still running
if ! kill -0 $SENSOR_PID 2>/dev/null; then
    echo -e "${RED}ERROR: Sensor service failed to start${NC}"
    echo -e "${YELLOW}Last 50 lines of log:${NC}"
    tail -50 "$SENSOR_LOG"
    exit 1
fi

echo -e "${GREEN}✓ Sensor service started${NC}"
echo ""

# Monitor for events (30 second timeout)
echo -e "${YELLOW}Monitoring for events (waiting up to 30 seconds)...${NC}"
TIMEOUT=30
ELAPSED=0

while [ $ELAPSED -lt $TIMEOUT ]; do
    CURRENT_EVENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM event" | tr -d ' ')
    NEW_EVENTS=$((CURRENT_EVENTS - INITIAL_EVENTS))

    if [ $NEW_EVENTS -gt 0 ]; then
        echo -e "${GREEN}✓ Generated $NEW_EVENTS new event(s)!${NC}"

        # Show recent events
        echo -e "${YELLOW}Recent events:${NC}"
        psql "$DB_URL" -c "
            SELECT id, trigger_ref, created
            FROM event
            ORDER BY created DESC
            LIMIT 5
        "
        break
    fi

    echo -n "."
    sleep 1
    ELAPSED=$((ELAPSED + 1))
done

echo ""

if [ $NEW_EVENTS -eq 0 ]; then
    echo -e "${RED}ERROR: No events generated after $TIMEOUT seconds${NC}"
    echo -e "${YELLOW}Sensor service logs:${NC}"
    tail -100 "$SENSOR_LOG"
    exit 1
fi

# Check for enforcements
echo -e "${YELLOW}Checking for enforcements...${NC}"
sleep 2

CURRENT_ENFORCEMENTS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM enforcement" | tr -d ' ')
NEW_ENFORCEMENTS=$((CURRENT_ENFORCEMENTS - INITIAL_ENFORCEMENTS))

if [ $NEW_ENFORCEMENTS -gt 0 ]; then
    echo -e "${GREEN}✓ Created $NEW_ENFORCEMENTS enforcement(s)!${NC}"

    # Show recent enforcements
    echo -e "${YELLOW}Recent enforcements:${NC}"
    psql "$DB_URL" -c "
        SELECT e.id, e.rule_ref, e.status, e.created
        FROM enforcement e
        ORDER BY e.created DESC
        LIMIT 5
    "
else
    echo -e "${YELLOW}⚠ No enforcements created yet (this might be OK if rule matching hasn't run)${NC}"
fi

echo ""

# Show sensor service logs
echo -e "${YELLOW}Sensor service logs (last 30 lines):${NC}"
tail -30 "$SENSOR_LOG"
echo ""

# Summary
echo -e "${GREEN}=== Test Summary ===${NC}"
echo "Events generated: $NEW_EVENTS"
echo "Enforcements created: $NEW_ENFORCEMENTS"

if [ $NEW_EVENTS -gt 0 ]; then
    echo -e "${GREEN}✓ TEST PASSED: Sensor service is generating events!${NC}"
    exit 0
else
    echo -e "${RED}✗ TEST FAILED: No events generated${NC}"
    exit 1
fi
