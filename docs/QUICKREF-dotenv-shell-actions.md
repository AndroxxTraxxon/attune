# Quick Reference: DOTENV Shell Actions Pattern

**Purpose:** Standard pattern for writing portable shell actions without external dependencies like `jq`.

## Core Principles

1. **Use POSIX shell** (`#!/bin/sh`), not bash
2. **Read parameters in DOTENV format** from stdin until EOF
3. **No external JSON parsers** (jq, yq, etc.)
4. **Minimal dependencies** (only POSIX utilities + curl)

## Complete Template

```sh
#!/bin/sh
# Action Name - Core Pack
# Brief description of what this action does
#
# This script uses pure POSIX shell without external dependencies like jq.
# It reads parameters in DOTENV format from stdin until EOF.

set -e

# Initialize variables with defaults
param1=""
param2="default_value"
bool_param="false"
numeric_param="0"

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
        param1)
            param1="$value"
            ;;
        param2)
            param2="$value"
            ;;
        bool_param)
            bool_param="$value"
            ;;
        numeric_param)
            numeric_param="$value"
            ;;
    esac
done

# Normalize boolean values
case "$bool_param" in
    true|True|TRUE|yes|Yes|YES|1) bool_param="true" ;;
    *) bool_param="false" ;;
esac

# Validate numeric parameters
case "$numeric_param" in
    ''|*[!0-9]*)
        echo "ERROR: numeric_param must be a positive integer" >&2
        exit 1
        ;;
esac

# Validate required parameters
if [ -z "$param1" ]; then
    echo "ERROR: param1 is required" >&2
    exit 1
fi

# Action logic goes here
echo "Processing with param1=$param1, param2=$param2"

# Exit successfully
exit 0
```

## YAML Metadata Configuration

```yaml
ref: core.action_name
label: "Action Name"
description: "Brief description"
enabled: true
runner_type: shell
entry_point: action_name.sh

# IMPORTANT: Use dotenv format for POSIX shell compatibility
parameter_delivery: stdin
parameter_format: dotenv

# Output format (text or json)
output_format: text

parameters:
  type: object
  properties:
    param1:
      type: string
      description: "First parameter"
    param2:
      type: string
      description: "Second parameter"
      default: "default_value"
    bool_param:
      type: boolean
      description: "Boolean parameter"
      default: false
  required:
    - param1
```

## Common Patterns

### 1. Parameter Parsing

**Read until EOF:**
```sh
while IFS= read -r line; do
    [ -z "$line" ] && continue
    # ... process line
done
```

**Extract key-value:**
```sh
key="${line%%=*}"     # Everything before first =
value="${line#*=}"    # Everything after first =
```

**Remove quotes:**
```sh
case "$value" in
    \"*\") value="${value#\"}"; value="${value%\"}" ;;
    \'*\') value="${value#\'}"; value="${value%\'}" ;;
esac
```

### 2. Boolean Normalization

```sh
case "$bool_param" in
    true|True|TRUE|yes|Yes|YES|1) bool_param="true" ;;
    *) bool_param="false" ;;
esac
```

### 3. Numeric Validation

```sh
case "$number" in
    ''|*[!0-9]*)
        echo "ERROR: must be a number" >&2
        exit 1
        ;;
esac
```

### 4. JSON Output (without jq)

**Escape special characters:**
```sh
escaped=$(printf '%s' "$value" | sed 's/\\/\\\\/g; s/"/\\"/g')
```

**Build JSON:**
```sh
cat <<EOF
{
  "field": "$escaped",
  "boolean": $bool_value,
  "number": $number
}
EOF
```

### 5. Making HTTP Requests

**With curl and temp files:**
```sh
temp_response=$(mktemp)
cleanup() { rm -f "$temp_response"; }
trap cleanup EXIT

http_code=$(curl -X POST \
    -H "Content-Type: application/json" \
    ${api_token:+-H "Authorization: Bearer ${api_token}"} \
    -d "$request_body" \
    -s \
    -w "%{http_code}" \
    -o "$temp_response" \
    --max-time 60 \
    "${api_url}/api/v1/endpoint" 2>/dev/null || echo "000")

if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
    cat "$temp_response"
    exit 0
else
    echo "ERROR: API call failed (HTTP $http_code)" >&2
    exit 1
fi
```

### 6. Extracting JSON Fields (simple cases)

**Extract field value:**
```sh
case "$response" in
    *'"field":'*)
        value=$(printf '%s' "$response" | sed -n 's/.*"field":\s*"\([^"]*\)".*/\1/p')
        ;;
esac
```

**Note:** For complex JSON, consider having the API return the exact format needed.

## Anti-Patterns (DO NOT DO)

❌ **Using jq:**
```sh
value=$(echo "$json" | jq -r '.field')  # NO!
```

❌ **Using bash-specific features:**
```sh
#!/bin/bash  # NO! Use #!/bin/sh
[[ "$var" == "value" ]]  # NO! Use [ "$var" = "value" ]
```

❌ **Reading JSON directly from stdin:**
```yaml
parameter_format: json  # NO! Use dotenv
```

❌ **Using Python/Node.js in core pack:**
```yaml
runner_type: python  # NO! Use shell for core pack
```

## Testing Checklist

- [ ] Script has `#!/bin/sh` shebang
- [ ] Script is executable (`chmod +x`)
- [ ] All parameters have defaults or validation
- [ ] Boolean values are normalized
- [ ] Numeric values are validated
- [ ] Required parameters are checked
- [ ] Error messages go to stderr (`>&2`)
- [ ] Successful output goes to stdout
- [ ] Temp files are cleaned up (trap handler)
- [ ] YAML has `parameter_format: dotenv`
- [ ] YAML has `runner_type: shell`
- [ ] No `jq`, `yq`, or bash-isms used
- [ ] Works on Alpine Linux (minimal environment)

## Examples from Core Pack

### Simple Action (echo.sh)
- Minimal parameter parsing
- Single string parameter
- Text output

### Complex Action (http_request.sh)
- Multiple parameters (headers, query params)
- HTTP client implementation
- JSON output construction
- Error handling

### API Wrapper (register_packs.sh)
- JSON request body construction
- API authentication
- Response parsing
- Structured error messages

## DOTENV Format Specification

**Format:** Each parameter on a new line as `key=value`

**Example:**
```
param1="string value"
param2=42
bool_param=true
```

**Key Rules:**
- Parameters are delivered via stdin; the script reads until EOF (stdin is closed after delivery)
- Values may be quoted (single or double quotes)
- Empty lines are skipped
- No multiline values (use base64 if needed)
- Array/object parameters passed as JSON strings

## When to Use This Pattern

✅ **Use DOTENV shell pattern for:**
- Core pack actions
- Simple utility actions
- Actions that need maximum portability
- Actions that run in minimal containers
- Actions that don't need complex JSON parsing

❌ **Consider other runtimes if you need:**
- Complex JSON manipulation
- External libraries (AWS SDK, etc.)
- Advanced string processing
- Parallel processing
- Language-specific features

## Further Reading

- `packs/core/actions/echo.sh` - Simplest example
- `packs/core/actions/http_request.sh` - Complex example
- `packs/core/actions/register_packs.sh` - API wrapper example
- `docs/pack-structure.md` - Pack development guide