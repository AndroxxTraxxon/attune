# Parameter Delivery Methods

**Last Updated**: 2025-02-05  
**Status**: Active Feature

---

## Overview

Attune provides secure parameter passing for actions with two delivery methods: **stdin** (default) and **file** (for large payloads). This document describes parameter delivery, formats, and best practices.

**Key Design Principle**: Action parameters and environment variables are completely separate:
- **Parameters** - Data the action operates on (always secure: stdin or file)
- **Environment Variables** - Execution context/configuration (set as env vars, stored in `execution.env_vars`)

---

## Security by Design

### Parameters Are Always Secure

Action parameters are **never** passed as environment variables. They are always delivered via:
- **stdin** (default) - Secure, not visible in process listings
- **file** - Secure temporary file with restrictive permissions (0400)

This ensures parameters (including sensitive data like passwords, API keys, tokens) are never exposed in process listings.

### Environment Variables Are Separate

Environment variables provide execution context and configuration:
- Stored in `execution.env_vars` (JSONB key-value pairs)
- Set as environment variables by the worker
- Examples: `ATTUNE_EXECUTION_ID`, custom config values, feature flags
- Typically non-sensitive (visible in process environment)

---

## Parameter Delivery Methods

### 1. Standard Input (`stdin`)

**Security**: ✅ **High** - Not visible in process listings  
**Use Case**: Sensitive data, structured parameters, credentials

Parameters are serialized in the specified format and passed via stdin. A delimiter `---ATTUNE_PARAMS_END---` separates parameters from secrets.

**Example** (this is the default):
```yaml
parameter_delivery: stdin
parameter_format: json
```

**Environment variables set**:
- `ATTUNE_PARAMETER_DELIVERY=stdin`
- `ATTUNE_PARAMETER_FORMAT=json`

**Stdin content (JSON format)**:
```
{"message":"Hello","count":42,"enabled":true}
---ATTUNE_PARAMS_END---
{"api_key":"secret123","db_password":"pass456"}
```

**Python script example**:
```python
#!/usr/bin/env python3
import sys
import json

def read_stdin_params():
    """Read parameters and secrets from stdin."""
    content = sys.stdin.read()
    parts = content.split('---ATTUNE_PARAMS_END---')
    
    # Parse parameters
    params = json.loads(parts[0].strip()) if parts[0].strip() else {}
    
    # Parse secrets (if present)
    secrets = {}
    if len(parts) > 1 and parts[1].strip():
        secrets = json.loads(parts[1].strip())
    
    return params, secrets

params, secrets = read_stdin_params()
message = params.get('message', 'default')
api_key = secrets.get('api_key')
print(f"Message: {message}")
```

**Shell script example**:
```bash
#!/bin/bash

# Read parameters from stdin (JSON format)
read -r PARAMS_JSON
# Parse JSON (requires jq)
MESSAGE=$(echo "$PARAMS_JSON" | jq -r '.message // "default"')
COUNT=$(echo "$PARAMS_JSON" | jq -r '.count // 0')

echo "Message: $MESSAGE, Count: $COUNT"
```

---

### 2. Temporary File (`file`)

**Security**: ✅ **High** - File has restrictive permissions (owner read-only)  
**Use Case**: Large parameter payloads, sensitive data, actions that need random access to parameters

Parameters are written to a temporary file with restrictive permissions (`0400` on Unix). The file path is provided via the `ATTUNE_PARAMETER_FILE` environment variable.

**Example**:
```yaml
# Explicitly set to file
parameter_delivery: file
parameter_format: yaml
```

**Environment variables set**:
- `ATTUNE_PARAMETER_DELIVERY=file`
- `ATTUNE_PARAMETER_FORMAT=yaml`
- `ATTUNE_PARAMETER_FILE=/tmp/attune-params-abc123.yaml`

**File content (YAML format)**:
```yaml
message: Hello
count: 42
enabled: true
```

**Python script example**:
```python
#!/usr/bin/env python3
import os
import yaml

def read_file_params():
    """Read parameters from temporary file."""
    param_file = os.environ.get('ATTUNE_PARAMETER_FILE')
    if not param_file:
        return {}
    
    with open(param_file, 'r') as f:
        return yaml.safe_load(f)

params = read_file_params()
message = params.get('message', 'default')
count = params.get('count', 0)
print(f"Message: {message}, Count: {count}")
```

**Shell script example**:
```bash
#!/bin/bash

# Read from parameter file
PARAM_FILE="${ATTUNE_PARAMETER_FILE}"
if [ -f "$PARAM_FILE" ]; then
    # Parse YAML (requires yq or similar)
    MESSAGE=$(yq eval '.message // "default"' "$PARAM_FILE")
    COUNT=$(yq eval '.count // 0' "$PARAM_FILE")
    echo "Message: $MESSAGE, Count: $COUNT"
fi
```

**Note**: The temporary file is automatically deleted after the action completes.

---

## Parameter Formats

### 1. JSON (`json`)

**Format**: JSON object  
**Best For**: Structured data, Python/Node.js actions, complex parameters  
**Type Preservation**: Yes (strings, numbers, booleans, arrays, objects)

**Example**:
```json
{
  "message": "Hello, World!",
  "count": 42,
  "enabled": true,
  "tags": ["prod", "api"],
  "config": {
    "timeout": 30,
    "retries": 3
  }
}
```

---

### 2. Dotenv (`dotenv`)

**Format**: `KEY='VALUE'` (one per line)  
**Best For**: Simple key-value pairs when needed  
**Type Preservation**: No (all values are strings)

**Example**:
```
MESSAGE='Hello, World!'
COUNT='42'
ENABLED='true'
```

**Escaping**: Single quotes in values are escaped as `'\''`

---

### 3. YAML (`yaml`)

**Format**: YAML document  
**Best For**: Human-readable structured data, complex configurations  
**Type Preservation**: Yes (strings, numbers, booleans, arrays, objects)

**Example**:
```yaml
message: Hello, World!
count: 42
enabled: true
tags:
  - prod
  - api
config:
  timeout: 30
  retries: 3
```

---

## Configuration in Action YAML

Add these fields to your action metadata file:

```yaml
name: my_action
ref: mypack.my_action
description: "My secure action"
runner_type: python
entry_point: my_action.py

# Parameter delivery configuration (optional - these are the defaults)
# parameter_delivery: stdin   # Options: stdin, file (default: stdin)
# parameter_format: json      # Options: json, dotenv, yaml (default: json)

parameters:
  type: object
  properties:
    api_key:
      type: string
      description: "API key for authentication"
      secret: true          # Mark sensitive parameters
    message:
      type: string
      description: "Message to process"
```

---

## Best Practices

### 1. Choose the Right Delivery Method

| Scenario | Recommended Delivery | Recommended Format |
|----------|---------------------|-------------------|
| Most actions (default) | `stdin` | `json` |
| Sensitive credentials | `stdin` (default) | `json` (default) |
| Large parameter payloads (>1MB) | `file` | `json` or `yaml` |
| Complex structured data | `stdin` (default) | `json` (default) |
| Shell scripts | `stdin` (default) | `json` or `dotenv` |
| Python/Node.js actions | `stdin` (default) | `json` (default) |

### 2. Mark Sensitive Parameters

Always mark sensitive parameters with `secret: true` in the parameter schema:

```yaml
parameters:
  type: object
  properties:
    password:
      type: string
      secret: true
    api_token:
      type: string
      secret: true
```

### 3. Handle Missing Parameters Gracefully

```python
# Python example
params = read_params()
api_key = params.get('api_key')
if not api_key:
    print("ERROR: api_key parameter is required", file=sys.stderr)
    sys.exit(1)
```

```bash
# Shell example
if [ -z "$ATTUNE_ACTION_API_KEY" ]; then
    echo "ERROR: api_key parameter is required" >&2
    exit 1
fi
```

### 4. Validate Parameter Format

Check the `ATTUNE_PARAMETER_DELIVERY` environment variable to determine how parameters were delivered:

```python
import os

delivery_method = os.environ.get('ATTUNE_PARAMETER_DELIVERY', 'env')
param_format = os.environ.get('ATTUNE_PARAMETER_FORMAT', 'dotenv')

if delivery_method == 'env':
    # Read from environment variables
    params = read_env_params()
elif delivery_method == 'stdin':
    # Read from stdin
    params = read_stdin_params()
elif delivery_method == 'file':
    # Read from file
    params = read_file_params()
```

### 5. Clean Up Sensitive Data

For file-based delivery, the system automatically deletes the temporary file. For stdin/env, ensure sensitive data doesn't leak into logs:

```python
# Don't log sensitive parameters
logger.info(f"Processing request for user: {params['username']}")
# Don't do this:
# logger.debug(f"Full params: {params}")  # May contain secrets!
```

---

## Design Philosophy

### Parameters vs Environment Variables

**Action Parameters** (`stdin` or `file`):
- Data the action operates on
- Always secure (never in environment)
- Examples: API payloads, credentials, business data
- Stored in `execution.config` → `parameters`
- Passed via stdin or temporary file

**Environment Variables** (`execution.env_vars`):
- Execution context and configuration
- Set as environment variables by worker
- Examples: `ATTUNE_EXECUTION_ID`, custom config, feature flags
- Stored in `execution.env_vars` JSONB
- Typically non-sensitive

### Default Behavior (Secure by Default)

**As of 2025-02-05**: Parameters default to:
- `parameter_delivery: stdin`
- `parameter_format: json`

All action parameters are secure by design. There is no option to pass parameters as environment variables.

### Migration from Environment Variables

If you were previously passing data as environment variables, you now have two options:

**Option 1: Move to Parameters** (for action data):
```python
# Read from stdin
import sys, json
content = sys.stdin.read()
params = json.loads(content.split('---ATTUNE_PARAMS_END---')[0])
value = params.get('key')
```

**Option 2: Use execution.env_vars** (for execution context):
Store non-sensitive configuration in `execution.env_vars` when creating the execution:
```json
{
  "action_ref": "mypack.myaction",
  "parameters": {"data": "value"},
  "env_vars": {"CUSTOM_CONFIG": "value"}
}
```

Then read from environment in action:
```python
import os
config = os.environ.get('CUSTOM_CONFIG')
```

---

## Examples

### Complete Python Action with Stdin/JSON

**Action YAML** (`mypack/actions/secure_action.yaml`):
```yaml
name: secure_action
ref: mypack.secure_action
description: "Secure action with stdin parameter delivery"
runner_type: python
entry_point: secure_action.py
# Uses default stdin + json (no need to specify)

parameters:
  type: object
  properties:
    api_token:
      type: string
      secret: true
    endpoint:
      type: string
    data:
      type: object
  required:
    - api_token
    - endpoint
```

**Action Script** (`mypack/actions/secure_action.py`):
```python
#!/usr/bin/env python3
import sys
import json
import requests

def read_stdin_params():
    """Read parameters and secrets from stdin."""
    content = sys.stdin.read()
    parts = content.split('---ATTUNE_PARAMS_END---')
    
    params = json.loads(parts[0].strip()) if parts[0].strip() else {}
    secrets = {}
    if len(parts) > 1 and parts[1].strip():
        secrets = json.loads(parts[1].strip())
    
    return {**params, **secrets}

def main():
    params = read_stdin_params()
    
    api_token = params.get('api_token')
    endpoint = params.get('endpoint')
    data = params.get('data', {})
    
    if not api_token or not endpoint:
        print(json.dumps({"error": "Missing required parameters"}))
        sys.exit(1)
    
    headers = {"Authorization": f"Bearer {api_token}"}
    response = requests.post(endpoint, json=data, headers=headers)
    
    result = {
        "status_code": response.status_code,
        "response": response.json() if response.ok else None,
        "success": response.ok
    }
    
    print(json.dumps(result))
    sys.exit(0 if response.ok else 1)

if __name__ == "__main__":
    main()
```

### Complete Shell Action with File/YAML

**Action YAML** (`mypack/actions/process_config.yaml`):
```yaml
name: process_config
ref: mypack.process_config
description: "Process configuration with file-based parameter delivery"
runner_type: shell
entry_point: process_config.sh
# Explicitly use file delivery for large configs
parameter_delivery: file
parameter_format: yaml

parameters:
  type: object
  properties:
    config:
      type: object
      description: "Configuration object"
    environment:
      type: string
      enum: [dev, staging, prod]
  required:
    - config
```

**Action Script** (`mypack/actions/process_config.sh`):
```bash
#!/bin/bash
set -e

# Check if parameter file exists
if [ -z "$ATTUNE_PARAMETER_FILE" ]; then
    echo "ERROR: No parameter file provided" >&2
    exit 1
fi

# Read configuration from YAML file (requires yq)
ENVIRONMENT=$(yq eval '.environment // "dev"' "$ATTUNE_PARAMETER_FILE")
CONFIG=$(yq eval '.config' "$ATTUNE_PARAMETER_FILE")

echo "Processing configuration for environment: $ENVIRONMENT"
echo "Config: $CONFIG"

# Process configuration...
# Your logic here

echo "Configuration processed successfully"
exit 0
```

---

## Environment Variables Reference

Actions automatically receive these environment variables:

**System Variables** (always set):
- `ATTUNE_EXECUTION_ID` - Current execution ID
- `ATTUNE_ACTION_REF` - Action reference (e.g., "mypack.myaction")
- `ATTUNE_PARAMETER_DELIVERY` - Delivery method (stdin/file)
- `ATTUNE_PARAMETER_FORMAT` - Format used (json/dotenv/yaml)
- `ATTUNE_PARAMETER_FILE` - File path (only for file delivery)

**Custom Variables** (from `execution.env_vars`):
Any key-value pairs in `execution.env_vars` are set as environment variables.

Example:
```json
{
  "env_vars": {
    "LOG_LEVEL": "debug",
    "RETRY_COUNT": "3"
  }
}
```

Action receives:
```bash
LOG_LEVEL=debug
RETRY_COUNT=3
```

---

## Related Documentation

- [Pack Structure](../packs/pack-structure.md)
- [Action Development Guide](./action-development-guide.md) (future)
- [Secrets Management](../authentication/secrets-management.md)
- [Security Best Practices](../authentication/security-review-2024-01-02.md)
- [Execution API](../api/api-executions.md)

---

## Support

For questions or issues related to parameter delivery:
1. Check the action logs for parameter delivery metadata
2. Verify the `ATTUNE_PARAMETER_DELIVERY` and `ATTUNE_PARAMETER_FORMAT` environment variables
3. Test with a simple action first before implementing complex parameter handling
4. Review the example actions in the `core` pack for reference implementations