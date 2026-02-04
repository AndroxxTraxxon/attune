#!/bin/bash
# Start all Attune services in the background

echo "Building services first..."
cargo build

echo "Starting services..."

# Create logs directory if it doesn't exist
mkdir -p logs

# Start each service in the background, logging to separate files
echo "Starting API service..."
cargo run --bin attune-api > logs/api.log 2>&1 &
echo $! > logs/api.pid

echo "Starting Executor service..."
cargo run --bin attune-executor > logs/executor.log 2>&1 &
echo $! > logs/executor.pid

echo "Starting Worker service..."
cargo run --bin attune-worker > logs/worker.log 2>&1 &
echo $! > logs/worker.pid

echo "Starting Sensor service..."
cargo run --bin attune-sensor > logs/sensor.log 2>&1 &
echo $! > logs/sensor.pid

echo "Starting Notifier service..."
cargo run --bin attune-notifier > logs/notifier.log 2>&1 &
echo $! > logs/notifier.pid

echo ""
echo "All services started!"
echo "Logs are in the logs/ directory"
echo "To stop services, run: ./scripts/stop-all-services.sh"
echo ""
echo "Service PIDs:"
echo "  API:      $(cat logs/api.pid)"
echo "  Executor: $(cat logs/executor.pid)"
echo "  Worker:   $(cat logs/worker.pid)"
echo "  Sensor:   $(cat logs/sensor.pid)"
echo "  Notifier: $(cat logs/notifier.pid)"
