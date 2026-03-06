#!/bin/sh
# Build Pack Environments Action - Core Pack
# API Wrapper for POST /api/v1/packs/build-envs
#
# This script uses pure POSIX shell without external dependencies like jq.
# It reads parameters in DOTENV format from stdin until EOF.

set -e

# Initialize variables
pack_paths=""
packs_base_dir="/opt/attune/packs"
python_version="3.11"
nodejs_version="20"
skip_python="false"
skip_nodejs="false"
force_rebuild="false"
timeout="600"
api_url="http://localhost:8080"
api_token=""

# Read DOTENV-formatted parameters from stdin until EOF
while IFS= read -r line; do
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
        packs_base_dir)
            packs_base_dir="$value"
            ;;
        python_version)
            python_version="$value"
            ;;
        nodejs_version)
            nodejs_version="$value"
            ;;
        skip_python)
            skip_python="$value"
            ;;
        skip_nodejs)
            skip_nodejs="$value"
            ;;
        force_rebuild)
            force_rebuild="$value"
            ;;
        timeout)
            timeout="$value"
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
    printf '{"built_environments":[],"failed_environments":[],"summary":{"total_packs":0,"success_count":0,"failure_count":0,"python_envs_built":0,"nodejs_envs_built":0,"total_duration_ms":0}}\n'
    exit 1
fi

# Normalize booleans
case "$skip_python" in
    true|True|TRUE|yes|Yes|YES|1) skip_python="true" ;;
    *) skip_python="false" ;;
esac

case "$skip_nodejs" in
    true|True|TRUE|yes|Yes|YES|1) skip_nodejs="true" ;;
    *) skip_nodejs="false" ;;
esac

case "$force_rebuild" in
    true|True|TRUE|yes|Yes|YES|1) force_rebuild="true" ;;
    *) force_rebuild="false" ;;
esac

# Validate timeout is numeric
case "$timeout" in
    ''|*[!0-9]*)
        timeout="600"
        ;;
esac

# Escape values for JSON
pack_paths_escaped=$(printf '%s' "$pack_paths" | sed 's/\\/\\\\/g; s/"/\\"/g')
packs_base_dir_escaped=$(printf '%s' "$packs_base_dir" | sed 's/\\/\\\\/g; s/"/\\"/g')
python_version_escaped=$(printf '%s' "$python_version" | sed 's/\\/\\\\/g; s/"/\\"/g')
nodejs_version_escaped=$(printf '%s' "$nodejs_version" | sed 's/\\/\\\\/g; s/"/\\"/g')

# Build JSON request body
request_body=$(cat <<EOF
{
  "pack_paths": $pack_paths_escaped,
  "packs_base_dir": "$packs_base_dir_escaped",
  "python_version": "$python_version_escaped",
  "nodejs_version": "$nodejs_version_escaped",
  "skip_python": $skip_python,
  "skip_nodejs": $skip_nodejs,
  "force_rebuild": $force_rebuild,
  "timeout": $timeout
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

# Calculate curl timeout (request timeout + buffer)
curl_timeout=$((timeout + 30))

# Make API call
http_code=$(curl -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    ${api_token:+-H "Authorization: Bearer ${api_token}"} \
    -d "$request_body" \
    -s \
    -w "%{http_code}" \
    -o "$temp_response" \
    --max-time "$curl_timeout" \
    --connect-timeout 10 \
    "${api_url}/api/v1/packs/build-envs" 2>/dev/null || echo "000")

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
  "built_environments": [],
  "failed_environments": [{
    "pack_ref": "api",
    "pack_path": "",
    "runtime": "unknown",
    "error": "API call failed (HTTP $http_code): $error_msg_escaped"
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
