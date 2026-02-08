# Core Pack Actions

## Overview

All actions in the core pack follow Attune's secure-by-design architecture:
- **Parameter delivery:** stdin (JSON format) - never environment variables
- **Output format:** Explicitly declared (text, json, or yaml)
- **Output schema:** Describes structured data shape (json/yaml only)
- **Execution metadata:** Automatically captured (stdout/stderr/exit_code)

## Parameter Delivery Method

**All actions:**
- Read parameters from **stdin** as JSON
- Use `parameter_delivery: stdin` and `parameter_format: json` in their YAML definitions
- **DO NOT** use environment variables for parameters

## Output Format

**All actions must specify an `output_format`:**
- `text` - Plain text output (stored as-is, no parsing)
- `json` - JSON structured data (parsed into JSONB field)
- `yaml` - YAML structured data (parsed into JSONB field)

**Output schema:**
- Only applicable for `json` and `yaml` formats
- Describes the structure of data written to stdout
- **Should NOT include** stdout/stderr/exit_code (captured automatically)

## Environment Variables

### Standard Environment Variables (Provided by Worker)

The worker automatically provides these environment variables to all action executions:

| Variable | Description | Always Present |
|----------|-------------|----------------|
| `ATTUNE_ACTION` | Action ref (e.g., `core.http_request`) | ✅ Yes |
| `ATTUNE_EXEC_ID` | Execution database ID | ✅ Yes |
| `ATTUNE_API_TOKEN` | Execution-scoped API token | ✅ Yes |
| `ATTUNE_RULE` | Rule ref that triggered execution | ❌ Only if from rule |
| `ATTUNE_TRIGGER` | Trigger ref that caused enforcement | ❌ Only if from trigger |

**Use cases:**
- Logging with execution context
- Calling Attune API (using `ATTUNE_API_TOKEN`)
- Conditional logic based on rule/trigger
- Creating child executions
- Accessing secrets via API

**Example:**
```bash
#!/bin/bash
# Log with context
echo "[$ATTUNE_ACTION] [Exec: $ATTUNE_EXEC_ID] Processing..." >&2

# Call Attune API
curl -s -H "Authorization: Bearer $ATTUNE_API_TOKEN" \
    "$ATTUNE_API_URL/api/v1/executions/$ATTUNE_EXEC_ID"

# Conditional behavior
if [ -n "$ATTUNE_RULE" ]; then
    echo "Triggered by rule: $ATTUNE_RULE" >&2
fi
```

See [Execution Environment Variables](../../../docs/QUICKREF-execution-environment.md) for complete documentation.

### Custom Environment Variables (Optional)

Custom environment variables can be set via `execution.env_vars` field for:
- **Debug/logging controls** (e.g., `DEBUG=1`, `LOG_LEVEL=debug`)
- **Runtime configuration** (e.g., custom paths, feature flags)
- **Action-specific context** (non-sensitive execution context)

Environment variables should **NEVER** be used for:
- Action parameters (use stdin instead)
- Secrets or credentials (use `ATTUNE_API_TOKEN` to fetch from key vault)
- User-provided data (use stdin parameters)

## Implementation Patterns

### Bash/Shell Actions

Shell actions read JSON from stdin using `jq`:

```bash
#!/bin/bash
set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters using jq
PARAM1=$(echo "$INPUT" | jq -r '.param1 // "default_value"')
PARAM2=$(echo "$INPUT" | jq -r '.param2 // ""')

# Check for null values (optional parameters)
if [ -n "$PARAM2" ] && [ "$PARAM2" != "null" ]; then
    echo "Param2 provided: $PARAM2"
fi

# Use the parameters
echo "Param1: $PARAM1"
```

### Advanced Bash Actions

For more complex bash actions (like http_request.sh), use `curl` or other standard utilities:

```bash
#!/bin/bash
set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters
URL=$(echo "$INPUT" | jq -r '.url // ""')
METHOD=$(echo "$INPUT" | jq -r '.method // "GET"')

# Validate required parameters
if [ -z "$URL" ]; then
    echo "ERROR: url parameter is required" >&2
    exit 1
fi

# Make HTTP request with curl
RESPONSE=$(curl -s -X "$METHOD" "$URL")

# Output result as JSON
jq -n \
    --arg body "$RESPONSE" \
    --argjson success true \
    '{body: $body, success: $success}'
```

## Core Pack Actions

### Simple Actions

1. **echo.sh** - Outputs a message
2. **sleep.sh** - Pauses execution for a specified duration
3. **noop.sh** - Does nothing (useful for testing)

### HTTP Action

4. **http_request.sh** - Makes HTTP requests with authentication support (curl-based)

### Pack Management Actions (API Wrappers)

These actions wrap API endpoints and pass parameters to the Attune API:

5. **download_packs.sh** - Downloads packs from git/HTTP/registry
6. **build_pack_envs.sh** - Builds runtime environments for packs
7. **register_packs.sh** - Registers packs in the database
8. **get_pack_dependencies.sh** - Analyzes pack dependencies

## Testing Actions Locally

You can test actions locally by piping JSON to stdin:

```bash
# Test echo action
echo '{"message": "Hello from stdin!"}' | ./echo.sh

# Test echo with no message (outputs empty line)
echo '{}' | ./echo.sh

# Test sleep action
echo '{"seconds": 2, "message": "Sleeping..."}' | ./sleep.sh

# Test http_request action
echo '{"url": "https://api.github.com", "method": "GET"}' | ./http_request.sh

# Test with file input
cat params.json | ./echo.sh
```

## Migration Summary

**Before (using environment variables):**
```bash
MESSAGE="${ATTUNE_ACTION_MESSAGE:-}"
```

**After (using stdin JSON):**
```bash
INPUT=$(cat)
MESSAGE=$(echo "$INPUT" | jq -r '.message // ""')
```

## Security Benefits

1. **No process exposure** - Parameters never appear in `ps`, `/proc/<pid>/environ`
2. **Secure by default** - All actions use stdin, no special configuration needed
3. **Clear separation** - Action parameters vs. environment configuration
4. **Audit friendly** - All sensitive data flows through stdin, not environment

## YAML Configuration

All action YAML files explicitly declare parameter delivery and output format:

```yaml
name: example_action
ref: core.example_action
runner_type: shell
entry_point: example.sh

# Parameter delivery: stdin for secure parameter passing (no env vars)
parameter_delivery: stdin
parameter_format: json

# Output format: text, json, or yaml
output_format: text

parameters:
  type: object
  properties:
    message:
      type: string
      description: "Message to output (empty string if not provided)"
  required: []

# Output schema: not applicable for text output format
# For json/yaml formats, describe the structure of data your action outputs
# Do NOT include stdout/stderr/exit_code - those are captured automatically
# Do NOT include generic "status" or "result" wrappers - output your data directly
```

## Best Practices

### Parameters
1. **Always use stdin** for action parameters
2. **Use jq for bash** scripts to parse JSON
3. **Handle null values** - Use jq's `// "default"` operator to provide defaults
4. **Provide sensible defaults** - Use empty string, 0, false, or empty array/object as appropriate
5. **Validate required params** - Exit with error if required parameters are missing (when truly required)
6. **Mark secrets** - Use `secret: true` in YAML for sensitive parameters
7. **Never use env vars for parameters** - Parameters come from stdin, not environment

### Environment Variables
1. **Use standard ATTUNE_* variables** - Worker provides execution context
2. **Access API with ATTUNE_API_TOKEN** - Execution-scoped authentication
3. **Log with context** - Include `ATTUNE_ACTION` and `ATTUNE_EXEC_ID` in logs
4. **Custom env vars via execution.env_vars** - For debug flags and configuration only
5. **Never log ATTUNE_API_TOKEN** - Security sensitive
6. **Check ATTUNE_RULE/ATTUNE_TRIGGER** - Conditional behavior for automated vs manual
7. **Use env vars for runtime context** - Not for user data or parameters

### Output Format
1. **Specify output_format** - Always set to "text", "json", or "yaml"
2. **Use text for simple output** - Messages, logs, unstructured data
3. **Use json for structured data** - API responses, complex results
4. **Use yaml for readable config** - Human-readable structured output
5. **Define schema for structured output** - Only for json/yaml formats
6. **Don't include execution metadata** - No stdout/stderr/exit_code in schema
7. **Use stderr for errors** - Diagnostic messages go to stderr, not stdout
8. **Return proper exit codes** - 0 for success, non-zero for failure

## Dependencies

All core pack actions have **zero runtime dependencies**:
- **Bash actions**: Require `jq` (for JSON parsing) and `curl` (for HTTP requests)
- Both `jq` and `curl` are standard utilities available in all Attune worker containers
- **No Python, Node.js, or other runtime dependencies required**

## Execution Metadata (Automatic)

The following are **automatically captured** by the worker and should **NOT** be included in output schemas:

- `stdout` - Raw standard output (captured as-is)
- `stderr` - Standard error output (written to log file)
- `exit_code` - Process exit code (0 = success)
- `duration_ms` - Execution duration in milliseconds

These are execution system concerns, not action output concerns.

## Example: Using Environment Variables and Parameters

```bash
#!/bin/bash
set -e
set -o pipefail

# Standard environment variables (provided by worker)
echo "[$ATTUNE_ACTION] [Exec: $ATTUNE_EXEC_ID] Starting execution" >&2

# Read action parameters from stdin
INPUT=$(cat)
URL=$(echo "$INPUT" | jq -r '.url // ""')

if [ -z "$URL" ]; then
    echo "ERROR: url parameter is required" >&2
    exit 1
fi

# Log execution context
if [ -n "$ATTUNE_RULE" ]; then
    echo "Triggered by rule: $ATTUNE_RULE" >&2
fi

# Make request
RESPONSE=$(curl -s "$URL")

# Output result
echo "$RESPONSE"

echo "[$ATTUNE_ACTION] [Exec: $ATTUNE_EXEC_ID] Completed successfully" >&2
exit 0
```

## Future Considerations

- Consider adding a bash library for common parameter parsing patterns
- Add parameter validation helpers
- Create templates for new actions in different languages
- Add output schema validation tooling
- Add helper functions for API interaction using ATTUNE_API_TOKEN