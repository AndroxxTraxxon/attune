#!/bin/sh
# Sensor service entrypoint script
# Copies sensor binary from image to shared volume on startup

set -e

echo "Sensor entrypoint: Checking for sensor binary..."

# Path to sensor binary in the image (baked in during build)
BINARY_IN_IMAGE="/opt/attune/packs-image/core/sensors/attune-core-timer-sensor"

# Destination in the shared volume
BINARY_DEST="/opt/attune/packs/core/sensors/attune-core-timer-sensor"

# Create sensors directory if it doesn't exist
mkdir -p "/opt/attune/packs/core/sensors"

# Check if we have a binary to copy from a different location in the image
# The Dockerfile copies the binary, but it gets hidden by the volume mount
# So we need to copy it from the image layer to the mounted volume

# Try to find the binary from docker build
if [ -f "$BINARY_IN_IMAGE" ]; then
    echo "Copying sensor binary from $BINARY_IN_IMAGE to $BINARY_DEST"
    cp -f "$BINARY_IN_IMAGE" "$BINARY_DEST"
    chmod +x "$BINARY_DEST"
    echo "✓ Sensor binary updated in shared volume"
elif [ ! -f "$BINARY_DEST" ]; then
    echo "ERROR: Sensor binary not found in image and not present in volume"
    echo "Expected at: $BINARY_IN_IMAGE or $BINARY_DEST"
    exit 1
else
    echo "Using existing sensor binary in shared volume: $BINARY_DEST"
fi

# Verify binary exists and is executable
if [ -f "$BINARY_DEST" ] && [ -x "$BINARY_DEST" ]; then
    echo "✓ Sensor binary ready: $BINARY_DEST"
    ls -lh "$BINARY_DEST"
else
    echo "ERROR: Sensor binary not executable or not found: $BINARY_DEST"
    exit 1
fi

echo "Starting Attune Sensor Service..."

# Execute the main service
exec /usr/local/bin/attune-service "$@"
