#!/bin/sh
# Sleep Action for E2E Testing
# Input: {"duration": N} — sleeps for N seconds (default: 1)
INPUT=$(cat)
DURATION=$(echo "$INPUT" | sed -n 's/.*"duration"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p')
DURATION=${DURATION:-1}
sleep "$DURATION"
echo "{\"success\": true, \"slept\": $DURATION}"
