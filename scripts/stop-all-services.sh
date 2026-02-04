#!/bin/bash
# Stop all Attune services

echo "Stopping services..."

if [ -f logs/api.pid ]; then
    kill $(cat logs/api.pid) 2>/dev/null && echo "Stopped API service" || echo "API service not running"
    rm logs/api.pid
fi

if [ -f logs/executor.pid ]; then
    kill $(cat logs/executor.pid) 2>/dev/null && echo "Stopped Executor service" || echo "Executor service not running"
    rm logs/executor.pid
fi

if [ -f logs/worker.pid ]; then
    kill $(cat logs/worker.pid) 2>/dev/null && echo "Stopped Worker service" || echo "Worker service not running"
    rm logs/worker.pid
fi

if [ -f logs/sensor.pid ]; then
    kill $(cat logs/sensor.pid) 2>/dev/null && echo "Stopped Sensor service" || echo "Sensor service not running"
    rm logs/sensor.pid
fi

if [ -f logs/notifier.pid ]; then
    kill $(cat logs/notifier.pid) 2>/dev/null && echo "Stopped Notifier service" || echo "Notifier service not running"
    rm logs/notifier.pid
fi

echo "All services stopped"
