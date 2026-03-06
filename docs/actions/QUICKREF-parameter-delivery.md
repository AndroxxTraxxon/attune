# Parameter Delivery Quick Reference

**Quick guide for choosing and implementing secure parameter passing in actions**

---

## TL;DR - Security First

**DEFAULT**: `stdin` + `json` (secure by default as of 2025-02-05)

**KEY DESIGN**: Parameters and environment variables are separate!
- **Parameters** = Action data (always secure: stdin or file)
- **Environment Variables** = Execution context (separate: `execution.env_vars`)

```yaml
# ✅ DEFAULT (no need to specify) - secure for all actions
# parameter_delivery: stdin
# parameter_format: json

# For large payloads only:
parameter_delivery: file
parameter_format: yaml
```

---

## Quick Decision Matrix

| Your Action Has... | Use This |
|--------------------|----------|
| 🔑 API keys, passwords, tokens | Default (`stdin` + `json`) |
| 📦 Large config files (>1MB) | `file` + `yaml` |
| 🐚 Shell scripts | Default (`stdin` + `json` or `dotenv`) |
| 🐍 Python/Node.js actions | Default (`stdin` + `json`) |
| 📝 Most actions | Default (`stdin` + `json`) |

---

## Two Delivery Methods

### 1. Standard Input (`stdin`)

**Security**: ✅ HIGH - Not in process list  
**When**: Credentials, API keys, structured data (DEFAULT)

```yaml
# This is the DEFAULT (no need to specify)
# parameter_delivery: stdin
# parameter_format: json
```

```python
# Read from stdin (secrets are merged into parameters)
import sys, json
content = sys.stdin.read().strip()
params = json.loads(content) if content else {}
api_key = params['api_key']  # Secure!
```

---

### 2. Temporary File (`file`)

**Security**: ✅ HIGH - Restrictive permissions (0400)  
**When**: Large payloads, complex configs

```yaml
# Explicitly use file for large payloads
parameter_delivery: file
parameter_format: yaml
```

```python
# Read from file
import os, yaml
param_file = os.environ['ATTUNE_PARAMETER_FILE']
with open(param_file) as f:
    params = yaml.safe_load(f)
```

---

## Format Options

| Format | Best For | Example |
|--------|----------|---------|
| `json` (default) | Python/Node.js, structured data | `{"key": "value"}` |
| `dotenv` | Simple key-value when needed | `KEY='value'` |
| `yaml` | Human-readable configs | `key: value` |

---

## Copy-Paste Templates

### Python Action (Secure with Stdin/JSON)

```yaml
# action.yaml
name: my_action
ref: mypack.my_action
runner_type: python
entry_point: my_action.py
parameter_delivery: stdin
parameter_format: json

parameters:
  type: object
  properties:
    api_key:
      type: string
      secret: true
```

```python
#!/usr/bin/env python3
# my_action.py
import sys
import json

def read_params():
    """Read parameters from stdin. Secrets are already merged in."""
    content = sys.stdin.read().strip()
    return json.loads(content) if content else {}

params = read_params()
api_key = params['api_key']
# Use api_key securely...
```

---

### Shell Action (Secure with Stdin/JSON)

```yaml
# action.yaml
name: my_script
ref: mypack.my_script
runner_type: shell
entry_point: my_script.sh
parameter_delivery: stdin
parameter_format: json
```

```bash
#!/bin/bash
# my_script.sh
set -e

# Read params from stdin (requires jq)
read -r PARAMS_JSON
API_KEY=$(echo "$PARAMS_JSON" | jq -r '.api_key')

# Use API_KEY securely...
```

---

### Shell Action (Using Stdin with Dotenv)

```yaml
name: simple_script
ref: mypack.simple_script
runner_type: shell
entry_point: simple.sh
# Can use dotenv format with stdin for simple shell scripts
parameter_delivery: stdin
parameter_format: dotenv
```

```bash
#!/bin/bash
# simple.sh
# Read dotenv from stdin
eval "$(cat)"
echo "$MESSAGE"
```

---

## Environment Variables

**System Variables** (always set):
- `ATTUNE_EXECUTION_ID` - Execution ID
- `ATTUNE_ACTION_REF` - Action reference
- `ATTUNE_PARAMETER_DELIVERY` - Method used (stdin/file, default: stdin)
- `ATTUNE_PARAMETER_FORMAT` - Format used (json/dotenv/yaml, default: json)
- `ATTUNE_PARAMETER_FILE` - Path to temp file (file delivery only)

**Custom Variables** (from `execution.env_vars`):
- Set any custom environment variables via `execution.env_vars` when creating execution
- These are separate from parameters
- Use for execution context, configuration, non-sensitive metadata

---

## Common Patterns

### Detect Delivery Method

```python
import os

delivery = os.environ.get('ATTUNE_PARAMETER_DELIVERY', 'env')
if delivery == 'stdin':
    params = read_from_stdin()
elif delivery == 'file':
    params = read_from_file()
else:
    params = read_from_env()
```

---

### Mark Sensitive Parameters

```yaml
parameters:
  type: object
  properties:
    api_key:
      type: string
      secret: true        # Mark as sensitive
    password:
      type: string
      secret: true
    public_url:
      type: string          # Not marked - not sensitive
```

---

### Validate Required Parameters

```python
params = read_params()
if not params.get('api_key'):
    print(json.dumps({"error": "api_key required"}))
    sys.exit(1)
```

---

## Security Checklist

- [ ] Identified all sensitive parameters
- [ ] Marked sensitive params with `secret: true`
- [ ] Set `parameter_delivery: stdin` or `file` (not `env`)
- [ ] Set appropriate `parameter_format`
- [ ] Updated action script to read from stdin/file
- [ ] Tested that secrets don't appear in `ps aux`
- [ ] Don't log sensitive parameters
- [ ] Handle missing parameters gracefully

---

## Testing

```bash
# Run action and check process list
./attune execution start mypack.my_action --params '{"api_key":"secret123"}' &

# In another terminal
ps aux | grep attune-worker
# Should NOT see "secret123" in output!
```

---

## Key Design Change (2025-02-05)

**Parameters and Environment Variables Are Separate**

**Parameters** (always secure):
- Passed via `stdin` (default) or `file` (large payloads)
- Never passed as environment variables
- Read from stdin or parameter file

```python
# Read parameters from stdin (secrets are merged in)
import sys, json
content = sys.stdin.read().strip()
params = json.loads(content) if content else {}
api_key = params['api_key']  # Secure!
```

**Environment Variables** (execution context):
- Set via `execution.env_vars` when creating execution
- Separate from parameters
- Read from environment

```python
# Read environment variables (context, not parameters)
import os
log_level = os.environ.get('LOG_LEVEL', 'info')
```

---

## Don't Do This

```python
# ❌ Don't log sensitive parameters
logger.debug(f"Params: {params}")  # May contain secrets!

# ❌ Don't confuse parameters with env vars
# Parameters come from stdin/file, not environment

# ❌ Don't forget to mark secrets
# api_key:
#   type: string
#   # Missing: secret: true

# ❌ Don't put sensitive data in execution.env_vars
# Use parameters for sensitive data, env_vars for context
```

---

## Do This Instead

```python
# ✅ Log only non-sensitive data
logger.info(f"Calling endpoint: {params['endpoint']}")

# ✅ Use stdin for parameters (the default!)
# parameter_delivery: stdin  # No need to specify

# ✅ Mark all secrets
# api_key:
#   type: string
#   secret: true

# ✅ Use env_vars for execution context
# Set when creating execution:
# {"env_vars": {"LOG_LEVEL": "debug"}}
```

---

## Help & Support

**Full Documentation**: `docs/actions/parameter-delivery.md`

**Examples**: See `packs/core/actions/http_request.yaml`

**Questions**: 
- Parameters: Check `ATTUNE_PARAMETER_DELIVERY` env var
- Env vars: Set via `execution.env_vars` when creating execution

---

## Summary

1. **Default is `stdin` + `json` - secure by default! 🎉**
2. **Parameters and environment variables are separate concepts**
3. **Parameters are always secure (stdin or file, never env)**
4. **Mark sensitive parameters with `secret: true`**
5. **Use `execution.env_vars` for execution context, not parameters**
6. **Test that secrets aren't in process list**

**Remember**: Parameters are secure by design - they're never in environment variables! 🔒