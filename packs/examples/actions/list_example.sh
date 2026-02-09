#!/bin/bash
# List Example Action
# Demonstrates JSON Lines output format for streaming results

set -euo pipefail

# Read parameters from stdin (JSON format)
read -r params_json

# Extract count parameter (default to 5 if not provided)
count=$(echo "$params_json" | jq -r '.count // 5')

# Generate JSON Lines output (one JSON object per line)
for i in $(seq 1 "$count"); do
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    echo "{\"id\": $i, \"value\": \"item_$i\", \"timestamp\": \"$timestamp\"}"
done
