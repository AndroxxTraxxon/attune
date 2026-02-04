#!/bin/bash
# Check status of all Attune services

echo "Service Status:"
echo "==============="

check_service() {
    local name=$1
    local pidfile="logs/${name}.pid"
    
    if [ -f "$pidfile" ]; then
        local pid=$(cat "$pidfile")
        if ps -p $pid > /dev/null 2>&1; then
            echo "✓ $name (PID: $pid) - RUNNING"
        else
            echo "✗ $name - NOT RUNNING (stale PID file)"
        fi
    else
        echo "✗ $name - NOT RUNNING"
    fi
}

check_service "API"
check_service "Executor"
check_service "Worker"
check_service "Sensor"
check_service "Notifier"

echo ""
echo "To view logs: tail -f logs/<service>.log"
