#!/bin/bash
# No Operation Action - Core Pack
# Does nothing - useful for testing and placeholder workflows

set -e

# Parse parameters from environment variables
MESSAGE="${ATTUNE_ACTION_MESSAGE:-}"
EXIT_CODE="${ATTUNE_ACTION_EXIT_CODE:-0}"

# Validate exit code parameter
if ! [[ "$EXIT_CODE" =~ ^[0-9]+$ ]]; then
    echo "ERROR: exit_code must be a positive integer" >&2
    exit 1
fi

if [ "$EXIT_CODE" -lt 0 ] || [ "$EXIT_CODE" -gt 255 ]; then
    echo "ERROR: exit_code must be between 0 and 255" >&2
    exit 1
fi

# Log message if provided
if [ -n "$MESSAGE" ]; then
    echo "[NOOP] $MESSAGE"
fi

# Output result
echo "No operation completed successfully"

# Exit with specified code
exit "$EXIT_CODE"
