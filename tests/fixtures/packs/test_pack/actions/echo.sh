#!/bin/sh
# Echo Action for E2E Testing (Shell version)
# Reads JSON from stdin, echoes it back as the result
INPUT=$(cat)
echo "{\"success\": true, \"input\": $INPUT}"
