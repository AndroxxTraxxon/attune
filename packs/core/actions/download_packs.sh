#!/bin/bash
# Download Packs Action - API Wrapper
# Thin wrapper around POST /api/v1/packs/download

set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters using jq
PACKS=$(echo "$INPUT" | jq -c '.packs // []')
DESTINATION_DIR=$(echo "$INPUT" | jq -r '.destination_dir // ""')
REGISTRY_URL=$(echo "$INPUT" | jq -r '.registry_url // "https://registry.attune.io/index.json"')
REF_SPEC=$(echo "$INPUT" | jq -r '.ref_spec // ""')
TIMEOUT=$(echo "$INPUT" | jq -r '.timeout // 300')
VERIFY_SSL=$(echo "$INPUT" | jq -r '.verify_ssl // true')
API_URL=$(echo "$INPUT" | jq -r '.api_url // "http://localhost:8080"')
API_TOKEN=$(echo "$INPUT" | jq -r '.api_token // ""')

# Validate required parameters
if [[ -z "$DESTINATION_DIR" ]] || [[ "$DESTINATION_DIR" == "null" ]]; then
    echo '{"downloaded_packs":[],"failed_packs":[{"source":"input","error":"destination_dir is required"}],"total_count":0,"success_count":0,"failure_count":1}' >&1
    exit 1
fi

# Build request body
REQUEST_BODY=$(jq -n \
    --argjson packs "$PACKS" \
    --arg destination_dir "$DESTINATION_DIR" \
    --arg registry_url "$REGISTRY_URL" \
    --argjson timeout "$TIMEOUT" \
    --argjson verify_ssl "$([[ "$VERIFY_SSL" == "true" ]] && echo true || echo false)" \
    '{
        packs: $packs,
        destination_dir: $destination_dir,
        registry_url: $registry_url,
        timeout: $timeout,
        verify_ssl: $verify_ssl
    }' | jq --arg ref_spec "$REF_SPEC" 'if $ref_spec != "" and $ref_spec != "null" then .ref_spec = $ref_spec else . end')

# Make API call
CURL_ARGS=(
    -X POST
    -H "Content-Type: application/json"
    -H "Accept: application/json"
    -d "$REQUEST_BODY"
    -s
    -w "\n%{http_code}"
    --max-time $((TIMEOUT + 30))
    --connect-timeout 10
)

if [[ -n "$API_TOKEN" ]] && [[ "$API_TOKEN" != "null" ]]; then
    CURL_ARGS+=(-H "Authorization: Bearer ${API_TOKEN}")
fi

RESPONSE=$(curl "${CURL_ARGS[@]}" "${API_URL}/api/v1/packs/download" 2>/dev/null || echo -e "\n000")

# Extract status code (last line)
HTTP_CODE=$(echo "$RESPONSE" | tail -n 1)
BODY=$(echo "$RESPONSE" | head -n -1)

# Check HTTP status
if [[ "$HTTP_CODE" -ge 200 ]] && [[ "$HTTP_CODE" -lt 300 ]]; then
    # Extract data field from API response
    echo "$BODY" | jq -r '.data // .'
    exit 0
else
    # Error response
    ERROR_MSG=$(echo "$BODY" | jq -r '.error // .message // "API request failed"' 2>/dev/null || echo "API request failed")

    cat <<EOF
{
  "downloaded_packs": [],
  "failed_packs": [{
    "source": "api",
    "error": "API call failed (HTTP $HTTP_CODE): $ERROR_MSG"
  }],
  "total_count": 0,
  "success_count": 0,
  "failure_count": 1
}
EOF
    exit 1
fi
