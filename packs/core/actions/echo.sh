#!/bin/sh
# Echo Action - Core Pack
# Outputs a message to stdout
#
# This script uses pure POSIX shell without external dependencies like jq or yq.
# It reads parameters in DOTENV format from stdin until the delimiter.

set -e

# Initialize message variable
message=""

# Read DOTENV-formatted parameters from stdin until delimiter
while IFS= read -r line; do
    # Check for parameter delimiter
    case "$line" in
        *"---ATTUNE_PARAMS_END---"*)
            break
            ;;
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
    esac
done

# Echo the message (even if empty)
echo -n "$message"

# Exit successfully
exit 0
