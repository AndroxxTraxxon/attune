#!/bin/sh
# Get Pack Dependencies Action - Core Pack
# API Wrapper for POST /api/v1/packs/dependencies
#
# This script uses pure POSIX shell without external dependencies like jq.
# It reads parameters in DOTENV format from stdin until the delimiter.

set -e

# Initialize variables
pack_paths=""
skip_validation="false"
api_url="http://localhost:8080"
api_token=""

# Read DOTENV-formatted parameters from stdin until delimiter
while IFS= read -r line; do
    # Check for parameter delimiter
    case "$line" in
        *"---ATTUNE_PARAMS_END---"*)
            break
            ;;
    esac
    [ -z "$line" ] && continue

    key="${line%%=*}"
    value="${line#*=}"

    # Remove quotes if present (both single and double)
    case "$value" in
        \"*\")
            value="${value#\"}"
            value="${value%\"}"
            ;;
        \'*\')
            value="${value#\'}"
            value="${value%\'}"
            ;;
    esac

    # Process parameters
    case "$key" in
        pack_paths)
            pack_paths="$value"
            ;;
        skip_validation)
            skip_validation="$value"
            ;;
        api_url)
            api_url="$value"
            ;;
        api_token)
            api_token="$value"
            ;;
    esac
done

# Validate required parameters
if [ -z "$pack_paths" ]; then
    printf '{"dependencies":[],"runtime_requirements":{},"missing_dependencies":[],"analyzed_packs":[],"errors":[{"pack_path":"input","error":"No pack paths provided"}]}\n'
    exit 1
fi

# Normalize boolean
case "$skip_validation" in
    true|True|TRUE|yes|Yes|YES|1) skip_validation="true" ;;
    *) skip_validation="false" ;;
esac

# Build JSON request body (escape pack_paths value for JSON)
pack_paths_escaped=$(printf '%s' "$pack_paths" | sed 's/\\/\\\\/g; s/"/\\"/g')

request_body=$(cat <<EOF
{
  "pack_paths": $pack_paths_escaped,
  "skip_validation": $skip_validation
}
EOF
)

# Create temp files for curl
temp_response=$(mktemp)
temp_headers=$(mktemp)

cleanup() {
    rm -f "$temp_response" "$temp_headers"
}
trap cleanup EXIT

# Make API call
http_code=$(curl -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    ${api_token:+-H "Authorization: Bearer ${api_token}"} \
    -d "$request_body" \
    -s \
    -w "%{http_code}" \
    -o "$temp_response" \
    --max-time 60 \
    --connect-timeout 10 \
    "${api_url}/api/v1/packs/dependencies" 2>/dev/null || echo "000")

# Check HTTP status
if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
    # Success - extract data field from API response
    response_body=$(cat "$temp_response")

    # Try to extract .data field using simple text processing
    # If response contains "data" field, extract it; otherwise use whole response
    case "$response_body" in
        *'"data":'*)
            # Extract content after "data": up to the closing brace
            # This is a simple extraction - assumes well-formed JSON
            data_content=$(printf '%s' "$response_body" | sed -n 's/.*"data":\s*\(.*\)}/\1/p')
            if [ -n "$data_content" ]; then
                printf '%s\n' "$data_content"
            else
                cat "$temp_response"
            fi
            ;;
        *)
            cat "$temp_response"
            ;;
    esac
    exit 0
else
    # Error response - try to extract error message
    error_msg="API request failed"
    if [ -s "$temp_response" ]; then
        # Try to extract error or message field
        response_content=$(cat "$temp_response")
        case "$response_content" in
            *'"error":'*)
                error_msg=$(printf '%s' "$response_content" | sed -n 's/.*"error":\s*"\([^"]*\)".*/\1/p')
                [ -z "$error_msg" ] && error_msg="API request failed"
                ;;
            *'"message":'*)
                error_msg=$(printf '%s' "$response_content" | sed -n 's/.*"message":\s*"\([^"]*\)".*/\1/p')
                [ -z "$error_msg" ] && error_msg="API request failed"
                ;;
        esac
    fi

    # Escape error message for JSON
    error_msg_escaped=$(printf '%s' "$error_msg" | sed 's/\\/\\\\/g; s/"/\\"/g')

    cat <<EOF
{
  "dependencies": [],
  "runtime_requirements": {},
  "missing_dependencies": [],
  "analyzed_packs": [],
  "errors": [{
    "pack_path": "api",
    "error": "API call failed (HTTP $http_code): $error_msg_escaped"
  }]
}
EOF
    exit 1
fi
