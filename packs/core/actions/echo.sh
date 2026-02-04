#!/bin/bash
# Echo Action - Core Pack
# Outputs a message to stdout with optional uppercase conversion

set -e

# Parse parameters from environment variables
# Attune passes action parameters as environment variables prefixed with ATTUNE_ACTION_
MESSAGE="${ATTUNE_ACTION_MESSAGE:-Hello, World!}"
UPPERCASE="${ATTUNE_ACTION_UPPERCASE:-false}"

# Convert to uppercase if requested
if [ "$UPPERCASE" = "true" ]; then
    MESSAGE=$(echo "$MESSAGE" | tr '[:lower:]' '[:upper:]')
fi

# Echo the message
echo "$MESSAGE"

# Exit successfully
exit 0
