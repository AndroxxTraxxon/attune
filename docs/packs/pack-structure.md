# Pack Structure Documentation

**Last Updated**: 2024-01-20  
**Status**: Reference Documentation

---

## Overview

Attune packs are bundles of automation components (actions, sensors, triggers, rules, workflows) organized in a standardized directory structure. This document defines the canonical pack structure and file formats.

---

## Pack Directory Structure

```
packs/<pack_name>/
├── pack.yaml                    # Pack manifest (required)
├── README.md                    # Pack documentation (recommended)
├── CHANGELOG.md                 # Version history (recommended)
├── LICENSE                      # License file (recommended)
├── requirements.txt             # Python dependencies (optional)
├── package.json                 # Node.js dependencies (optional)
├── actions/                     # Action definitions
│   ├── <action_name>.yaml      # Action metadata
│   ├── <action_name>.sh        # Shell action implementation
│   ├── <action_name>.py        # Python action implementation
│   └── <action_name>.js        # Node.js action implementation
├── sensors/                     # Sensor definitions
│   ├── <sensor_name>.yaml      # Sensor metadata
│   ├── <sensor_name>.py        # Python sensor implementation
│   └── <sensor_name>.js        # Node.js sensor implementation
├── triggers/                    # Trigger type definitions
│   └── <trigger_name>.yaml     # Trigger metadata
├── rules/                       # Rule definitions (optional)
│   └── <rule_name>.yaml        # Rule metadata
├── workflows/                   # Workflow definitions (optional)
│   └── <workflow_name>.yaml    # Workflow metadata
├── policies/                    # Policy definitions (optional)
│   └── <policy_name>.yaml      # Policy metadata
├── config.schema.yaml           # Pack configuration schema (optional)
├── icon.png                     # Pack icon (optional)
└── tests/                       # Tests (optional)
    ├── test_actions.py
    └── test_sensors.py
```

---

## File Formats

### Pack Manifest (`pack.yaml`)

The pack manifest is the main metadata file for a pack. It defines the pack's identity, version, dependencies, and configuration.

**Required Fields:**
- `ref` (string): Unique pack reference/identifier (lowercase, alphanumeric, hyphens, underscores)
- `label` (string): Human-readable pack name
- `description` (string): Brief description of the pack
- `version` (string): Semantic version (e.g., "1.0.0")

**Optional Fields:**
- `author` (string): Pack author name
- `email` (string): Author email
- `system` (boolean): Whether this is a system pack (default: false)
- `enabled` (boolean): Whether pack is enabled by default (default: true)
- `conf_schema` (object): JSON Schema for pack configuration
- `config` (object): Default pack configuration
- `meta` (object): Additional metadata
- `tags` (array): Tags for categorization
- `runtime_deps` (array): Runtime dependencies (e.g., "python3", "nodejs", "shell")

**Example:**

```yaml
ref: core
label: "Core Pack"
description: "Built-in core functionality including timer triggers and basic actions"
version: "1.0.0"
author: "Attune Team"
email: "core@attune.io"
system: true
enabled: true

conf_schema:
  type: object
  properties:
    max_action_timeout:
      type: integer
      description: "Maximum timeout for action execution in seconds"
      default: 300
      minimum: 1
      maximum: 3600
  required: []

config:
  max_action_timeout: 300

meta:
  category: "system"
  keywords:
    - "core"
    - "utilities"
  python_dependencies:
    - "requests>=2.28.0"
  documentation_url: "https://docs.attune.io/packs/core"
  repository_url: "https://github.com/attune-io/attune"

tags:
  - core
  - system
  - utilities

runtime_deps:
  - shell
  - python3
```

---

### Action Metadata (`actions/<action_name>.yaml`)

Action metadata files define the parameters, output schema, and execution details for actions.

**Required Fields:**
- `name` (string): Action name (matches filename)
- `ref` (string): Full action reference (e.g., "core.echo")
- `description` (string): Action description
- `runner_type` (string): Execution runtime (shell, python, nodejs, docker)
- `entry_point` (string): Script filename to execute

**Optional Fields:**
- `enabled` (boolean): Whether action is enabled (default: true)
- `parameters` (object): Parameter definitions (JSON Schema style)
- `output_schema` (object): Output schema definition
- `tags` (array): Tags for categorization
- `timeout` (integer): Default timeout in seconds
- `examples` (array): Usage examples

**Example:**

```yaml
name: echo
ref: core.echo
description: "Echo a message to stdout"
enabled: true
runner_type: shell
entry_point: echo.sh

parameters:
  message:
    type: string
    description: "Message to echo"
    required: true
    default: "Hello, World!"
  uppercase:
    type: boolean
    description: "Convert message to uppercase"
    required: false
    default: false

output_schema:
  type: object
  properties:
    stdout:
      type: string
      description: "Standard output from the command"
    exit_code:
      type: integer
      description: "Exit code (0 = success)"

tags:
  - utility
  - testing
```

---

### Action Implementation

Action implementations receive parameters as environment variables prefixed with `ATTUNE_ACTION_`.

**Shell Example (`actions/echo.sh`):**

```bash
#!/bin/bash
set -e

# Parse parameters from environment variables
MESSAGE="${ATTUNE_ACTION_MESSAGE:-Hello, World!}"
UPPERCASE="${ATTUNE_ACTION_UPPERCASE:-false}"

# Convert to uppercase if requested
if [ "$UPPERCASE" = "true" ]; then
    MESSAGE=$(echo "$MESSAGE" | tr '[:lower:]' '[:upper:]')
fi

# Echo the message
echo "$MESSAGE"

# Exit successfully
exit 0
```

**Python Example (`actions/http_request.py`):**

```python
#!/usr/bin/env python3
import json
import os
import sys

def get_env_param(name: str, default=None):
    """Get action parameter from environment variable."""
    env_key = f"ATTUNE_ACTION_{name.upper()}"
    return os.environ.get(env_key, default)

def main():
    url = get_env_param("url", required=True)
    method = get_env_param("method", "GET")
    
    # Perform action logic
    result = {
        "url": url,
        "method": method,
        "success": True
    }
    
    # Output result as JSON
    print(json.dumps(result, indent=2))
    sys.exit(0)

if __name__ == "__main__":
    main()
```

---

### Sensor Metadata (`sensors/<sensor_name>.yaml`)

Sensor metadata files define sensors that monitor for events and fire triggers.

**Required Fields:**
- `name` (string): Sensor name
- `ref` (string): Full sensor reference (e.g., "core.interval_timer_sensor")
- `description` (string): Sensor description
- `runner_type` (string): Execution runtime (python, nodejs)
- `entry_point` (string): Script filename to execute
- `trigger_types` (array): List of trigger types this sensor monitors

**Optional Fields:**
- `enabled` (boolean): Whether sensor is enabled (default: true)
- `parameters` (object): Sensor configuration parameters
- `poll_interval` (integer): Poll interval in seconds
- `tags` (array): Tags for categorization
- `meta` (object): Additional metadata

**Example:**

```yaml
name: interval_timer_sensor
ref: core.interval_timer_sensor
description: "Monitors time and fires interval timer triggers"
enabled: true
runner_type: python
entry_point: interval_timer_sensor.py

trigger_types:
  - core.intervaltimer

parameters:
  check_interval_seconds:
    type: integer
    description: "How often to check if triggers should fire"
    default: 1
    minimum: 1
    maximum: 60

poll_interval: 1

tags:
  - timer
  - system
  - builtin

meta:
  builtin: true
  system: true
```

---

### Sensor Implementation

Sensors run continuously and emit events to stdout as JSON, one per line.

**Python Example (`sensors/interval_timer_sensor.py`):**

```python
#!/usr/bin/env python3
import json
import time
from datetime import datetime

def check_triggers():
    """Check configured triggers and return events to fire."""
    # Load trigger instances from environment
    # Check if any should fire
    # Return list of events
    return []

def main():
    while True:
        events = check_triggers()
        
        # Output events as JSON (one per line)
        for event in events:
            print(json.dumps(event))
            sys.stdout.flush()
        
        # Sleep until next check
        time.sleep(1)

if __name__ == "__main__":
    main()
```

---

### Trigger Metadata (`triggers/<trigger_name>.yaml`)

Trigger metadata files define event types that sensors can fire.

**Required Fields:**
- `name` (string): Trigger name
- `ref` (string): Full trigger reference (e.g., "core.intervaltimer")
- `description` (string): Trigger description
- `type` (string): Trigger type (interval, cron, one_shot, webhook, custom)

**Optional Fields:**
- `enabled` (boolean): Whether trigger is enabled (default: true)
- `parameters_schema` (object): Schema for trigger instance configuration
- `payload_schema` (object): Schema for event payload
- `tags` (array): Tags for categorization
- `examples` (array): Configuration examples

**Example:**

```yaml
name: intervaltimer
ref: core.intervaltimer
description: "Fires at regular intervals"
enabled: true
type: interval

parameters_schema:
  type: object
  properties:
    unit:
      type: string
      enum: [seconds, minutes, hours]
      description: "Time unit for the interval"
    interval:
      type: integer
      minimum: 1
      description: "Number of time units between triggers"
  required: [unit, interval]

payload_schema:
  type: object
  properties:
    type:
      type: string
      const: interval
    interval_seconds:
      type: integer
    fired_at:
      type: string
      format: date-time
  required: [type, interval_seconds, fired_at]

tags:
  - timer
  - interval

examples:
  - description: "Fire every 10 seconds"
    parameters:
      unit: "seconds"
      interval: 10
```

---

## Pack Loading Process

When a pack is loaded, Attune performs the following steps:

1. **Parse Pack Manifest**: Read and validate `pack.yaml`
2. **Register Pack**: Insert pack metadata into database
3. **Load Actions**: Parse all `actions/*.yaml` files and register actions
4. **Load Sensors**: Parse all `sensors/*.yaml` files and register sensors
5. **Load Triggers**: Parse all `triggers/*.yaml` files and register triggers
6. **Load Rules** (optional): Parse all `rules/*.yaml` files
7. **Load Workflows** (optional): Parse all `workflows/*.yaml` files
8. **Validate Dependencies**: Check that all dependencies are available
9. **Apply Configuration**: Apply default configuration from pack manifest

---

## Pack Types

### System Packs

System packs are built-in packs that ship with Attune.

- `system: true` in pack manifest
- Installed automatically
- Cannot be uninstalled
- Examples: `core`, `system`, `utils`

### Community Packs

Community packs are third-party packs installed from repositories.

- `system: false` in pack manifest
- Installed via CLI or API
- Can be updated and uninstalled
- Examples: `slack`, `aws`, `github`, `datadog`

### Ad-Hoc Packs

Ad-hoc packs are user-created packs without code-based components.

- `system: false` in pack manifest
- Created via Web UI
- May only contain triggers (no actions/sensors)
- Used for custom webhook integrations

---

## Best Practices

### Naming Conventions

- **Pack refs**: lowercase, alphanumeric, hyphens/underscores (e.g., `my-pack`, `aws_ec2`)
- **Component refs**: `<pack_ref>.<component_name>` (e.g., `core.echo`, `slack.send_message`)
- **File names**: Match component names (e.g., `echo.yaml`, `echo.sh`)

### Versioning

- Use semantic versioning (MAJOR.MINOR.PATCH)
- Update `CHANGELOG.md` with each release
- Increment version in `pack.yaml` when releasing

### Documentation

- Include comprehensive `README.md` with usage examples
- Document all parameters and output schemas
- Provide example configurations

### Testing

- Include unit tests in `tests/` directory
- Test actions/sensors independently
- Validate parameter schemas

### Dependencies

- Specify all runtime dependencies in pack manifest
- Pin dependency versions in `requirements.txt` or `package.json`
- Test with minimum required versions

### Security

- Use `secret: true` for sensitive parameters (passwords, tokens, API keys)
- Validate all user inputs
- Sanitize command-line arguments to prevent injection
- Use HTTPS for API calls with SSL verification enabled

---

## Example Packs

### Minimal Pack

```
my-pack/
├── pack.yaml
├── README.md
└── actions/
    ├── hello.yaml
    └── hello.sh
```

### Full-Featured Pack

```
slack-pack/
├── pack.yaml
├── README.md
├── CHANGELOG.md
├── LICENSE
├── requirements.txt
├── icon.png
├── actions/
│   ├── send_message.yaml
│   ├── send_message.py
│   ├── upload_file.yaml
│   └── upload_file.py
├── sensors/
│   ├── message_sensor.yaml
│   └── message_sensor.py
├── triggers/
│   ├── message_received.yaml
│   └── reaction_added.yaml
├── config.schema.yaml
└── tests/
    ├── test_actions.py
    └── test_sensors.py
```

---

## Related Documentation

- [Pack Management Architecture](./pack-management-architecture.md)
- [Pack Management API](./api-packs.md)
- [Trigger and Sensor Architecture](./trigger-sensor-architecture.md)
- [Action Development Guide](./action-development-guide.md) (future)
- [Sensor Development Guide](./sensor-development-guide.md) (future)