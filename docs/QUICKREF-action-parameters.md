# Quick Reference: Action Parameter Handling

**Last Updated:** 2026-02-07  
**Status:** Current standard for all actions

## TL;DR

- ✅ **DO:** Read action parameters from **stdin as JSON**
- ❌ **DON'T:** Use environment variables for action parameters
- 💡 **Environment variables** are for debug/config only (e.g., `DEBUG=1`)

## Secure Parameter Delivery

All action parameters are delivered via **stdin** in **JSON format** to prevent exposure in process listings.

### YAML Configuration

```yaml
name: my_action
ref: mypack.my_action
runner_type: shell  # or python, nodejs
entry_point: my_action.sh

# Always specify stdin parameter delivery
parameter_delivery: stdin
parameter_format: json

parameters:
  type: object
  properties:
    message:
      type: string
      default: "Hello"
    api_key:
      type: string
      secret: true  # Mark sensitive parameters
```

## Implementation Patterns

### Bash/Shell Actions

```bash
#!/bin/bash
set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters with jq (includes default values)
MESSAGE=$(echo "$INPUT" | jq -r '.message // "Hello, World!"')
API_KEY=$(echo "$INPUT" | jq -r '.api_key // ""')
COUNT=$(echo "$INPUT" | jq -r '.count // 1')
ENABLED=$(echo "$INPUT" | jq -r '.enabled // false')

# Handle optional parameters (check for null)
if [ -n "$API_KEY" ] && [ "$API_KEY" != "null" ]; then
    echo "API key provided"
fi

# Use parameters
echo "Message: $MESSAGE"
echo "Count: $COUNT"
```

### Python Actions

```python
#!/usr/bin/env python3
import json
import sys
from typing import Dict, Any

def read_parameters() -> Dict[str, Any]:
    """Read and parse JSON parameters from stdin."""
    try:
        input_data = sys.stdin.read()
        if not input_data:
            return {}
        return json.loads(input_data)
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON input: {e}", file=sys.stderr)
        sys.exit(1)

def main():
    # Read parameters
    params = read_parameters()
    
    # Access parameters with defaults
    message = params.get('message', 'Hello, World!')
    api_key = params.get('api_key')
    count = params.get('count', 1)
    enabled = params.get('enabled', False)
    
    # Validate required parameters
    if not params.get('url'):
        print("ERROR: 'url' parameter is required", file=sys.stderr)
        sys.exit(1)
    
    # Use parameters
    print(f"Message: {message}")
    print(f"Count: {count}")
    
    # Output result as JSON
    result = {"status": "success", "message": message}
    print(json.dumps(result))

if __name__ == "__main__":
    main()
```

### Node.js Actions

```javascript
#!/usr/bin/env node

const readline = require('readline');

async function readParameters() {
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
        terminal: false
    });

    let input = '';
    for await (const line of rl) {
        input += line;
    }

    try {
        return JSON.parse(input || '{}');
    } catch (err) {
        console.error('ERROR: Invalid JSON input:', err.message);
        process.exit(1);
    }
}

async function main() {
    // Read parameters
    const params = await readParameters();
    
    // Access parameters with defaults
    const message = params.message || 'Hello, World!';
    const apiKey = params.api_key;
    const count = params.count || 1;
    const enabled = params.enabled || false;
    
    // Use parameters
    console.log(`Message: ${message}`);
    console.log(`Count: ${count}`);
    
    // Output result as JSON
    const result = { status: 'success', message };
    console.log(JSON.stringify(result, null, 2));
}

main().catch(err => {
    console.error('ERROR:', err.message);
    process.exit(1);
});
```

## Testing Actions Locally

```bash
# Test with specific parameters
echo '{"message": "Test", "count": 5}' | ./my_action.sh

# Test with defaults (empty JSON)
echo '{}' | ./my_action.sh

# Test with file input
cat test-params.json | ./my_action.sh

# Test Python action
echo '{"url": "https://api.example.com"}' | python3 my_action.py

# Test with multiple parameters including secrets
echo '{"url": "https://api.example.com", "api_key": "secret123"}' | ./my_action.sh
```

## Environment Variables Usage

### ✅ Correct Usage (Configuration/Debug)

```bash
# Debug logging control
DEBUG=1 ./my_action.sh

# Log level control
LOG_LEVEL=debug ./my_action.sh

# System configuration
PATH=/usr/local/bin:$PATH ./my_action.sh
```

### ❌ Incorrect Usage (Parameters)

```bash
# NEVER do this - parameters should come from stdin
ATTUNE_ACTION_MESSAGE="Hello" ./my_action.sh  # ❌ WRONG
API_KEY="secret" ./my_action.sh               # ❌ WRONG - exposed in ps!
```

## Common Patterns

### Required Parameters

```bash
# Bash
URL=$(echo "$INPUT" | jq -r '.url // ""')
if [ -z "$URL" ] || [ "$URL" == "null" ]; then
    echo "ERROR: 'url' parameter is required" >&2
    exit 1
fi
```

```python
# Python
if not params.get('url'):
    print("ERROR: 'url' parameter is required", file=sys.stderr)
    sys.exit(1)
```

### Optional Parameters with Null Check

```bash
# Bash
API_KEY=$(echo "$INPUT" | jq -r '.api_key // ""')
if [ -n "$API_KEY" ] && [ "$API_KEY" != "null" ]; then
    # Use API key
    echo "Authenticated request"
fi
```

```python
# Python
api_key = params.get('api_key')
if api_key:
    # Use API key
    print("Authenticated request")
```

### Boolean Parameters

```bash
# Bash - jq outputs lowercase 'true'/'false'
ENABLED=$(echo "$INPUT" | jq -r '.enabled // false')
if [ "$ENABLED" = "true" ]; then
    echo "Feature enabled"
fi
```

```python
# Python - native boolean
enabled = params.get('enabled', False)
if enabled:
    print("Feature enabled")
```

### Array Parameters

```bash
# Bash
ITEMS=$(echo "$INPUT" | jq -c '.items // []')
ITEM_COUNT=$(echo "$ITEMS" | jq 'length')
echo "Processing $ITEM_COUNT items"
```

```python
# Python
items = params.get('items', [])
print(f"Processing {len(items)} items")
for item in items:
    print(f"  - {item}")
```

### Object Parameters

```bash
# Bash
HEADERS=$(echo "$INPUT" | jq -c '.headers // {}')
# Extract specific header
AUTH=$(echo "$HEADERS" | jq -r '.Authorization // ""')
```

```python
# Python
headers = params.get('headers', {})
auth = headers.get('Authorization')
```

## Security Best Practices

1. **Never log sensitive parameters** - Avoid printing secrets to stdout/stderr
2. **Mark secrets in YAML** - Use `secret: true` for sensitive parameters
3. **No parameter echoing** - Don't echo input JSON back in error messages
4. **Clear error messages** - Don't include parameter values in errors
5. **Validate input** - Check parameter types and ranges

### Example: Safe Error Handling

```python
# ❌ BAD - exposes parameter value
if not valid_url(url):
    print(f"ERROR: Invalid URL: {url}", file=sys.stderr)

# ✅ GOOD - generic error message
if not valid_url(url):
    print("ERROR: 'url' parameter must be a valid HTTP/HTTPS URL", file=sys.stderr)
```

## Migration from Environment Variables

If you have existing actions using environment variables:

```bash
# OLD (environment variables)
MESSAGE="${ATTUNE_ACTION_MESSAGE:-Hello}"
COUNT="${ATTUNE_ACTION_COUNT:-1}"

# NEW (stdin JSON)
INPUT=$(cat)
MESSAGE=$(echo "$INPUT" | jq -r '.message // "Hello"')
COUNT=$(echo "$INPUT" | jq -r '.count // 1')
```

```python
# OLD (environment variables)
import os
message = os.environ.get('ATTUNE_ACTION_MESSAGE', 'Hello')
count = int(os.environ.get('ATTUNE_ACTION_COUNT', '1'))

# NEW (stdin JSON)
import json, sys
params = json.loads(sys.stdin.read() or '{}')
message = params.get('message', 'Hello')
count = params.get('count', 1)
```

## Dependencies

- **Bash**: Requires `jq` (installed in all Attune worker containers)
- **Python**: Standard library only (`json`, `sys`)
- **Node.js**: Built-in modules only (`readline`)

## References

- [Core Pack Actions README](../packs/core/actions/README.md) - Reference implementations
- [Secure Action Parameter Handling Formats](zed:///agent/thread/e68272e6-a5a2-4d88-aaca-a9009f33a812) - Design document
- [Worker Service Architecture](./architecture/worker-service.md) - Parameter delivery details

## See Also

- Environment variables via `execution.env_vars` (for runtime context)
- Secret management via `key` table (for encrypted storage)
- Parameter validation in action YAML schemas