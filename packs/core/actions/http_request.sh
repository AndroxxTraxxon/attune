#!/bin/bash
# HTTP Request Action - Core Pack
# Make HTTP requests to external APIs using curl

set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse required parameters
URL=$(echo "$INPUT" | jq -r '.url // ""')

if [ -z "$URL" ] || [ "$URL" = "null" ]; then
    echo "ERROR: 'url' parameter is required" >&2
    exit 1
fi

# Parse optional parameters
METHOD=$(echo "$INPUT" | jq -r '.method // "GET"' | tr '[:lower:]' '[:upper:]')
HEADERS=$(echo "$INPUT" | jq -r '.headers // {}')
BODY=$(echo "$INPUT" | jq -r '.body // ""')
JSON_BODY=$(echo "$INPUT" | jq -c '.json_body // null')
QUERY_PARAMS=$(echo "$INPUT" | jq -r '.query_params // {}')
TIMEOUT=$(echo "$INPUT" | jq -r '.timeout // 30')
VERIFY_SSL=$(echo "$INPUT" | jq -r '.verify_ssl // true')
AUTH_TYPE=$(echo "$INPUT" | jq -r '.auth_type // "none"')
FOLLOW_REDIRECTS=$(echo "$INPUT" | jq -r '.follow_redirects // true')
MAX_REDIRECTS=$(echo "$INPUT" | jq -r '.max_redirects // 10')

# Build URL with query parameters
FINAL_URL="$URL"
if [ "$QUERY_PARAMS" != "{}" ] && [ "$QUERY_PARAMS" != "null" ]; then
    QUERY_STRING=$(echo "$QUERY_PARAMS" | jq -r 'to_entries | map("\(.key)=\(.value | @uri)") | join("&")')
    if [[ "$FINAL_URL" == *"?"* ]]; then
        FINAL_URL="${FINAL_URL}&${QUERY_STRING}"
    else
        FINAL_URL="${FINAL_URL}?${QUERY_STRING}"
    fi
fi

# Build curl arguments array
CURL_ARGS=(
    -X "$METHOD"
    -s  # Silent mode
    -w "\n%{http_code}\n%{time_total}\n%{url_effective}\n"  # Write out metadata
    --max-time "$TIMEOUT"
    --connect-timeout 10
)

# Handle SSL verification
if [ "$VERIFY_SSL" = "false" ]; then
    CURL_ARGS+=(-k)
fi

# Handle redirects
if [ "$FOLLOW_REDIRECTS" = "true" ]; then
    CURL_ARGS+=(-L --max-redirs "$MAX_REDIRECTS")
fi

# Add headers
if [ "$HEADERS" != "{}" ] && [ "$HEADERS" != "null" ]; then
    while IFS= read -r header; do
        if [ -n "$header" ]; then
            CURL_ARGS+=(-H "$header")
        fi
    done < <(echo "$HEADERS" | jq -r 'to_entries | map("\(.key): \(.value)") | .[]')
fi

# Handle authentication
case "$AUTH_TYPE" in
    basic)
        AUTH_USERNAME=$(echo "$INPUT" | jq -r '.auth_username // ""')
        AUTH_PASSWORD=$(echo "$INPUT" | jq -r '.auth_password // ""')
        if [ -n "$AUTH_USERNAME" ] && [ "$AUTH_USERNAME" != "null" ]; then
            CURL_ARGS+=(-u "${AUTH_USERNAME}:${AUTH_PASSWORD}")
        fi
        ;;
    bearer)
        AUTH_TOKEN=$(echo "$INPUT" | jq -r '.auth_token // ""')
        if [ -n "$AUTH_TOKEN" ] && [ "$AUTH_TOKEN" != "null" ]; then
            CURL_ARGS+=(-H "Authorization: Bearer ${AUTH_TOKEN}")
        fi
        ;;
esac

# Handle request body
if [ "$JSON_BODY" != "null" ] && [ "$JSON_BODY" != "" ]; then
    CURL_ARGS+=(-H "Content-Type: application/json")
    CURL_ARGS+=(-d "$JSON_BODY")
elif [ -n "$BODY" ] && [ "$BODY" != "null" ]; then
    CURL_ARGS+=(-d "$BODY")
fi

# Capture start time
START_TIME=$(date +%s%3N)

# Make the request and capture response headers
TEMP_HEADERS=$(mktemp)
CURL_ARGS+=(--dump-header "$TEMP_HEADERS")

# Execute curl and capture output
set +e
RESPONSE=$(curl "${CURL_ARGS[@]}" "$FINAL_URL" 2>&1)
CURL_EXIT_CODE=$?
set -e

# Calculate elapsed time
END_TIME=$(date +%s%3N)
ELAPSED_MS=$((END_TIME - START_TIME))

# Parse curl output (last 3 lines are: http_code, time_total, url_effective)
BODY_OUTPUT=$(echo "$RESPONSE" | head -n -3)
HTTP_CODE=$(echo "$RESPONSE" | tail -n 3 | head -n 1 | tr -d '\r\n')
CURL_TIME=$(echo "$RESPONSE" | tail -n 2 | head -n 1 | tr -d '\r\n')
EFFECTIVE_URL=$(echo "$RESPONSE" | tail -n 1 | tr -d '\r\n')

# Ensure HTTP_CODE is numeric, default to 0 if not
if ! [[ "$HTTP_CODE" =~ ^[0-9]+$ ]]; then
    HTTP_CODE=0
fi

# If curl failed, handle error
if [ "$CURL_EXIT_CODE" -ne 0 ]; then
    ERROR_MSG="curl failed with exit code $CURL_EXIT_CODE"

    # Determine specific error
    case $CURL_EXIT_CODE in
        6)  ERROR_MSG="Could not resolve host" ;;
        7)  ERROR_MSG="Failed to connect to host" ;;
        28) ERROR_MSG="Request timeout" ;;
        35) ERROR_MSG="SSL/TLS connection error" ;;
        52) ERROR_MSG="Empty reply from server" ;;
        56) ERROR_MSG="Failure receiving network data" ;;
        *)  ERROR_MSG="curl error code $CURL_EXIT_CODE" ;;
    esac

    # Output error result as JSON
    jq -n \
        --arg error "$ERROR_MSG" \
        --argjson elapsed "$ELAPSED_MS" \
        --arg url "$FINAL_URL" \
        '{
            status_code: 0,
            headers: {},
            body: "",
            json: null,
            elapsed_ms: $elapsed,
            url: $url,
            success: false,
            error: $error
        }'

    rm -f "$TEMP_HEADERS"
    exit 1
fi

# Parse response headers into JSON
HEADERS_JSON="{}"
if [ -f "$TEMP_HEADERS" ]; then
    # Skip the status line and parse headers
    HEADERS_JSON=$(grep -v "^HTTP/" "$TEMP_HEADERS" | grep ":" | sed 's/\r$//' | jq -R -s -c '
        split("\n") |
        map(select(length > 0)) |
        map(split(": "; "") | select(length > 1) | {key: .[0], value: (.[1:] | join(": "))}) |
        map({(.key): .value}) |
        add // {}
    ' || echo '{}')
    rm -f "$TEMP_HEADERS"
fi

# Ensure HEADERS_JSON is valid JSON
if ! echo "$HEADERS_JSON" | jq empty 2>/dev/null; then
    HEADERS_JSON="{}"
fi

# Determine if successful (2xx status code)
SUCCESS=false
if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
    SUCCESS=true
fi

# Try to parse body as JSON
JSON_PARSED="null"
if [ -n "$BODY_OUTPUT" ] && echo "$BODY_OUTPUT" | jq empty 2>/dev/null; then
    JSON_PARSED=$(echo "$BODY_OUTPUT" | jq -c '.' || echo 'null')
fi

# Output result as JSON
jq -n \
    --argjson status_code "$HTTP_CODE" \
    --argjson headers "$HEADERS_JSON" \
    --arg body "$BODY_OUTPUT" \
    --argjson json "$JSON_PARSED" \
    --argjson elapsed "$ELAPSED_MS" \
    --arg url "$EFFECTIVE_URL" \
    --argjson success "$SUCCESS" \
    '{
        status_code: $status_code,
        headers: $headers,
        body: $body,
        json: $json,
        elapsed_ms: $elapsed,
        url: $url,
        success: $success
    }'

# Exit with success
exit 0
