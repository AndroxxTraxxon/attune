#!/bin/sh
# Prompt Copilot - Examples Pack
# Runs a one-shot GitHub Copilot CLI prompt with the local attune-mcp binary
# wired into Copilot's MCP config, so the agent can call Attune tools using the
# current execution's scoped API token. Captures Copilot stdout and emits a
# JSON envelope on this action's stdout for downstream workflow tasks.

set -eu

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

json_escape() {
  # POSIX-portable JSON string escaper. Escapes \ " and ASCII control chars.
  awk 'BEGIN{
         for (i=0;i<32;i++) ctrl[sprintf("%c",i)]=sprintf("\\u%04x",i);
       }
       {
         line=$0;
         out="";
         n=length(line);
         for (i=1;i<=n;i++) {
           c=substr(line,i,1);
           if (c=="\\")      out=out "\\\\";
           else if (c=="\"") out=out "\\\"";
           else if (c in ctrl) out=out ctrl[c];
           else              out=out c;
         }
         printf "%s", out;
         if (NR>0) printf "\\n";
       }' "$1"
}

resolve_pack_copilot() {
  runtime_envs_dir="${ATTUNE_RUNTIME_ENVS_DIR:-/opt/attune/runtime_envs}"
  pack_ref="${ATTUNE_PACK_REF:-${ATTUNE_ACTION%%.*}}"

  for runtime_name in node nodejs node.js; do
    candidate="$runtime_envs_dir/$pack_ref/$runtime_name/node_modules/.bin/copilot"
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

extract_json_string_value() {
  tr -d '\n' | sed -nE 's/.*"value"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p'
}

params_file="$(mktemp)"
mcp_dir="$(mktemp -d)"
out_dir="$(mktemp -d)"
cleanup() {
  rm -f "$params_file"
  rm -rf "$mcp_dir"
  rm -rf "$out_dir"
}
trap cleanup EXIT INT TERM

cat >"$params_file"

# Locate node so we can parse JSON parameters robustly. The action declares
# required_worker_runtimes.node so node is guaranteed to be installed on the
# worker, but we may need to find it via the pack's runtime env if it's not on
# PATH (e.g. when running on a slim base image).
resolve_node() {
  if command -v node >/dev/null 2>&1; then
    command -v node
    return 0
  fi
  for candidate in \
      "${ATTUNE_RUNTIME_ENVS_DIR:-/opt/attune/runtime_envs}/${ATTUNE_PACK_REF:-${ATTUNE_ACTION%%.*}}/node/node_modules/.bin/node" \
      /usr/bin/node /usr/local/bin/node; do
    [ -x "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
  done
  return 1
}

NODE_BIN="$(resolve_node)" || fail "node interpreter not found; cannot parse JSON parameters"

# Extract a single string field from the JSON params on stdin.
get_param() {
  "$NODE_BIN" -e '
    let raw = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", c => raw += c);
    process.stdin.on("end", () => {
      try {
        const d = JSON.parse(raw);
        const v = d[process.argv[1]];
        if (v === undefined || v === null) { process.stdout.write(""); }
        else if (typeof v === "string") { process.stdout.write(v); }
        else { process.stdout.write(String(v)); }
      } catch (e) {
        process.stderr.write("param parse error: " + e.message + "\n");
        process.exit(2);
      }
    });
  ' "$1" <"$params_file"
}

prompt="$(get_param prompt)"
copilot_command="$(get_param copilot_command)"
copilot_token="$(get_param copilot_token)"
copilot_token_key_ref="$(get_param copilot_token_key_ref)"
mcp_command="$(get_param mcp_command)"
attune_command="$(get_param attune_command)"
disable_builtin_mcps="$(get_param disable_builtin_mcps)"
working_dir="$(get_param working_dir)"

# Apply defaults for parameters that weren't set in the JSON input.
[ -n "$mcp_command" ] || mcp_command="/opt/attune/agent/attune-mcp"
[ -n "$attune_command" ] || attune_command="/opt/attune/agent/attune"
[ -n "$disable_builtin_mcps" ] || disable_builtin_mcps="true"

[ -n "$prompt" ] || fail "prompt parameter is required"
[ -n "${ATTUNE_API_URL:-}" ] || fail "ATTUNE_API_URL is required for Attune MCP access"
[ -n "${ATTUNE_API_TOKEN:-}" ] || fail "ATTUNE_API_TOKEN is required for Attune MCP access"

if [ -n "$working_dir" ]; then
  [ -d "$working_dir" ] || fail "working_dir does not exist: $working_dir"
  cd "$working_dir"
fi

if [ -n "$copilot_token" ]; then
  export GH_TOKEN="$copilot_token"
fi

if [ -z "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ] && [ -n "$copilot_token_key_ref" ]; then
  if [ -x "$attune_command" ]; then
    key_response="$("$attune_command" --output json key show "$copilot_token_key_ref" --decrypt)" \
      || fail "failed to fetch key '$copilot_token_key_ref' using $attune_command"
  elif command -v curl >/dev/null 2>&1; then
    key_response="$(
      curl -fsS \
        -H "Authorization: Bearer $ATTUNE_API_TOKEN" \
        "${ATTUNE_API_URL%/}/api/v1/keys/$copilot_token_key_ref"
    )" || fail "failed to fetch key '$copilot_token_key_ref' from Attune"
  else
    fail "cannot fetch copilot_token_key_ref; neither $attune_command nor curl is available"
  fi

  key_value="$(printf '%s' "$key_response" | extract_json_string_value)"
  [ -n "$key_value" ] || fail "key '$copilot_token_key_ref' did not return a string value"
  export GH_TOKEN="$key_value"
fi

if [ -z "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]; then
  fail "Provide copilot_token, copilot_token_key_ref, or pre-set GH_TOKEN/GITHUB_TOKEN for Copilot CLI auth"
fi

if [ -z "$copilot_command" ]; then
  if resolved="$(resolve_pack_copilot 2>/dev/null)"; then
    copilot_command="$resolved"
  else
    copilot_command="copilot"
  fi
fi

if ! command -v "$copilot_command" >/dev/null 2>&1 && [ ! -x "$copilot_command" ]; then
  fail "Copilot CLI executable not found: $copilot_command"
fi

[ -x "$mcp_command" ] || fail "attune-mcp binary is not executable at $mcp_command"

mcp_config_file="$mcp_dir/config.json"
mcp_command_escaped=$(printf '%s' "$mcp_command"      | sed 's/\\/\\\\/g; s/"/\\"/g')
api_url_escaped=$(printf '%s'      "$ATTUNE_API_URL"   | sed 's/\\/\\\\/g; s/"/\\"/g')
api_token_escaped=$(printf '%s'    "$ATTUNE_API_TOKEN" | sed 's/\\/\\\\/g; s/"/\\"/g')
cat >"$mcp_config_file" <<EOF
{
  "mcpServers": {
    "attune": {
      "type": "stdio",
      "command": "$mcp_command_escaped",
      "args": [],
      "env": {
        "ATTUNE_API_URL": "$api_url_escaped",
        "ATTUNE_API_TOKEN": "$api_token_escaped"
      },
      "tools": ["*"]
    }
  }
}
EOF

echo "Running GitHub Copilot CLI with execution-scoped Attune MCP" >&2

set -- -p "$prompt" --allow-all-tools --allow-all-paths --additional-mcp-config "@$mcp_config_file"
case "$(printf '%s' "$disable_builtin_mcps" | tr '[:upper:]' '[:lower:]')" in
  true|yes|1) set -- "$@" --disable-builtin-mcps ;;
esac

# Capture Copilot stdout to a file so we can wrap it in a JSON envelope on
# this action's stdout. Stderr passes through untouched for diagnostics.
copilot_stdout="$out_dir/copilot.stdout"
exit_code=0
"$copilot_command" "$@" >"$copilot_stdout" || exit_code=$?

# Decide whether Copilot's stdout is valid JSON. Prefer python3 if present
# because it's a robust parser; otherwise fall back to a heuristic and emit a
# raw-text envelope. Either way, the action's own stdout is always valid JSON.
emit_json_envelope() {
  payload_json="$1"
  raw_text="$2"
  printf '{"final_output":%s,"raw_text":%s,"exit_code":%d}\n' \
    "$payload_json" "$raw_text" "$exit_code"
}

if command -v python3 >/dev/null 2>&1; then
  python3 - "$copilot_stdout" "$exit_code" <<'PYEOF'
import json
import re
import sys

stdout_path, exit_code = sys.argv[1], int(sys.argv[2])
with open(stdout_path, "r", encoding="utf-8", errors="replace") as fh:
    raw = fh.read()


def try_parse(text):
    text = text.strip()
    if not text:
        return None
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return None


def extract_last_json_object(text):
    """Scan from the right for a balanced top-level {...} or [...] block."""
    if not text:
        return None
    # Strip code fences ```json ... ```
    fenced = re.search(
        r"```(?:json)?\s*(\{.*?\}|\[.*?\])\s*```",
        text,
        re.DOTALL | re.IGNORECASE,
    )
    if fenced:
        parsed = try_parse(fenced.group(1))
        if parsed is not None:
            return parsed
    # Fallback: find the last top-level balanced { ... } block.
    last_close = max(text.rfind("}"), text.rfind("]"))
    while last_close != -1:
        opener = "{" if text[last_close] == "}" else "["
        closer = text[last_close]
        depth = 0
        i = last_close
        start = -1
        while i >= 0:
            ch = text[i]
            if ch == closer:
                depth += 1
            elif ch == opener:
                depth -= 1
                if depth == 0:
                    start = i
                    break
            i -= 1
        if start != -1:
            parsed = try_parse(text[start : last_close + 1])
            if parsed is not None and isinstance(parsed, (dict, list)):
                return parsed
        # Try an earlier closing bracket.
        last_close = max(text.rfind("}", 0, last_close), text.rfind("]", 0, last_close))
    return None


parsed = try_parse(raw) or extract_last_json_object(raw)

if parsed is not None and isinstance(parsed, (dict, list)):
    final = parsed
    raw_text = False
else:
    final = {"text": raw}
    raw_text = True

json.dump(
    {"final_output": final, "raw_text": raw_text, "exit_code": exit_code},
    sys.stdout,
)
sys.stdout.write("\n")
PYEOF
else
  # Heuristic fallback: treat as JSON only if first non-space char is { or [.
  first_char=$(awk 'BEGIN{ORS=""} {gsub(/^[[:space:]]+/,""); print substr($0,1,1); exit}' "$copilot_stdout")
  if [ "$first_char" = "{" ] || [ "$first_char" = "[" ]; then
    payload="$(cat "$copilot_stdout")"
    emit_json_envelope "$payload" "false"
  else
    escaped=$(json_escape "$copilot_stdout")
    emit_json_envelope "{\"text\":\"$escaped\"}" "true"
  fi
fi

exit "$exit_code"
