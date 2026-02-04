# Log Size Limits

## Overview

The log size limits feature prevents Out-of-Memory (OOM) issues when actions produce large amounts of output. Instead of buffering all stdout/stderr in memory, the worker service streams logs with configurable size limits and adds truncation notices when limits are exceeded.

## Configuration

Log size limits are configured in the worker configuration:

```yaml
worker:
  max_stdout_bytes: 10485760  # 10MB (default)
  max_stderr_bytes: 10485760  # 10MB (default)
  stream_logs: true           # Enable log streaming (default)
```

Or via environment variables:

```bash
ATTUNE__WORKER__MAX_STDOUT_BYTES=10485760
ATTUNE__WORKER__MAX_STDERR_BYTES=10485760
ATTUNE__WORKER__STREAM_LOGS=true
```

## How It Works

### 1. Streaming Architecture

Instead of using `wait_with_output()` which buffers all output in memory, the worker:

1. Spawns the process with piped stdout/stderr
2. Creates `BoundedLogWriter` instances for each stream
3. Reads output line-by-line concurrently
4. Writes to bounded writers that enforce size limits
5. Waits for process completion while streaming continues

### 2. Truncation Behavior

When output exceeds the configured limit:

1. The writer stops accepting new data after reaching the effective limit (configured limit - 128 byte reserve)
2. A truncation notice is appended to the log
3. Additional output is counted but discarded
4. The execution result includes truncation metadata

**Truncation Notices:**
- **stdout**: `[OUTPUT TRUNCATED: stdout exceeded size limit]`
- **stderr**: `[OUTPUT TRUNCATED: stderr exceeded size limit]`

### 3. Execution Result Metadata

The `ExecutionResult` struct includes truncation information:

```rust
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    // ... other fields ...
    
    // Truncation metadata
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub stdout_bytes_truncated: usize,
    pub stderr_bytes_truncated: usize,
}
```

**Example:**
```json
{
  "stdout": "Line 1\nLine 2\n...\nLine 100\n\n[OUTPUT TRUNCATED: stdout exceeded size limit]\n",
  "stderr": "",
  "stdout_truncated": true,
  "stderr_truncated": false,
  "stdout_bytes_truncated": 950000,
  "exit_code": 0
}
```

## Implementation Details

### BoundedLogWriter

The core component is `BoundedLogWriter`, which implements `AsyncWrite`:

- **Reserve Space**: Reserves 128 bytes for the truncation notice
- **Line-by-Line Reading**: Reads output line-by-line to ensure clean truncation boundaries
- **No Backpressure**: Always reports successful writes to avoid blocking the process
- **Concurrent Streaming**: stdout and stderr are streamed concurrently using `tokio::join!`

### Runtime Integration

All runtimes (Python, Shell, Local) use the streaming approach:

1. **Python Runtime**: `execute_with_streaming()` method handles both `-c` and file execution
2. **Shell Runtime**: `execute_with_streaming()` method handles both `-c` and file execution
3. **Local Runtime**: Delegates to Python/Shell, inheriting streaming behavior

### Memory Safety

Without log size limits:
- Action outputting 1GB → Worker uses 1GB+ memory
- 10 concurrent large actions → 10GB+ memory usage → OOM

With log size limits (10MB default):
- Action outputting 1GB → Worker uses ~10MB per action
- 10 concurrent large actions → ~100MB memory usage
- Safe and predictable memory usage

## Examples

### Action with Large Output

**Action:**
```python
# outputs 100MB
for i in range(1000000):
    print(f"Line {i}: " + "x" * 100)
```

**Result (with 10MB limit):**
```json
{
  "exit_code": 0,
  "stdout": "[first 10MB of output]\n\n[OUTPUT TRUNCATED: stdout exceeded size limit]\n",
  "stdout_truncated": true,
  "stdout_bytes_truncated": 90000000,
  "duration_ms": 1234
}
```

### Action with Large stderr

**Action:**
```python
import sys
# outputs 50MB to stderr
for i in range(500000):
    sys.stderr.write(f"Warning {i}\n")
```

**Result (with 10MB limit):**
```json
{
  "exit_code": 0,
  "stdout": "",
  "stderr": "[first 10MB of warnings]\n\n[OUTPUT TRUNCATED: stderr exceeded size limit]\n",
  "stderr_truncated": true,
  "stderr_bytes_truncated": 40000000,
  "duration_ms": 2345
}
```

### No Truncation (Under Limit)

**Action:**
```python
print("Hello, World!")
```

**Result:**
```json
{
  "exit_code": 0,
  "stdout": "Hello, World!\n",
  "stderr": "",
  "stdout_truncated": false,
  "stderr_truncated": false,
  "stdout_bytes_truncated": 0,
  "stderr_bytes_truncated": 0,
  "duration_ms": 45
}
```

## API Access

### Execution Result

When retrieving execution results via the API, truncation metadata is included:

```bash
curl http://localhost:8080/api/v1/executions/123
```

**Response:**
```json
{
  "data": {
    "id": 123,
    "status": "succeeded",
    "result": {
      "stdout": "...[OUTPUT TRUNCATED]...",
      "stderr": "",
      "exit_code": 0
    },
    "stdout_truncated": true,
    "stderr_truncated": false,
    "stdout_bytes_truncated": 1500000
  }
}
```

## Best Practices

### 1. Configure Appropriate Limits

Choose limits based on your use case:

- **Small actions** (< 1MB output): Use default 10MB limit
- **Data processing** (moderate output): Consider 50-100MB
- **Log analysis** (large output): Consider 100-500MB
- **Never**: Set to unlimited (risks OOM)

### 2. Design Actions for Limited Logs

Instead of printing all data:

```python
# BAD: Prints entire dataset
for item in large_dataset:
    print(item)
```

Use structured output:

```python
# GOOD: Print summary, store data elsewhere
print(f"Processed {len(large_dataset)} items")
print(f"Results saved to: {output_file}")
```

### 3. Monitor Truncation

Track truncation events:
- Alert if many executions are truncated
- May indicate actions need refactoring
- Or limits need adjustment

### 4. Use Artifacts for Large Data

For large outputs, use artifacts:

```python
import json

# Write large data to artifact
with open('/tmp/results.json', 'w') as f:
    json.dump(large_results, f)

# Print only summary
print(f"Results written: {len(large_results)} items")
```

## Performance Impact

### Before (Buffered Output)

- **Memory**: O(output_size) per execution
- **Risk**: OOM on large output
- **Speed**: Fast (no streaming overhead)

### After (Streaming with Limits)

- **Memory**: O(limit_size) per execution, bounded
- **Risk**: No OOM, predictable memory usage
- **Speed**: Minimal overhead (~1-2% for line-by-line reading)
- **Safety**: Production-ready

## Testing

Test log truncation in your actions:

```python
import sys

def test_truncation():
    # Output 20MB (exceeds 10MB limit)
    for i in range(200000):
        print("x" * 100)
    
    # This line won't appear in output if truncated
    print("END")
    
    # But execution still completes successfully
    return {"status": "success"}
```

Check truncation in result:
```python
if result.stdout_truncated:
    print(f"Output was truncated by {result.stdout_bytes_truncated} bytes")
```

## Troubleshooting

### Issue: Important output is truncated

**Solution**: Refactor action to:
1. Print only essential information
2. Store detailed data in artifacts
3. Use structured logging

### Issue: Need to see all output for debugging

**Solution**: Temporarily increase limits:
```yaml
worker:
  max_stdout_bytes: 104857600  # 100MB for debugging
```

### Issue: Memory usage still high

**Check**:
1. Are limits configured correctly?
2. Are multiple workers running with high concurrency?
3. Are artifacts consuming memory?

## Limitations

1. **Line Boundaries**: Truncation happens at line boundaries, so the last line before truncation is included completely
2. **Binary Output**: Only text output is supported; binary output may be corrupted
3. **Reserve Space**: 128 bytes reserved for truncation notice reduces effective limit
4. **No Rotation**: Logs don't rotate; truncation is permanent

## Future Enhancements

Potential improvements:

1. **Log Rotation**: Rotate logs to files instead of truncation
2. **Compressed Storage**: Store truncated logs compressed
3. **Streaming API**: Stream logs in real-time via WebSocket
4. **Per-Action Limits**: Configure limits per action
5. **Smart Truncation**: Preserve first N bytes and last M bytes

## Related Features

- **Artifacts**: Store large output as artifacts instead of logs
- **Timeouts**: Prevent runaway processes (separate from log limits)
- **Resource Limits**: CPU/memory limits for actions (future)

## See Also

- [Worker Configuration](worker-configuration.md)
- [Runtime Architecture](runtime-architecture.md)
- [Performance Tuning](performance-tuning.md)