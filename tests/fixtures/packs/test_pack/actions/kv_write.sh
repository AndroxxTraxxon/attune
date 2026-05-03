#!/bin/sh
# Key-Value Write Action for E2E Testing
# Input: {"key": "...", "value": "..."}
INPUT=$(cat)
KEY=$(echo "$INPUT" | sed -n 's/.*"key"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
VALUE=$(echo "$INPUT" | sed -n 's/.*"value"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

if [ -z "$KEY" ]; then
  echo '{"success": false, "error": "Missing key parameter"}' >&2
  exit 1
fi

RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "${ATTUNE_API_URL}/api/v1/keys" \
  -H "Authorization: Bearer ${ATTUNE_API_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{\"ref\": \"${KEY}\", \"name\": \"${KEY}\", \"value\": \"${VALUE}\", \"owner_type\": \"system\"}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
  echo "{\"success\": true, \"key\": \"${KEY}\", \"value\": \"${VALUE}\"}"
else
  echo "{\"success\": false, \"error\": \"Failed to write key (HTTP ${HTTP_CODE}): ${BODY}\"}" >&2
  exit 1
fi
