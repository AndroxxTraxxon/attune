#!/bin/bash
# Register Packs Action - API Wrapper
# Thin wrapper around POST /api/v1/packs/register-batch

set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters using jq
PACK_PATHS=$(echo "$INPUT" | jq -c '.pack_paths // []')
PACKS_BASE_DIR=$(echo "$INPUT" | jq -r '.packs_base_dir // "/opt/attune/packs"')
SKIP_VALIDATION=$(echo "$INPUT" | jq -r '.skip_validation // false')
SKIP_TESTS=$(echo "$INPUT" | jq -r '.skip_tests // false')
FORCE=$(echo "$INPUT" | jq -r '.force // false')
API_URL=$(echo "$INPUT" | jq -r '.api_url // "http://localhost:8080"')
API_TOKEN=$(echo "$INPUT" | jq -r '.api_token // ""')

# Validate required parameters
PACK_COUNT=$(echo "$PACK_PATHS" | jq -r 'length' 2>/dev/null || echo "0")
if [[ "$PACK_COUNT" -eq 0 ]]; then
    echo '{"registered_packs":[],"failed_packs":[{"pack_ref":"input","pack_path":"","error":"No pack paths provided","error_stage":"input_validation"}],"summary":{"total_packs":0,"success_count":0,"failure_count":1,"total_components":0,"duration_ms":0}}' >&1
    exit 1
fi

# Build request body
REQUEST_BODY=$(jq -n \
    --argjson pack_paths "$PACK_PATHS" \
    --arg packs_base_dir "$PACKS_BASE_DIR" \
    --argjson skip_validation "$([[ "$SKIP_VALIDATION" == "true" ]] && echo true || echo false)" \
    --argjson skip_tests "$([[ "$SKIP_TESTS" == "true" ]] && echo true || echo false)" \
    --argjson force "$([[ "$FORCE" == "true" ]] && echo true || echo false)" \
    '{
        pack_paths: $pack_paths,
        packs_base_dir: $packs_base_dir,
        skip_validation: $skip_validation,
        skip_tests: $skip_tests,
        force: $force
    }')

# Make API call
CURL_ARGS=(
    -X POST
    -H "Content-Type: application/json"
    -H "Accept: application/json"
    -d "$REQUEST_BODY"
    -s
    -w "\n%{http_code}"
    --max-time 300
    --connect-timeout 10
)

if [[ -n "$API_TOKEN" ]] && [[ "$API_TOKEN" != "null" ]]; then
    CURL_ARGS+=(-H "Authorization: Bearer ${API_TOKEN}")
fi

RESPONSE=$(curl "${CURL_ARGS[@]}" "${API_URL}/api/v1/packs/register-batch" 2>/dev/null || echo -e "\n000")

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
  "registered_packs": [],
  "failed_packs": [{
    "pack_ref": "api",
    "pack_path": "",
    "error": "API call failed (HTTP $HTTP_CODE): $ERROR_MSG",
    "error_stage": "api_call"
  }],
  "summary": {
    "total_packs": 0,
    "success_count": 0,
    "failure_count": 1,
    "total_components": 0,
    "duration_ms": 0
  }
}
EOF
    exit 1
fi
