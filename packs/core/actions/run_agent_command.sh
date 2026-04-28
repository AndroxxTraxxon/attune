#!/bin/sh
# Run Agent Command - Core Pack
# Launches an AI/agent harness with execution-scoped MCP environment.

set -eu

command=""
working_dir=""
mcp_command="/opt/attune/agent/attune-mcp"
state_dir=""
require_mcp_binary="true"

while IFS= read -r line; do
    [ -z "$line" ] && continue

    key="${line%%=*}"
    value="${line#*=}"

    case "$value" in
        \"*\") value="${value#\"}"; value="${value%\"}" ;;
        \'*\') value="${value#\'}"; value="${value%\'}" ;;
    esac

    case "$key" in
        command) command="$value" ;;
        working_dir) working_dir="$value" ;;
        mcp_command) mcp_command="$value" ;;
        state_dir) state_dir="$value" ;;
        require_mcp_binary) require_mcp_binary="$value" ;;
    esac
done

case "$require_mcp_binary" in
    true|True|TRUE|yes|Yes|YES|1) require_mcp_binary="true" ;;
    *) require_mcp_binary="false" ;;
esac

if [ -z "$command" ]; then
    echo "ERROR: command parameter is required" >&2
    exit 1
fi

if [ -z "${ATTUNE_API_TOKEN:-}" ]; then
    echo "ERROR: ATTUNE_API_TOKEN is required for execution-scoped MCP access" >&2
    exit 1
fi

if [ -z "${ATTUNE_API_URL:-}" ]; then
    echo "ERROR: ATTUNE_API_URL is required for attune-mcp" >&2
    exit 1
fi

if [ -n "$working_dir" ]; then
    if [ ! -d "$working_dir" ]; then
        echo "ERROR: working_dir does not exist: $working_dir" >&2
        exit 1
    fi
    cd "$working_dir"
fi

if [ -z "$state_dir" ] && [ -n "${ATTUNE_ARTIFACTS_DIR:-}" ] && [ -n "${ATTUNE_EXEC_ID:-}" ]; then
    state_dir="${ATTUNE_ARTIFACTS_DIR%/}/agent/${ATTUNE_EXEC_ID}"
fi

if [ -n "$state_dir" ]; then
    mkdir -p "$state_dir"
    export ATTUNE_AGENT_STATE_DIR="$state_dir"
fi

export ATTUNE_MCP_COMMAND="$mcp_command"
export ATTUNE_MCP_TRANSPORT="stdio"

if [ "$require_mcp_binary" = "true" ] && [ ! -x "$ATTUNE_MCP_COMMAND" ]; then
    echo "ERROR: attune-mcp binary is not executable at $ATTUNE_MCP_COMMAND" >&2
    exit 1
fi

echo "Running agent command with execution-scoped MCP access" >&2
echo "ATTUNE_MCP_COMMAND=$ATTUNE_MCP_COMMAND" >&2

exec /bin/sh -c "$command"
