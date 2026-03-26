#!/bin/sh
# Echo Action - Core Pack
# Outputs a message to stdout
#
# This script uses pure POSIX shell without external dependencies like jq or yq.
# It reads parameters in DOTENV format from stdin until EOF.

set -e

# Initialize message variable
message=""

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
    esac
done

# Echo the message (even if empty)
echo -n "$message"

# Exit successfully
exit 0
