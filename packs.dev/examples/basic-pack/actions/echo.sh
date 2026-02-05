#!/bin/bash
set -e

# Get parameter from environment
MESSAGE="${ATTUNE_ACTION_message:-Hello from basic-pack!}"

# Output JSON result
echo "{\"result\": \"$MESSAGE\"}"
