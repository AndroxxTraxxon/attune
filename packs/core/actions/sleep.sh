#!/bin/sh
# Sleep Action - Core Pack
# Pauses execution for a specified duration
#
# This script uses pure POSIX shell without external dependencies like jq or yq.
# It reads parameters in DOTENV format from stdin until the delimiter.

set -e

# Initialize variables
seconds="1"
message=""

# Read DOTENV-formatted parameters from stdin until delimiter
while IFS= read -r line; do
    # Check for parameter delimiter
    case "$line" in
        *"---ATTUNE_PARAMS_END---"*)
            break
            ;;
        seconds=*)
            # Extract value after seconds=
            seconds="${line#seconds=}"
            # Remove quotes if present (both single and double)
            case "$seconds" in
                \"*\")
                    seconds="${seconds#\"}"
                    seconds="${seconds%\"}"
                    ;;
                \'*\')
                    seconds="${seconds#\'}"
                    seconds="${seconds%\'}"
                    ;;
            esac
            ;;
        message=*)
            # Extract value after message=
            message="${line#message=}"
            # Remove quotes if present
            case "$message" in
                \"*\")
                    message="${message#\"}"
                    message="${message%\"}"
                    ;;
                \'*\')
                    message="${message#\'}"
                    message="${message%\'}"
                    ;;
            esac
            ;;
    esac
done

# Validate seconds parameter (must be numeric)
case "$seconds" in
    ''|*[!0-9]*)
        echo "ERROR: seconds must be a positive integer" >&2
        exit 1
        ;;
esac

# Validate seconds range (0-3600)
if [ "$seconds" -lt 0 ] || [ "$seconds" -gt 3600 ]; then
    echo "ERROR: seconds must be between 0 and 3600" >&2
    exit 1
fi

# Display message if provided
if [ -n "$message" ]; then
    echo "$message"
fi

# Sleep for the specified duration
sleep "$seconds"

# Output result
echo "Slept for $seconds seconds"

# Exit successfully
exit 0
