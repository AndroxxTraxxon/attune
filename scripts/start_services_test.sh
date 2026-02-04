#!/bin/bash
# Start all Attune services for testing the timer demo
# This script starts API, Sensor, Executor, and Worker services in separate tmux panes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Attune Service Startup Script ===${NC}"
echo ""

# Check if tmux is available
if ! command -v tmux &> /dev/null; then
    echo -e "${RED}Error: tmux is not installed${NC}"
    echo "Please install tmux to use this script, or start services manually in separate terminals"
    exit 1
fi

# Set environment variables
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
export ATTUNE__DATABASE__URL="$DATABASE_URL"
export ATTUNE__MESSAGE_QUEUE__URL="amqp://guest:guest@localhost:5672/%2F"
export ATTUNE__JWT__SECRET="dev-secret-not-for-production"

echo -e "${GREEN}✓ Environment variables set${NC}"
echo ""

# Check if services are already running
if tmux has-session -t attune 2>/dev/null; then
    echo -e "${YELLOW}Attune session already exists${NC}"
    echo "Do you want to kill it and start fresh? (y/n)"
    read -r response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        tmux kill-session -t attune
        echo -e "${GREEN}✓ Old session killed${NC}"
    else
        echo "Attaching to existing session..."
        tmux attach -t attune
        exit 0
    fi
fi

echo -e "${BLUE}Starting services in tmux session 'attune'...${NC}"
echo ""

# Create new tmux session with 4 panes
tmux new-session -d -s attune -n services

# Split into 4 panes
tmux split-window -h -t attune
tmux split-window -v -t attune:0.0
tmux split-window -v -t attune:0.2

# Set environment in all panes
for pane in 0 1 2 3; do
    tmux send-keys -t attune:0.$pane "cd $(pwd)" C-m
    tmux send-keys -t attune:0.$pane "export DATABASE_URL='$DATABASE_URL'" C-m
    tmux send-keys -t attune:0.$pane "export ATTUNE__DATABASE__URL='$DATABASE_URL'" C-m
    tmux send-keys -t attune:0.$pane "export ATTUNE__MESSAGE_QUEUE__URL='amqp://guest:guest@localhost:5672/%2F'" C-m
    tmux send-keys -t attune:0.$pane "export ATTUNE__JWT__SECRET='dev-secret-not-for-production'" C-m
done

# Start API service (top-left)
echo -e "${GREEN}Starting API service...${NC}"
tmux send-keys -t attune:0.0 "echo '=== API Service ===' && cargo run --bin attune-api" C-m

# Start Sensor service (top-right)
echo -e "${GREEN}Starting Sensor service...${NC}"
tmux send-keys -t attune:0.1 "echo '=== Sensor Service ===' && sleep 5 && cargo run --bin attune-sensor" C-m

# Start Executor service (bottom-left)
echo -e "${GREEN}Starting Executor service...${NC}"
tmux send-keys -t attune:0.2 "echo '=== Executor Service ===' && sleep 5 && cargo run --bin attune-executor" C-m

# Start Worker service (bottom-right)
echo -e "${GREEN}Starting Worker service...${NC}"
tmux send-keys -t attune:0.3 "echo '=== Worker Service ===' && sleep 5 && cargo run --bin attune-worker" C-m

echo ""
echo -e "${GREEN}✓ All services starting in tmux session 'attune'${NC}"
echo ""
echo -e "${BLUE}Tmux commands:${NC}"
echo "  Attach to session:  tmux attach -t attune"
echo "  Detach from session: Ctrl+b, then d"
echo "  Switch panes:       Ctrl+b, then arrow keys"
echo "  Kill session:       tmux kill-session -t attune"
echo ""
echo -e "${BLUE}Service layout:${NC}"
echo "  ┌─────────────┬─────────────┐"
echo "  │ API         │ Sensor      │"
echo "  ├─────────────┼─────────────┤"
echo "  │ Executor    │ Worker      │"
echo "  └─────────────┴─────────────┘"
echo ""
echo -e "${YELLOW}Wait 30-60 seconds for all services to compile and start...${NC}"
echo ""
echo "Attaching to tmux session in 3 seconds..."
sleep 3

# Attach to the session
tmux attach -t attune
