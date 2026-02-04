#!/bin/bash
# Script to stop system services that conflict with Docker Compose services
# Run this before starting Docker Compose to avoid port conflicts

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=========================================="
echo "Stopping System Services for Docker"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if service is running
is_service_running() {
    local service=$1
    systemctl is-active --quiet "$service" 2>/dev/null
}

# Function to stop a system service
stop_service() {
    local service=$1
    local port=$2

    echo -n "Checking $service (port $port)... "

    if is_service_running "$service"; then
        echo -e "${YELLOW}RUNNING${NC}"
        echo -n "  Stopping $service... "

        if sudo systemctl stop "$service" 2>/dev/null; then
            echo -e "${GREEN}STOPPED${NC}"

            # Optionally disable to prevent auto-restart on boot
            read -p "  Disable $service on boot? (y/N) " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                sudo systemctl disable "$service" 2>/dev/null
                echo -e "    ${GREEN}DISABLED on boot${NC}"
            fi
        else
            echo -e "${RED}FAILED${NC}"
            echo "    You may need to stop it manually: sudo systemctl stop $service"
        fi
    else
        echo -e "${GREEN}NOT RUNNING${NC}"
    fi
}

# Function to check if port is in use
check_port() {
    local port=$1
    local service=$2

    echo -n "Checking port $port ($service)... "

    if nc -z localhost "$port" 2>/dev/null; then
        echo -e "${YELLOW}IN USE${NC}"

        # Try to find what's using it
        local pid=$(lsof -ti tcp:"$port" 2>/dev/null || fuser "$port"/tcp 2>/dev/null | awk '{print $1}')

        if [ -n "$pid" ]; then
            local process=$(ps -p "$pid" -o comm= 2>/dev/null || echo "unknown")
            echo "    Process: $process (PID: $pid)"
            echo "    To kill: sudo kill $pid"
        fi
    else
        echo -e "${GREEN}FREE${NC}"
    fi
}

echo "Step 1: Stopping System Services"
echo "----------------------------------"

# PostgreSQL (port 5432)
stop_service "postgresql" "5432"

# RabbitMQ (ports 5672, 15672)
stop_service "rabbitmq-server" "5672"

# Redis (port 6379)
stop_service "redis" "6379"
stop_service "redis-server" "6379"

echo ""
echo "Step 2: Verifying Ports are Free"
echo "----------------------------------"

# Check critical ports
check_port 5432 "PostgreSQL"
check_port 5672 "RabbitMQ AMQP"
check_port 15672 "RabbitMQ Management"
check_port 6379 "Redis"
check_port 8080 "API Service"
check_port 8081 "Notifier Service"
check_port 3000 "Web UI"

echo ""
echo "Step 3: Cleanup Docker Resources"
echo "----------------------------------"

# Check for any existing Attune containers
echo -n "Checking for existing Attune containers... "
if docker ps -a --format '{{.Names}}' | grep -q "attune-"; then
    echo -e "${YELLOW}FOUND${NC}"
    echo "  Stopping and removing existing containers..."
    docker compose -f "$PROJECT_ROOT/docker compose.yaml" down 2>/dev/null || true
    echo -e "  ${GREEN}CLEANED${NC}"
else
    echo -e "${GREEN}NONE${NC}"
fi

# Check for orphaned containers on these ports
echo -n "Checking for orphaned containers on critical ports... "
ORPHANED=$(docker ps --format '{{.ID}} {{.Ports}}' | grep -E '5432|5672|6379|8080|8081|3000' | awk '{print $1}' || true)

if [ -n "$ORPHANED" ]; then
    echo -e "${YELLOW}FOUND${NC}"
    echo "  Orphaned container IDs: $ORPHANED"
    read -p "  Stop these containers? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "$ORPHANED" | xargs docker stop 2>/dev/null || true
        echo "$ORPHANED" | xargs docker rm 2>/dev/null || true
        echo -e "  ${GREEN}REMOVED${NC}"
    fi
else
    echo -e "${GREEN}NONE${NC}"
fi

echo ""
echo "=========================================="
echo "Summary"
echo "=========================================="
echo ""

# Final port check
ALL_CLEAR=true

for port in 5432 5672 6379 8080 8081 3000; do
    if nc -z localhost "$port" 2>/dev/null; then
        echo -e "${RED}✗${NC} Port $port is still in use"
        ALL_CLEAR=false
    else
        echo -e "${GREEN}✓${NC} Port $port is free"
    fi
done

echo ""

if $ALL_CLEAR; then
    echo -e "${GREEN}All ports are clear! You can now run:${NC}"
    echo ""
    echo "  cd $PROJECT_ROOT"
    echo "  docker compose up -d"
    echo ""
    echo "Or use the Makefile:"
    echo ""
    echo "  make docker-up"
    echo ""
else
    echo -e "${YELLOW}Some ports are still in use. Please resolve manually.${NC}"
    echo ""
    echo "Helpful commands:"
    echo "  lsof -i :PORT        # Find process using PORT"
    echo "  sudo kill PID        # Kill process by PID"
    echo "  docker ps -a         # List all containers"
    echo "  docker stop NAME     # Stop container"
    echo ""
fi

echo "To re-enable system services later:"
echo "  sudo systemctl start postgresql"
echo "  sudo systemctl start rabbitmq-server"
echo "  sudo systemctl start redis-server"
echo ""
