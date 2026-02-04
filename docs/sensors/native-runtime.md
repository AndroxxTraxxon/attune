# Native Runtime Support

## Overview

The native runtime allows Attune to execute compiled binaries directly without requiring any language interpreter or shell wrapper. This is ideal for:

- Rust applications (like the timer sensor)
- Go binaries
- C/C++ executables
- Any other compiled native executable

## Runtime Configuration

Native runtime entries are automatically seeded in the database:

- **Action Runtime**: `core.action.native`
- **Sensor Runtime**: `core.sensor.native`

These runtimes are available in the `runtime` table and can be referenced by actions and sensors.

## Using Native Runtime in Actions

To create an action that uses the native runtime:

### 1. Action YAML Definition

```yaml
name: my_native_action
ref: mypack.my_native_action
description: "Execute a compiled binary"
enabled: true

# Specify native as the runner type
runner_type: native

# Entry point is the binary name (relative to pack directory)
entry_point: my_binary

parameters:
  input_data:
    type: string
    description: "Input data for the action"
    required: true

result_schema:
  type: object
  properties:
    status:
      type: string
    data:
      type: object
```

### 2. Binary Location

Place your compiled binary in the pack's actions directory:

```
packs/
└── mypack/
    └── actions/
        └── my_binary  (executable)
```

### 3. Binary Requirements

Your native binary should:

- **Accept parameters** via environment variables with `ATTUNE_ACTION_` prefix
  - Example: `ATTUNE_ACTION_INPUT_DATA` for parameter `input_data`
- **Accept secrets** via stdin as JSON (optional)
- **Output results** to stdout as JSON (optional)
- **Exit with code 0** for success, non-zero for failure
- **Be executable** (chmod +x on Unix systems)

### Example Native Action (Rust)

```rust
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read parameters from environment variables
    let input_data = env::var("ATTUNE_ACTION_INPUT_DATA")
        .unwrap_or_else(|_| "default".to_string());

    // Optionally read secrets from stdin
    let mut secrets = HashMap::new();
    if !atty::is(atty::Stream::Stdin) {
        let mut stdin = String::new();
        io::stdin().read_to_string(&mut stdin)?;
        if !stdin.is_empty() {
            secrets = serde_json::from_str(&stdin)?;
        }
    }

    // Perform action logic
    let result = serde_json::json!({
        "status": "success",
        "data": {
            "input": input_data,
            "processed": true
        }
    });

    // Output result as JSON to stdout
    println!("{}", serde_json::to_string(&result)?);

    Ok(())
}
```

## Using Native Runtime in Sensors

The timer sensor (`attune-core-timer-sensor`) is the primary example of a native sensor.

### 1. Sensor YAML Definition

```yaml
name: interval_timer_sensor
ref: core.interval_timer_sensor
description: "Timer sensor built in Rust"
enabled: true

# Specify native as the runner type
runner_type: native

# Entry point is the binary name
entry_point: attune-core-timer-sensor

trigger_types:
  - core.intervaltimer
```

### 2. Binary Location

Place the sensor binary in the pack's sensors directory:

```
packs/
└── core/
    └── sensors/
        └── attune-core-timer-sensor  (executable)
```

### 3. Sensor Binary Requirements

Native sensor binaries typically:

- **Run as daemons** - continuously monitor for trigger events
- **Accept configuration** via environment variables or stdin JSON
- **Authenticate with API** using service account tokens
- **Listen to RabbitMQ** for rule lifecycle events
- **Emit events** to the Attune API when triggers fire
- **Handle graceful shutdown** on SIGTERM/SIGINT

See `attune-core-timer-sensor` source code for a complete example.

## Runtime Selection

The worker service automatically selects the native runtime when:

1. The action/sensor explicitly specifies `runtime_name: "native"` in the execution context, OR
2. The code_path points to a file without a common script extension (.py, .js, .sh, etc.)

The native runtime performs these checks before execution:

- Binary file exists at the specified path
- Binary has executable permissions (Unix systems)

## Execution Details

### Environment Variables

Parameters are passed as environment variables:

- Format: `ATTUNE_ACTION_{PARAMETER_NAME_UPPERCASE}`
- Example: `input_data` becomes `ATTUNE_ACTION_INPUT_DATA`
- Values are converted to strings (JSON for complex types)

### Secrets

Secrets are passed via stdin as JSON:

```json
{
  "api_key": "secret-value",
  "db_password": "another-secret"
}
```

### Output Handling

- **stdout**: Captured and optionally parsed as JSON result
- **stderr**: Captured and included in execution logs
- **Exit code**: 0 = success, non-zero = failure
- **Size limits**: Both stdout and stderr are bounded (default 10MB each)
- **Truncation**: If output exceeds limits, it's truncated with a notice

### Timeout

- Default: Configured per action in the database
- Behavior: Process is killed (SIGKILL) if timeout is exceeded
- Error: Execution marked as timed out

## Building Native Binaries

### Rust Example

```bash
# Build release binary
cargo build --release --package mypack-action

# Copy to pack directory
cp target/release/mypack-action packs/mypack/actions/
```

### Go Example

```bash
# Build static binary
CGO_ENABLED=0 go build -o my_action -ldflags="-s -w" main.go

# Copy to pack directory
cp my_action packs/mypack/actions/
```

### Make Executable

```bash
chmod +x packs/mypack/actions/my_action
```

## Advantages

- **Performance**: No interpreter overhead, direct execution
- **Dependencies**: No runtime installation required (self-contained binaries)
- **Type Safety**: Compile-time checks for Rust/Go/C++
- **Security**: No script injection vulnerabilities
- **Portability**: Single binary can be distributed

## Limitations

- **Platform-specific**: Binaries must be compiled for the target OS/architecture
- **Deployment**: Requires binary recompilation for updates
- **Debugging**: Stack traces may be less readable than scripts
- **Development cycle**: Slower iteration compared to interpreted languages

## Worker Capabilities

The worker service advertises native runtime support in its capabilities:

```json
{
  "runtimes": ["native", "python", "shell", "node"],
  "max_concurrent_executions": 10
}
```

## Database Schema

Runtime entries in the `runtime` table:

```sql
-- Native Action Runtime
INSERT INTO runtime (ref, pack_ref, name, description, runtime_type, distributions, installation)
VALUES (
    'core.action.native',
    'core',
    'Native Action Runtime',
    'Execute actions as native compiled binaries',
    'action',
    '["native"]'::jsonb,
    '{"method": "binary", "description": "Native executable - no runtime installation required"}'::jsonb
);

-- Native Sensor Runtime
INSERT INTO runtime (ref, pack_ref, name, description, runtime_type, distributions, installation)
VALUES (
    'core.sensor.native',
    'core',
    'Native Sensor Runtime',
    'Execute sensors as native compiled binaries',
    'sensor',
    '["native"]'::jsonb,
    '{"method": "binary", "description": "Native executable - no runtime installation required"}'::jsonb
);
```

## Best Practices

1. **Error Handling**: Always handle errors gracefully and exit with appropriate codes
2. **Logging**: Use structured logging (JSON) for better observability
3. **Validation**: Validate input parameters before processing
4. **Timeout Awareness**: Handle long-running operations with progress reporting
5. **Graceful Shutdown**: Listen for SIGTERM and clean up resources
6. **Binary Size**: Strip debug symbols for production (`-ldflags="-s -w"` in Go, `--release` in Rust)
7. **Testing**: Test binaries independently before deploying to Attune
8. **Versioning**: Include version info in binary metadata

## Troubleshooting

### Binary Not Found

- Check the binary exists in `{packs_base_dir}/{pack_ref}/actions/{entrypoint}`
- Verify `packs_base_dir` configuration
- Check file permissions

### Permission Denied

```bash
chmod +x packs/mypack/actions/my_binary
```

### Wrong Architecture

Ensure binary is compiled for the target platform:
- Linux x86_64 for most cloud deployments
- Use `file` command to check binary format

### Missing Dependencies

Use static linking to avoid runtime library dependencies:
- Rust: Use `musl` target for fully static binaries
- Go: Use `CGO_ENABLED=0`

## See Also

- [Worker Service Architecture](worker-service.md)
- [Action Development Guide](actions.md)
- [Sensor Architecture](sensor-architecture.md)
- [Timer Sensor Implementation](../crates/core-timer-sensor/README.md)
