#!/bin/bash
# Test script for timer-driven echo action
# This script starts the sensor, executor, and worker services to test the happy path

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Attune Timer Echo Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Set environment variables
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
export RUST_LOG="info,attune_sensor=debug,attune_executor=debug,attune_worker=debug"

# Check if services are built
if [ ! -f "target/debug/attune-sensor" ]; then
    echo -e "${YELLOW}Building sensor service...${NC}"
    cargo build --bin attune-sensor
fi

if [ ! -f "target/debug/attune-executor" ]; then
    echo -e "${YELLOW}Building executor service...${NC}"
    cargo build --bin attune-executor
fi

if [ ! -f "target/debug/attune-worker" ]; then
    echo -e "${YELLOW}Building worker service...${NC}"
    cargo build --bin attune-worker
fi

# Create log directory
mkdir -p logs

# Verify database has the rule and action parameters
echo -e "${BLUE}Checking database setup...${NC}"
RULE_CHECK=$(PGPASSWORD=postgres psql -h localhost -U postgres -d attune -t -c "SELECT action_params::text FROM attune.rule WHERE ref = 'core.timer_echo_10s';" 2>/dev/null || echo "")

if [ -z "$RULE_CHECK" ]; then
    echo -e "${RED}ERROR: Rule 'core.timer_echo_10s' not found!${NC}"
    echo -e "${YELLOW}Please ensure the database is seeded properly.${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Rule found with action_params: $RULE_CHECK${NC}"
echo ""

# Function to cleanup on exit
cleanup() {
    echo -e "\n${YELLOW}Stopping services...${NC}"
    kill $SENSOR_PID $EXECUTOR_PID $WORKER_PID 2>/dev/null || true
    wait 2>/dev/null || true
    echo -e "${GREEN}Services stopped${NC}"
}

trap cleanup EXIT INT TERM

# Start services
echo -e "${BLUE}Starting services...${NC}"
echo ""

echo -e "${GREEN}Starting Sensor Service...${NC}"
./target/debug/attune-sensor > logs/sensor.log 2>&1 &
SENSOR_PID=$!
sleep 2

echo -e "${GREEN}Starting Executor Service...${NC}"
./target/debug/attune-executor > logs/executor.log 2>&1 &
EXECUTOR_PID=$!
sleep 2

echo -e "${GREEN}Starting Worker Service...${NC}"
./target/debug/attune-worker > logs/worker.log 2>&1 &
WORKER_PID=$!
sleep 2

echo ""
echo -e "${GREEN}✓ All services started${NC}"
echo -e "${BLUE}  Sensor PID:   $SENSOR_PID${NC}"
echo -e "${BLUE}  Executor PID: $EXECUTOR_PID${NC}"
echo -e "${BLUE}  Worker PID:   $WORKER_PID${NC}"
echo ""
echo -e "${YELLOW}Monitoring logs for 'hello, world' message...${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
echo ""
echo -e "${BLUE}========================================${NC}"
echo ""

# Monitor logs for the expected output
tail -f logs/sensor.log logs/executor.log logs/worker.log | while read line; do
    # Highlight "hello, world" in the output
    if echo "$line" | grep -qi "hello.*world"; then
        echo -e "${GREEN}>>> $line${NC}"
    elif echo "$line" | grep -qi "error\|failed"; then
        echo -e "${RED}$line${NC}"
    elif echo "$line" | grep -qi "event.*created\|enforcement.*created\|execution.*created"; then
        echo -e "${YELLOW}$line${NC}"
    else
        echo "$line"
    fi
done
