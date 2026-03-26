#!/bin/sh
# No Operation Action - Core Pack
# Does nothing - useful for testing and placeholder workflows
#
# This script uses pure POSIX shell without external dependencies like jq or yq.
# It reads parameters in DOTENV format from stdin until EOF.

set -e

# Initialize variables
message=""
exit_code="0"

# Read DOTENV-formatted parameters from stdin until EOF
while IFS= read -r line; do
    case "$line" in
        message=*)
            # Extract value after message=
            message="${line#message=}"
            # Remove quotes if present (both single and double)
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
        exit_code=*)
            # Extract value after exit_code=
            exit_code="${line#exit_code=}"
            # Remove quotes if present
            case "$exit_code" in
                \"*\")
                    exit_code="${exit_code#\"}"
                    exit_code="${exit_code%\"}"
                    ;;
                \'*\')
                    exit_code="${exit_code#\'}"
                    exit_code="${exit_code%\'}"
                    ;;
            esac
            ;;
    esac
done

# Validate exit code parameter (must be numeric)
case "$exit_code" in
    ''|*[!0-9]*)
        echo "ERROR: exit_code must be a positive integer" >&2
        exit 1
        ;;
esac

# Validate exit code range (0-255)
if [ "$exit_code" -lt 0 ] || [ "$exit_code" -gt 255 ]; then
    echo "ERROR: exit_code must be between 0 and 255" >&2
    exit 1
fi

# Log message if provided
if [ -n "$message" ]; then
    echo "[NOOP] $message"
fi

# Output result
echo "No operation completed successfully"

# Exit with specified code
exit "$exit_code"
