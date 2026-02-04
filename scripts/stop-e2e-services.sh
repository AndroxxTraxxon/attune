#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
PID_DIR="./tests/pids"

echo -e "${GREEN}=== Stopping Attune E2E Services ===${NC}\n"

# Function to stop a service
stop_service() {
    local service_name=$1
    local pid_file="$PID_DIR/$service_name.pid"

    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${YELLOW}→${NC} Stopping $service_name (PID: $pid)..."
            kill $pid 2>/dev/null || true

            # Wait up to 5 seconds for graceful shutdown
            local count=0
            while ps -p $pid > /dev/null 2>&1 && [ $count -lt 5 ]; do
                sleep 1
                count=$((count + 1))
            done

            # Force kill if still running
            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${YELLOW}!${NC} Forcefully killing $service_name..."
                kill -9 $pid 2>/dev/null || true
                sleep 1
            fi

            echo -e "${GREEN}✓${NC} $service_name stopped"
        else
            echo -e "${YELLOW}!${NC} $service_name PID file exists but process not running"
        fi
        rm -f "$pid_file"
    else
        echo -e "${YELLOW}!${NC} No PID file found for $service_name"
    fi
}

# Check if any services are running
if [ ! -d "$PID_DIR" ] || [ -z "$(ls -A $PID_DIR 2>/dev/null)" ]; then
    echo -e "${YELLOW}No E2E services appear to be running${NC}"
    exit 0
fi

# Stop services in reverse order
echo -e "${YELLOW}Stopping services...${NC}\n"

# Stop in reverse dependency order
stop_service "notifier"
stop_service "sensor"
stop_service "worker"
stop_service "executor"
stop_service "api"

echo -e "\n${GREEN}=== All E2E Services Stopped ===${NC}\n"

# Clean up PID directory if empty
if [ -d "$PID_DIR" ] && [ -z "$(ls -A $PID_DIR)" ]; then
    rmdir "$PID_DIR" 2>/dev/null || true
fi

echo -e "To restart services:"
echo -e "  ${YELLOW}./scripts/start-e2e-services.sh${NC}\n"
