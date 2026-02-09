# Action Output Formats

## Overview

Attune actions can specify how their output should be parsed and stored in the execution result. This allows actions to produce structured data (JSON, YAML, JSON Lines) or plain text output, and have it automatically parsed and stored in the `execution.result` field.

## Output Format Types

### `text` (Default)

**Use Case**: Simple actions that produce human-readable output without structured data.

**Behavior**: 
- No parsing is performed on stdout
- Full stdout content is captured in `execution.stdout`
- `execution.result` field is `null`

**Example Action**:
```yaml
name: echo
output_format: text
```

**Example Output**:
```
Hello, World!
```

**Execution Result**:
```json
{
  "exit_code": 0,
  "succeeded": true,
  "stdout": "Hello, World!",
  "data": null
}
```

---

### `json`

**Use Case**: Actions that produce a single JSON object or value as their final output.

**Behavior**:
- Parses the **last line** of stdout as JSON
- Stores parsed JSON in `execution.result`
- Full stdout still available in `execution.stdout`
- If parsing fails, `result` is `null` (no error)

**Example Action**:
```yaml
name: http_request
output_format: json
output_schema:
  type: object
  properties:
    status_code:
      type: integer
    body:
      type: string
    elapsed_ms:
      type: integer
```

**Example Output**:
```
Connecting to example.com...
Request sent
{"status_code": 200, "body": "{\"message\":\"ok\"}", "elapsed_ms": 142}
```

**Execution Result**:
```json
{
  "exit_code": 0,
  "succeeded": true,
  "stdout": "Connecting to example.com...\nRequest sent\n{...}",
  "data": {
    "status_code": 200,
    "body": "{\"message\":\"ok\"}",
    "elapsed_ms": 142
  }
}
```

---

### `yaml`

**Use Case**: Actions that produce YAML-formatted output, common in configuration management and infrastructure tools.

**Behavior**:
- Parses **entire stdout** as YAML
- Stores parsed data in `execution.result`
- Full stdout still available in `execution.stdout`
- If parsing fails, `result` is `null` (no error)

**Example Action**:
```yaml
name: get_config
output_format: yaml
output_schema:
  type: object
  properties:
    version:
      type: string
    settings:
      type: object
```

**Example Output**:
```yaml
version: "1.2.3"
settings:
  enabled: true
  max_retries: 3
  timeout: 30
```

**Execution Result**:
```json
{
  "exit_code": 0,
  "succeeded": true,
  "stdout": "version: \"1.2.3\"\nsettings:\n  enabled: true\n...",
  "data": {
    "version": "1.2.3",
    "settings": {
      "enabled": true,
      "max_retries": 3,
      "timeout": 30
    }
  }
}
```

---

### `jsonl` (JSON Lines)

**Use Case**: Actions that produce multiple records or streaming results, where each line is a separate JSON object.

**Behavior**:
- Parses **each line** of stdout as a separate JSON object
- Collects all parsed objects into a JSON array
- Stores array in `execution.result`
- Full stdout still available in `execution.stdout`
- Invalid JSON lines are silently skipped
- If no valid JSON lines found, `result` is `null`

**Important**: When using `jsonl`, the `output_schema` root type **must be `array`**.

**Example Action**:
```yaml
name: list_users
output_format: jsonl
output_schema:
  type: array
  items:
    type: object
    properties:
      id:
        type: integer
      username:
        type: string
      email:
        type: string
```

**Example Output**:
```
{"id": 1, "username": "alice", "email": "alice@example.com"}
{"id": 2, "username": "bob", "email": "bob@example.com"}
{"id": 3, "username": "charlie", "email": "charlie@example.com"}
```

**Execution Result**:
```json
{
  "exit_code": 0,
  "succeeded": true,
  "stdout": "{\"id\": 1, ...}\n{\"id\": 2, ...}\n{\"id\": 3, ...}",
  "data": [
    {"id": 1, "username": "alice", "email": "alice@example.com"},
    {"id": 2, "username": "bob", "email": "bob@example.com"},
    {"id": 3, "username": "charlie", "email": "charlie@example.com"}
  ]
}
```

**Benefits**:
- Memory efficient for large datasets (streaming)
- Easy to process line-by-line
- Resilient to partial failures (invalid lines skipped)
- Compatible with standard JSONL tools and libraries

---

## Choosing an Output Format

| Format | Best For | Parsing | Result Type |
|--------|----------|---------|-------------|
| `text` | Simple messages, logs, human output | None | `null` |
| `json` | Single structured result | Last line only | Object/Value |
| `yaml` | Configuration, complex nested data | Entire output | Object/Value |
| `jsonl` | Lists, streaming, multiple records | Each line | Array |

---

## Action Definition Examples

### Text Output Action
```yaml
name: echo
ref: core.echo
output_format: text
entry_point: echo.sh
parameters:
  type: object
  properties:
    message:
      type: string
```

### JSON Output Action
```yaml
name: http_request
ref: core.http_request
output_format: json
entry_point: http_request.sh
output_schema:
  type: object
  properties:
    status_code:
      type: integer
    headers:
      type: object
    body:
      type: string
```

### JSONL Output Action
```yaml
name: list_files
ref: custom.list_files
output_format: jsonl
entry_point: list_files.sh
output_schema:
  type: array
  items:
    type: object
    properties:
      path:
        type: string
      size:
        type: integer
      modified:
        type: string
```

---

## Writing Actions with Structured Output

### JSON Output (Bash)
```bash
#!/bin/bash
# Action script that produces JSON output

# Do work...
result=$(curl -s https://api.example.com/data)
status=$?

# Output JSON on last line
echo "{\"status\": $status, \"data\": \"$result\"}"
```

### JSON Output (Python)
```python
#!/usr/bin/env python3
import json
import sys

# Do work...
result = {"count": 42, "items": ["a", "b", "c"]}

# Output JSON on last line
print(json.dumps(result))
```

### JSONL Output (Bash)
```bash
#!/bin/bash
# Action script that produces JSONL output

# Process items and output one JSON object per line
for item in $(ls -1 /path/to/files); do
    size=$(stat -f%z "$item")
    echo "{\"name\": \"$item\", \"size\": $size}"
done
```

### JSONL Output (Python)
```python
#!/usr/bin/env python3
import json
import os

# Process items and output one JSON object per line
for filename in os.listdir('/path/to/files'):
    info = os.stat(filename)
    record = {
        "name": filename,
        "size": info.st_size,
        "modified": info.st_mtime
    }
    print(json.dumps(record))
```

---

## Error Handling

### Parsing Failures

If output parsing fails:
- The action execution is still considered successful (if exit code is 0)
- `execution.result` is set to `null`
- Full stdout is still captured in `execution.stdout`
- No error is logged (parsing is best-effort)

**Example**: Action has `output_format: json` but produces invalid JSON:
```json
{
  "exit_code": 0,
  "succeeded": true,
  "stdout": "Not valid JSON!",
  "data": null
}
```

### Mixed Output

For `json` and `jsonl` formats, you can still include informational output:

**JSON** - Only last line is parsed:
```
Starting process...
Processing 100 items...
Done!
{"processed": 100, "errors": 0}
```

**JSONL** - Only valid JSON lines are parsed:
```
Starting scan...
{"file": "a.txt", "size": 1024}
{"file": "b.txt", "size": 2048}
Scan complete
```

---

## Output Schema Validation

While the `output_schema` field is used to document expected output structure, Attune does **not** currently validate action output against the schema. The schema serves as:

1. **Documentation** for action consumers
2. **Type hints** for workflow parameter mapping
3. **API documentation** generation
4. **Future validation** (planned feature)

---

## Best Practices

### 1. Choose the Right Format
- Use `text` for simple actions without structured output
- Use `json` for single-result APIs or calculations
- Use `yaml` when working with configuration management tools
- Use `jsonl` for lists, batches, or streaming results

### 2. JSON/JSONL: One JSON Per Line
```bash
# Good - Each JSON on its own line
echo '{"id": 1, "name": "Alice"}'
echo '{"id": 2, "name": "Bob"}'

# Bad - Pretty-printed JSON spans multiple lines
echo '{
  "id": 1,
  "name": "Alice"
}'
```

### 3. Informational Output
- Add logging/progress messages **before** the final JSON line
- For JSONL, non-JSON lines are silently ignored

### 4. Error Messages
- Write errors to **stderr**, not stdout
- Stdout should contain only the structured output
- Use non-zero exit codes for failures

```bash
# Good
if [[ $status -ne 0 ]]; then
    echo "Error: Failed to connect" >&2
    exit 1
fi
echo '{"success": true}'

# Bad - mixes error in stdout
if [[ $status -ne 0 ]]; then
    echo "Error: Failed to connect"
    echo '{"success": false}'
fi
```

### 5. Always Flush Output
```python
# Python - ensure output is written immediately
import sys
print(json.dumps(result))
sys.stdout.flush()
```

```bash
# Bash - automatic, but can force with
echo '{"result": "data"}'
sync
```

---

## Database Schema

The `output_format` field is stored in the `action` table:

```sql
CREATE TABLE action (
    -- ... other columns ...
    output_format TEXT NOT NULL DEFAULT 'text'
        CHECK (output_format IN ('text', 'json', 'yaml', 'jsonl')),
    -- ... other columns ...
);
```

Default value is `'text'` for backward compatibility.

---

## Related Documentation

- [Action Structure](pack-structure.md#actions)
- [Parameter Delivery](parameter-delivery.md)
- [Execution Results](execution-system.md#results)
- [Output Schema](json-schema.md)