#!/bin/bash
# Build Pack Environments Action - API Wrapper
# Thin wrapper around POST /api/v1/packs/build-envs

set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters using jq
PACK_PATHS=$(echo "$INPUT" | jq -c '.pack_paths // []')
PACKS_BASE_DIR=$(echo "$INPUT" | jq -r '.packs_base_dir // "/opt/attune/packs"')
PYTHON_VERSION=$(echo "$INPUT" | jq -r '.python_version // "3.11"')
NODEJS_VERSION=$(echo "$INPUT" | jq -r '.nodejs_version // "20"')
SKIP_PYTHON=$(echo "$INPUT" | jq -r '.skip_python // false')
SKIP_NODEJS=$(echo "$INPUT" | jq -r '.skip_nodejs // false')
FORCE_REBUILD=$(echo "$INPUT" | jq -r '.force_rebuild // false')
TIMEOUT=$(echo "$INPUT" | jq -r '.timeout // 600')
API_URL=$(echo "$INPUT" | jq -r '.api_url // "http://localhost:8080"')
API_TOKEN=$(echo "$INPUT" | jq -r '.api_token // ""')

# Validate required parameters
PACK_COUNT=$(echo "$PACK_PATHS" | jq -r 'length' 2>/dev/null || echo "0")
if [[ "$PACK_COUNT" -eq 0 ]]; then
    echo '{"built_environments":[],"failed_environments":[],"summary":{"total_packs":0,"success_count":0,"failure_count":0,"python_envs_built":0,"nodejs_envs_built":0,"total_duration_ms":0}}' >&1
    exit 1
fi

# Build request body
REQUEST_BODY=$(jq -n \
    --argjson pack_paths "$PACK_PATHS" \
    --arg packs_base_dir "$PACKS_BASE_DIR" \
    --arg python_version "$PYTHON_VERSION" \
    --arg nodejs_version "$NODEJS_VERSION" \
    --argjson skip_python "$([[ "$SKIP_PYTHON" == "true" ]] && echo true || echo false)" \
    --argjson skip_nodejs "$([[ "$SKIP_NODEJS" == "true" ]] && echo true || echo false)" \
    --argjson force_rebuild "$([[ "$FORCE_REBUILD" == "true" ]] && echo true || echo false)" \
    --argjson timeout "$TIMEOUT" \
    '{
        pack_paths: $pack_paths,
        packs_base_dir: $packs_base_dir,
        python_version: $python_version,
        nodejs_version: $nodejs_version,
        skip_python: $skip_python,
        skip_nodejs: $skip_nodejs,
        force_rebuild: $force_rebuild,
        timeout: $timeout
    }')

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

RESPONSE=$(curl "${CURL_ARGS[@]}" "${API_URL}/api/v1/packs/build-envs" 2>/dev/null || echo -e "\n000")

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
  "built_environments": [],
  "failed_environments": [{
    "pack_ref": "api",
    "pack_path": "",
    "runtime": "unknown",
    "error": "API call failed (HTTP $HTTP_CODE): $ERROR_MSG"
  }],
  "summary": {
    "total_packs": 0,
    "success_count": 0,
    "failure_count": 1,
    "python_envs_built": 0,
    "nodejs_envs_built": 0,
    "total_duration_ms": 0
  }
}
EOF
    exit 1
fi
