#!/bin/sh
# List Example Action
# Demonstrates JSON Lines output format for streaming results
#
# This script uses pure POSIX shell without external dependencies like jq.
# It reads parameters in DOTENV format from stdin until the delimiter.

set -e

# Initialize count with default
count=5

# Read DOTENV-formatted parameters from stdin until delimiter
while IFS= read -r line; do
    case "$line" in
        *"---ATTUNE_PARAMS_END---"*)
            break
            ;;
        count=*)
            # Extract value after count=
            count="${line#count=}"
            # Remove quotes if present (both single and double)
            case "$count" in
                \"*\")
                    count="${count#\"}"
                    count="${count%\"}"
                    ;;
                \'*\')
                    count="${count#\'}"
                    count="${count%\'}"
                    ;;
            esac
            ;;
    esac
done

# Validate count is a positive integer
case "$count" in
    ''|*[!0-9]*)
        count=5
        ;;
esac

if [ "$count" -lt 1 ]; then
    count=1
elif [ "$count" -gt 100 ]; then
    count=100
fi

# Generate JSON Lines output (one JSON object per line)
i=1
while [ "$i" -le "$count" ]; do
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    printf '{"id": %d, "value": "item_%d", "timestamp": "%s"}\n' "$i" "$i" "$timestamp"
    i=$((i + 1))
done

exit 0
