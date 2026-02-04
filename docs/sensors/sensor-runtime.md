# Sensor Runtime Execution

**Version:** 1.0  
**Last Updated:** 2024-01-17

---

## Overview

The Sensor Runtime Execution module provides the infrastructure for executing sensor code in multiple runtime environments (Python, Node.js, Shell). Sensors are polled periodically to detect trigger conditions and generate event payloads that drive automated actions in the Attune platform.

---

## Architecture

### Components

1. **SensorRuntime** - Main executor that manages sensor execution across runtimes
2. **Runtime Wrappers** - Language-specific wrappers (Python, Node.js) that execute sensor code
3. **Output Parser** - Parses sensor output and extracts event payloads
4. **Validator** - Validates runtime availability and configuration

### Execution Flow

```
SensorManager
    ↓
Poll Sensor (every N seconds)
    ↓
SensorRuntime.execute_sensor()
    ↓ (based on runtime_ref)
├─→ execute_python_sensor()
├─→ execute_nodejs_sensor()
└─→ execute_shell_sensor()
    ↓
Generate wrapper script
    ↓
Execute in subprocess (with timeout)
    ↓
Parse output as JSON
    ↓
Extract event payloads
    ↓
Return SensorExecutionResult
    ↓
EventGenerator.generate_event() (for each payload)
    ↓
RuleMatcher.match_event()
    ↓
Create Enforcements
```

---

## Supported Runtimes

### Python (`python` / `python3`)

**Sensor Format:**
```python
def poll_sensor(config: Dict[str, Any]) -> Iterator[Dict[str, Any]]:
    """
    Sensor entrypoint function.
    
    Args:
        config: Sensor configuration (from sensor.param_schema)
    
    Yields:
        Event payloads as dictionaries
    """
    # Check for trigger condition
    if condition_detected():
        yield {
            "message": "Event detected",
            "timestamp": datetime.now().isoformat(),
            "data": {...}
        }
```

**Features:**
- Supports generator functions (yield multiple events)
- Supports regular functions (return single event)
- Configuration passed as dictionary
- Automatic JSON serialization of output
- Traceback capture on errors

### Node.js (`nodejs` / `node`)

**Sensor Format:**
```javascript
async function poll_sensor(config) {
    /**
     * Sensor entrypoint function.
     * 
     * @param {Object} config - Sensor configuration
     * @returns {Array<Object>} Array of event payloads
     */
    const events = [];
    
    // Check for trigger condition
    if (conditionDetected()) {
        events.push({
            message: "Event detected",
            timestamp: new Date().toISOString(),
            data: {...}
        });
    }
    
    return events;
}
```

**Features:**
- Supports async functions
- Returns array of event payloads
- Configuration passed as object
- Automatic JSON serialization
- Stack trace capture on errors

### Shell (`shell` / `bash`)

**Sensor Format:**
```bash
#!/bin/bash
# Sensor entrypoint is the shell command itself

# Access configuration via SENSOR_CONFIG environment variable
config=$(echo "$SENSOR_CONFIG" | jq -r '.')

# Check for trigger condition
if [[ condition_detected ]]; then
    # Output JSON with events array
    echo '{"events": [{"message": "Event detected", "timestamp": "'$(date -Iseconds)'"}], "count": 1}'
fi

# No events
echo '{"events": [], "count": 0}'
```

**Features:**
- Direct shell command execution
- Configuration via `SENSOR_CONFIG` env var
- Must output JSON with `events` array
- Access to all shell utilities
- Lightweight for simple checks

---

## Configuration

### SensorRuntime Configuration

```rust
use std::path::PathBuf;

let runtime = SensorRuntime::with_config(
    PathBuf::from("/tmp/attune/sensors"),    // work_dir
    PathBuf::from("python3"),                 // python_path
    PathBuf::from("node"),                    // node_path
    30,                                       // timeout_secs
);
```

**Default Configuration:**
- `work_dir`: `/tmp/attune/sensors`
- `python_path`: `python3`
- `node_path`: `node`
- `timeout_secs`: `30`

### Environment Variables

Sensors receive these environment variables:

- `SENSOR_REF` - Sensor reference (e.g., `mypack.file_watcher`)
- `TRIGGER_REF` - Trigger reference (e.g., `mypack.file_changed`)
- `SENSOR_CONFIG` - JSON configuration (shell sensors only)

---

## Output Format

### Success

Sensors must output JSON in this format:

```json
{
    "events": [
        {
            "message": "File created",
            "path": "/tmp/test.txt",
            "size": 1024
        },
        {
            "message": "File modified",
            "path": "/tmp/data.json",
            "size": 2048
        }
    ],
    "count": 2
}
```

**Fields:**
- `events` (required): Array of event payloads (each becomes a separate Event)
- `count` (optional): Number of events (for validation)

### Error

If sensor execution fails:

```json
{
    "error": "Connection timeout",
    "error_type": "TimeoutError",
    "traceback": "...",
    "stack": "..."
}
```

**Exit Codes:**
- `0` - Success (events will be processed)
- Non-zero - Failure (error logged, no events generated)

---

## SensorExecutionResult

### Structure

```rust
pub struct SensorExecutionResult {
    /// Sensor reference
    pub sensor_ref: String,
    
    /// Event payloads generated by the sensor
    pub events: Vec<JsonValue>,
    
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    
    /// Standard output
    pub stdout: String,
    
    /// Standard error
    pub stderr: String,
    
    /// Error message if execution failed
    pub error: Option<String>,
}
```

### Methods

```rust
// Check if execution was successful
result.is_success() -> bool

// Get number of events generated
result.event_count() -> usize
```

---

## Error Handling

### Timeout

If sensor execution exceeds timeout:

```rust
SensorExecutionResult {
    sensor_ref: "mypack.sensor",
    events: vec![],
    duration_ms: 30000,
    error: Some("Sensor execution timed out after 30 seconds"),
    ...
}
```

### Runtime Not Found

If runtime is not available:

```rust
Error: "Unsupported sensor runtime: unknown_runtime"
```

### Invalid Output

If sensor output is not valid JSON:

```rust
SensorExecutionResult {
    sensor_ref: "mypack.sensor",
    events: vec![],
    error: Some("Failed to parse sensor output: expected value at line 1 column 1"),
    ...
}
```

### Output Size Limit

Maximum output size: **10MB**

If exceeded, output is truncated and warning logged.

---

## Integration with Sensor Manager

### Polling Loop

```rust
// In SensorManager::poll_sensor()

// 1. Execute sensor
let execution_result = sensor_runtime
    .execute_sensor(sensor, trigger, None)
    .await?;

// 2. Check success
if !execution_result.is_success() {
    return Err(anyhow!("Sensor execution failed: {}", error));
}

// 3. Generate events for each payload
for payload in execution_result.events {
    // Create event
    let event_id = event_generator
        .generate_event(sensor, trigger, payload)
        .await?;
    
    // Match rules and create enforcements
    let event = event_generator.get_event(event_id).await?;
    let enforcement_ids = rule_matcher.match_event(&event).await?;
}
```

---

## Example Sensors

### Python: File Watcher

```python
import os
from pathlib import Path
from typing import Dict, Any, Iterator

def poll_sensor(config: Dict[str, Any]) -> Iterator[Dict[str, Any]]:
    """Watch directory for new files."""
    watch_path = Path(config.get('path', '/tmp'))
    last_check_file = Path('/tmp/last_check.txt')
    
    # Get last check time
    if last_check_file.exists():
        last_check = float(last_check_file.read_text())
    else:
        last_check = 0
    
    current_time = time.time()
    
    # Find new files
    for file_path in watch_path.iterdir():
        if file_path.is_file():
            mtime = file_path.stat().st_mtime
            if mtime > last_check:
                yield {
                    "event_type": "file_created",
                    "path": str(file_path),
                    "size": file_path.stat().st_size,
                    "modified": datetime.fromtimestamp(mtime).isoformat()
                }
    
    # Update last check time
    last_check_file.write_text(str(current_time))
```

### Node.js: HTTP Endpoint Monitor

```javascript
const https = require('https');

async function poll_sensor(config) {
    const url = config.url || 'https://example.com';
    const timeout = config.timeout || 5000;
    
    return new Promise((resolve) => {
        const start = Date.now();
        
        https.get(url, { timeout }, (res) => {
            const duration = Date.now() - start;
            const events = [];
            
            // Check if status changed or response time is high
            if (res.statusCode !== 200) {
                events.push({
                    event_type: "endpoint_down",
                    url: url,
                    status_code: res.statusCode,
                    response_time_ms: duration
                });
            } else if (duration > 1000) {
                events.push({
                    event_type: "endpoint_slow",
                    url: url,
                    response_time_ms: duration
                });
            }
            
            resolve(events);
        }).on('error', (err) => {
            resolve([{
                event_type: "endpoint_error",
                url: url,
                error: err.message
            }]);
        });
    });
}
```

### Shell: Disk Usage Monitor

```bash
#!/bin/bash
# Monitor disk usage and alert if threshold exceeded

THRESHOLD=${THRESHOLD:-80}

usage=$(df -h / | awk 'NR==2 {print $5}' | sed 's/%//')

if [ "$usage" -gt "$THRESHOLD" ]; then
    echo "{\"events\": [{\"event_type\": \"disk_full\", \"usage_percent\": $usage, \"threshold\": $THRESHOLD}], \"count\": 1}"
else
    echo "{\"events\": [], \"count\": 0}"
fi
```

---

## Testing

### Unit Tests

```rust
#[test]
fn test_parse_sensor_output_success() {
    let runtime = SensorRuntime::new();
    let output = r#"{"events": [{"key": "value"}], "count": 1}"#;
    
    let result = runtime.parse_sensor_output(
        &sensor,
        output.as_bytes().to_vec(),
        vec![],
        Some(0)
    ).unwrap();
    
    assert!(result.is_success());
    assert_eq!(result.event_count(), 1);
}
```

### Integration Tests

See `docs/testing-status.md` for sensor runtime integration test requirements.

---

## Performance Considerations

### Timeouts

- **Default:** 30 seconds
- **Recommended:** 10-60 seconds depending on sensor complexity
- **Maximum:** No hard limit, but keep reasonable to avoid blocking

### Polling Intervals

- **Default:** 30 seconds
- **Minimum:** 5 seconds (avoid excessive load)
- **Typical:** 30-300 seconds depending on use case

### Resource Usage

- Each sensor runs in a subprocess (isolated)
- Subprocesses are short-lived (created per poll)
- Maximum 10MB output per execution
- Concurrent sensor execution (multiple sensors can run simultaneously)

---

## Security Considerations

### Code Execution

- Sensors execute arbitrary code (use with caution)
- Run sensor service with minimal privileges
- Consider containerization for production
- Validate sensor code before deployment

### Input Validation

- Configuration is passed as untrusted input
- Sensors should validate all config parameters
- Use schema validation (param_schema)

### Output Sanitization

- Output is parsed as JSON (injection safe)
- Large outputs are truncated (DoS prevention)
- stderr is logged but not exposed to users

---

## Troubleshooting

### Sensor Not Executing

**Symptom:** Sensor polls but generates no events

**Checks:**
1. Verify sensor is enabled (`sensor.enabled = true`)
2. Check sensor logs for execution errors
3. Test sensor code manually
4. Verify runtime is available (`python3 --version`)

### Runtime Not Found

**Symptom:** Error "Unsupported sensor runtime"

**Solution:**
```bash
# Verify Python
which python3
python3 --version

# Verify Node.js
which node
node --version

# Update SensorRuntime config if needed
```

### Timeout Issues

**Symptom:** Sensor execution times out

**Solutions:**
1. Increase timeout in SensorRuntime config
2. Optimize sensor code (reduce external calls)
3. Split into multiple sensors
4. Use asynchronous operations

### Invalid JSON Output

**Symptom:** "Failed to parse sensor output"

**Solution:**
1. Test sensor output format
2. Ensure `events` array exists
3. Validate JSON with `jq` or similar
4. Check for syntax errors in sensor code

---

## Future Enhancements

### Planned Features

- [ ] Container runtime support (Docker/Podman)
- [ ] Sensor code caching (avoid regenerating wrappers)
- [ ] Streaming output support (for long-running sensors)
- [ ] Sensor debugging mode (verbose logging)
- [ ] Runtime health checks (automatic failover)
- [ ] Pack storage integration (load sensor code from packs)

---

## API Reference

### SensorRuntime

```rust
impl SensorRuntime {
    /// Create with default configuration
    pub fn new() -> Self;
    
    /// Create with custom configuration
    pub fn with_config(
        work_dir: PathBuf,
        python_path: PathBuf,
        node_path: PathBuf,
        timeout_secs: u64,
    ) -> Self;
    
    /// Execute a sensor and return event payloads
    pub async fn execute_sensor(
        &self,
        sensor: &Sensor,
        trigger: &Trigger,
        config: Option<JsonValue>,
    ) -> Result<SensorExecutionResult>;
    
    /// Validate runtime configuration
    pub async fn validate(&self) -> Result<()>;
}
```

---

## See Also

- [Sensor Service Architecture](sensor-service.md)
- [Sensor Service Setup](sensor-service-setup.md)
- [Testing Status](../testing-status.md)
- [Worker Runtime Documentation](../TODO.md) (when available)

---

**Status:** ✅ Implemented and Tested  
**Next Steps:** Pack storage integration for sensor code loading