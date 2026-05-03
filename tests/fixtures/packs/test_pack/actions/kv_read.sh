#!/bin/sh
# Key-Value Read Action for E2E Testing
# Input: {"key": "..."}
INPUT=$(cat)
KEY=$(echo "$INPUT" | sed -n 's/.*"key"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

if [ -z "$KEY" ]; then
  echo '{"success": false, "error": "Missing key parameter"}' >&2
  exit 1
fi

RESPONSE=$(curl -s -w "\n%{http_code}" "${ATTUNE_API_URL}/api/v1/keys/${KEY}" \
  -H "Authorization: Bearer ${ATTUNE_API_TOKEN}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
  echo "$BODY"
else
  echo "{\"success\": false, \"error\": \"Key not found: ${KEY} (HTTP ${HTTP_CODE})\"}" >&2
  exit 1
fi
