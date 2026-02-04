#!/bin/bash
# Sleep Action - Core Pack
# Pauses execution for a specified duration

set -e

# Parse parameters from environment variables
SLEEP_SECONDS="${ATTUNE_ACTION_SECONDS:-1}"
MESSAGE="${ATTUNE_ACTION_MESSAGE:-}"

# Validate seconds parameter
if ! [[ "$SLEEP_SECONDS" =~ ^[0-9]+$ ]]; then
    echo "ERROR: seconds must be a positive integer" >&2
    exit 1
fi

if [ "$SLEEP_SECONDS" -lt 0 ] || [ "$SLEEP_SECONDS" -gt 3600 ]; then
    echo "ERROR: seconds must be between 0 and 3600" >&2
    exit 1
fi

# Display message if provided
if [ -n "$MESSAGE" ]; then
    echo "$MESSAGE"
fi

# Sleep for the specified duration
sleep "$SLEEP_SECONDS"

# Output result
echo "Slept for $SLEEP_SECONDS seconds"

# Exit successfully
exit 0
