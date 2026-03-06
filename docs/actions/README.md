# Action Parameter Delivery

This directory contains documentation for Attune's secure parameter passing system for actions.

## Quick Links

- **[Parameter Delivery Guide](./parameter-delivery.md)** - Complete guide to parameter delivery methods, formats, and best practices (568 lines)
- **[Quick Reference](./QUICKREF-parameter-delivery.md)** - Quick decision matrix and copy-paste templates (365 lines)

## Overview

Attune provides three methods for delivering parameters to actions, with **stdin + JSON as the secure default** (as of 2025-02-05):

### Delivery Methods

| Method | Security | Use Case |
|--------|----------|----------|
| **stdin** (default) | ✅ High | Credentials, structured data, most actions |
| **env** (explicit) | ⚠️ Low | Simple non-sensitive shell scripts only |
| **file** | ✅ High | Large payloads, complex configurations |

### Serialization Formats

| Format | Best For | Example |
|--------|----------|---------|
| **json** (default) | Python/Node.js, structured data | `{"key": "value"}` |
| **dotenv** | Shell scripts, simple key-value | `KEY='value'` |
| **yaml** | Human-readable configs | `key: value` |

## Security Warning

⚠️ **Environment variables are visible in process listings** (`ps aux`, `/proc/<pid>/environ`)

**Never use `env` delivery for sensitive parameters** like passwords, API keys, or tokens.

## Quick Start

### Secure Action (Default - No Configuration Needed)

```yaml
# action.yaml
name: my_action
ref: mypack.my_action
runner_type: python
entry_point: my_action.py
# Uses default stdin + json (no need to specify)

parameters:
  type: object
  properties:
    api_key:
      type: string
      secret: true
```

```python
# my_action.py
import sys, json

# Read from stdin (the default) — secrets are merged into parameters
content = sys.stdin.read().strip()
params = json.loads(content) if content else {}
api_key = params['api_key']  # Secure - not in process list!
```

### Simple Shell Script (Non-Sensitive - Explicit env)

```yaml
# action.yaml
name: simple_script
ref: mypack.simple_script
runner_type: shell
entry_point: simple.sh
# Explicitly use env for non-sensitive data
parameter_delivery: env
parameter_format: dotenv
```

```bash
# simple.sh
MESSAGE="${ATTUNE_ACTION_MESSAGE:-Hello}"
echo "$MESSAGE"
```

## Key Features

- ✅ **Secure by default** - stdin prevents process listing exposure
- ✅ **Type preservation** - JSON format maintains data types
- ✅ **Automatic cleanup** - Temporary files auto-deleted
- ✅ **Flexible formats** - Choose JSON, YAML, or dotenv
- ✅ **Explicit opt-in** - Only use env when you really need it

## Environment Variables

All actions receive these metadata variables:

- `ATTUNE_PARAMETER_DELIVERY` - Method used (stdin/env/file)
- `ATTUNE_PARAMETER_FORMAT` - Format used (json/dotenv/yaml)
- `ATTUNE_PARAMETER_FILE` - File path (file delivery only)
- `ATTUNE_ACTION_<KEY>` - Individual parameters (env delivery only)

## Breaking Change Notice

**As of 2025-02-05**, the default parameter delivery changed from `env` to `stdin` for security.

Actions that need environment variable delivery must **explicitly opt-in** by setting:

```yaml
parameter_delivery: env
parameter_format: dotenv
```

This is allowed because Attune is in pre-production with no users or deployments (per AGENTS.md policy).

## Best Practices

1. ✅ **Use default stdin + json** for most actions
2. ✅ **Mark sensitive parameters** with `secret: true`
3. ✅ **Only use env explicitly** for simple, non-sensitive shell scripts
4. ✅ **Test credentials don't appear** in `ps aux` output
5. ✅ **Never log sensitive parameters**

## Example Actions

See the core pack for examples:

- `packs/core/actions/http_request.yaml` - Uses stdin + json (handles API tokens)
- `packs/core/actions/echo.yaml` - Uses env + dotenv (no secrets)
- `packs/core/actions/sleep.yaml` - Uses env + dotenv (no secrets)

## Documentation Structure

```
docs/actions/
├── README.md                          # This file - Overview and quick links
├── parameter-delivery.md              # Complete guide (568 lines)
│   ├── Security concerns
│   ├── Detailed method descriptions
│   ├── Format specifications
│   ├── Configuration syntax
│   ├── Best practices
│   ├── Migration guide
│   └── Complete examples
└── QUICKREF-parameter-delivery.md     # Quick reference (365 lines)
    ├── TL;DR
    ├── Decision matrix
    ├── Copy-paste templates
    ├── Common patterns
    └── Testing tips
```

## Getting Help

1. **Quick decisions**: See [QUICKREF-parameter-delivery.md](./QUICKREF-parameter-delivery.md)
2. **Detailed guide**: See [parameter-delivery.md](./parameter-delivery.md)
3. **Check delivery method**: Look at `ATTUNE_PARAMETER_DELIVERY` env var
4. **Test security**: Run `ps aux | grep attune-worker` to verify secrets aren't visible

## Summary

**Default**: `stdin` + `json` - Secure, structured, type-preserving parameter passing.

**Remember**: stdin is the default. Environment variables require explicit opt-in! 🔒