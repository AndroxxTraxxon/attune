# Attune CLI

The Attune CLI is a comprehensive command-line tool for interacting with the Attune automation platform. It provides an intuitive interface for managing all aspects of the platform including packs, actions, rules, executions, and more.

## Overview

The CLI is designed to be:
- **Intuitive**: Natural command structure with helpful prompts
- **Flexible**: Multiple output formats for human and machine consumption
- **Powerful**: Full access to all API functionality
- **Scriptable**: JSON/YAML output for automation

## Installation

### From Source

```bash
cd attune
cargo install --path crates/cli
```

This will install the `attune` binary to your cargo bin directory (usually `~/.cargo/bin`).

### Development

```bash
cargo build -p attune-cli
./target/debug/attune --help
```

## Configuration

The CLI stores configuration in `~/.config/attune/config.yaml` (respects `$XDG_CONFIG_HOME`).

### Configuration Structure

```yaml
api_url: http://localhost:8080
auth_token: <jwt-access-token>
refresh_token: <jwt-refresh-token>
output_format: table
```

### Environment Variables

- `ATTUNE_API_URL`: Override the API endpoint
- `XDG_CONFIG_HOME`: Change config directory location

### Global Options

All commands support:
- `--api-url <URL>`: Override API endpoint
- `--output <format>`: Set output format (table, json, yaml)
- `-j, --json`: Output as JSON (shorthand for `--output json`)
- `-y, --yaml`: Output as YAML (shorthand for `--output yaml`)
- `-v, --verbose`: Enable debug logging

## Command Reference

### Authentication

#### Login
```bash
attune auth login --username admin
# Prompts for password securely
```

#### Logout
```bash
attune auth logout
```

#### Check Current User
```bash
attune auth whoami
```

### Pack Management

#### List Packs
```bash
attune pack list
attune pack list --name core
attune pack list --output json  # Long form
attune pack list -j             # Shorthand for JSON
attune pack list -y             # Shorthand for YAML
```

#### Show Pack Details
```bash
attune pack show core
attune pack show 1
```

#### Install Pack
```bash
attune pack install https://github.com/example/pack-example
attune pack install https://github.com/example/pack-example --ref v1.0.0
attune pack install <url> --force
```

#### Register Local Pack
```bash
attune pack register /path/to/pack
```

#### Uninstall Pack
```bash
attune pack uninstall core
attune pack uninstall core --yes
```

### Action Management

#### List Actions
```bash
attune action list
attune action list --pack core
attune action list --name echo
```

#### Show Action Details
```bash
attune action show core.echo
attune action show 1
```

#### Execute Action
```bash
# With key=value parameters
attune action execute core.echo --param message="Hello" --param count=3

# With JSON parameters
attune action execute core.echo --params-json '{"message": "Hello", "count": 5}'

# Wait for completion
attune action execute core.long_task --wait

# Wait with timeout
attune action execute core.long_task --wait --timeout 600
```

### Rule Management

#### List Rules
```bash
attune rule list
attune rule list --pack core
attune rule list --enabled true
```

#### Show Rule Details
```bash
attune rule show core.on_webhook
attune rule show 1
```

#### Enable/Disable Rules
```bash
attune rule enable core.on_webhook
attune rule disable core.on_webhook
```

#### Create Rule
```bash
attune rule create \
  --name my_rule \
  --pack core \
  --trigger core.webhook \
  --action core.notify \
  --description "Notify on webhook" \
  --enabled

# With criteria
attune rule create \
  --name filtered_rule \
  --pack core \
  --trigger core.webhook \
  --action core.notify \
  --criteria '{"trigger.payload.severity": "critical"}'
```

#### Delete Rule
```bash
attune rule delete core.my_rule
attune rule delete core.my_rule --yes
```

### Execution Monitoring

#### List Executions
```bash
attune execution list
attune execution list --pack core
attune execution list --action core.echo
attune execution list --status succeeded
attune execution list --result "error"
attune execution list --pack monitoring --status failed --result "timeout"
attune execution list --limit 100
```

#### Show Execution Details
```bash
attune execution show 123
```

#### View Logs
```bash
attune execution logs 123
attune execution logs 123 --follow
```

#### Cancel Execution
```bash
attune execution cancel 123
attune execution cancel 123 --yes
```

#### Get Raw Execution Result
```bash
# Get result as JSON (default)
attune execution result 123

# Get result as YAML
attune execution result 123 --format yaml

# Pipe to jq for processing
attune execution result 123 | jq '.data.field'

# Extract specific field
attune execution result 123 | jq -r '.status'
```

### Trigger Management

#### List Triggers
```bash
attune trigger list
attune trigger list --pack core
```

#### Show Trigger Details
```bash
attune trigger show core.webhook
```

### Sensor Management

#### List Sensors
```bash
attune sensor list
attune sensor list --pack core
```

#### Show Sensor Details
```bash
attune sensor show core.file_watcher
```

### Configuration Management

#### List Configuration
```bash
attune config list
```

#### Get Value
```bash
attune config get api_url
```

#### Set Value
```bash
attune config set api_url https://attune.example.com
attune config set output_format json
```

#### Show Config Path
```bash
attune config path
```

## Output Formats

### Table (Default)

Human-readable format with colors and formatting:
```bash
attune pack list
```

Output:
```
╭────┬──────┬─────────┬─────────┬─────────────────╮
│ ID │ Name │ Version │ Enabled │ Description     │
├────┼──────┼─────────┼─────────┼─────────────────┤
│ 1  │ core │ 1.0.0   │ ✓       │ Core actions... │
╰────┴──────┴─────────┴─────────┴─────────────────╯
```

### JSON

Machine-readable format for scripting:
```bash
attune pack list --output json  # Long form
attune pack list -j             # Shorthand
```

Output:
```json
[
  {
    "id": 1,
    "name": "core",
    "version": "1.0.0",
    "enabled": true,
    "description": "Core actions..."
  }
]
```

### YAML

Alternative structured format:
```bash
attune pack list --output yaml  # Long form
attune pack list -y             # Shorthand
```

Output:
```yaml
- id: 1
  name: core
  version: 1.0.0
  enabled: true
  description: Core actions...
```

## Scripting Examples

### Bash Script: Deploy Pack

```bash
#!/bin/bash
set -e

PACK_URL="https://github.com/example/monitoring-pack"
PACK_NAME="monitoring"

# Install pack
echo "Installing pack..."
PACK_ID=$(attune pack install "$PACK_URL" -j | jq -r '.id')

# Verify installation
if [ -z "$PACK_ID" ]; then
  echo "Pack installation failed"
  exit 1
fi

echo "Pack installed: ID=$PACK_ID"

# Enable all rules
attune rule list --pack "$PACK_NAME" -j | \
  jq -r '.[].id' | \
  xargs -I {} attune rule enable {}

echo "All rules enabled"
```

### Bash Script: Process Execution Results

```bash
#!/bin/bash
# Extract and process execution results

EXECUTION_ID=123

# Get raw result
RESULT=$(attune execution result $EXECUTION_ID)

# Extract specific fields
STATUS=$(echo "$RESULT" | jq -r '.status')
MESSAGE=$(echo "$RESULT" | jq -r '.message')

echo "Status: $STATUS"
echo "Message: $MESSAGE"

# Or pipe directly
attune execution result $EXECUTION_ID | jq -r '.errors[]'
```

### Python Script: Monitor Executions

```python
#!/usr/bin/env python3
import json
import subprocess
import time

def get_executions(status=None, pack=None, result_contains=None, limit=10):
    cmd = ["attune", "execution", "list", "-j", f"--limit={limit}"]
    if status:
        cmd.extend(["--status", status])
    if pack:
        cmd.extend(["--pack", pack])
    if result_contains:
        cmd.extend(["--result", result_contains])
    
    result = subprocess.run(cmd, capture_output=True, text=True)
    return json.loads(result.stdout)

def main():
    print("Monitoring failed executions with errors...")
    while True:
        # Find failed executions containing "error" in result
        failed = get_executions(status="failed", result_contains="error", limit=5)
        if failed:
            print(f"Found {len(failed)} failed executions:")
            for exec in failed:
                print(f"  - ID {exec['id']}: {exec['action_name']}")
        time.sleep(30)

if __name__ == "__main__":
    main()
```

## Troubleshooting

### Authentication Issues

**Problem**: "Not logged in" error

**Solution**:
```bash
# Check auth status
attune auth whoami

# Login again
attune auth login --username admin
```

### Connection Issues

**Problem**: Cannot connect to API

**Solution**:
```bash
# Check API URL
attune config get api_url

# Override temporarily
attune --api-url http://localhost:8080 auth whoami

# Update permanently
attune config set api_url http://localhost:8080
```

### Token Expiration

**Problem**: "Invalid token" error

**Solution**:
```bash
# Login again to refresh token
attune auth login --username admin
```

### Verbose Debugging

Enable verbose output to see HTTP requests:
```bash
attune --verbose pack list
```

## Best Practices

### Security

1. **Never hardcode passwords**: Use interactive prompts
2. **Protect config file**: Contains JWT tokens
3. **Use environment variables** for CI/CD: `ATTUNE_API_URL`

### Scripting

1. **Use JSON output** for parsing: `--output json`
2. **Check exit codes**: Non-zero on error
3. **Handle errors**: Use `set -e` in bash scripts
4. **Use jq** for JSON processing

### Performance

1. **Limit results**: Use `--limit` for large lists
2. **Filter server-side**: Use `--pack`, `--action`, `--status`, `--result` filters
3. **Avoid polling**: Use `--wait` for action execution
4. **Use specific filters**: Narrow results with combined filters for faster queries

## Architecture

### Components

```
attune-cli/
├── src/
│   ├── main.rs           # Entry point, CLI structure
│   ├── client.rs         # HTTP client wrapper
│   ├── config.rs         # Config file management
│   ├── output.rs         # Output formatting
│   └── commands/         # Command implementations
│       ├── auth.rs
│       ├── pack.rs
│       ├── action.rs
│       ├── rule.rs
│       ├── execution.rs
│       ├── trigger.rs
│       ├── sensor.rs
│       └── config.rs
```

### Key Dependencies

- **clap**: CLI argument parsing
- **reqwest**: HTTP client
- **serde_json/yaml**: Serialization
- **colored**: Terminal colors
- **comfy-table**: Table formatting
- **dialoguer**: Interactive prompts

### API Communication

The CLI communicates with the Attune API using:
- REST endpoints at `/api/v1/*`
- JWT bearer token authentication
- Standard JSON request/response format

## Future Enhancements

Potential future features:
- Shell completion (bash, zsh, fish)
- Interactive TUI mode
- Execution streaming (real-time logs)
- Bulk operations
- Pack development commands
- Workflow visualization
- Config profiles (dev, staging, prod)

## Related Documentation

- [Main README](../README.md)
- [API Documentation](api-overview.md)
- [Pack Development](packs.md)
- [Configuration Guide](configuration.md)