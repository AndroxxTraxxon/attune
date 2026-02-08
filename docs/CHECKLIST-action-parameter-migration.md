# Checklist: Migrating Actions to Stdin Parameter Delivery & Output Format

**Purpose:** Convert existing actions from environment variable-based parameter handling to secure stdin-based JSON parameter delivery, and ensure proper output format configuration.

**Target Audience:** Pack developers updating existing actions or creating new ones.

---

## Pre-Migration

- [ ] **Review current action** - Understand what parameters it uses
- [ ] **Identify sensitive parameters** - Note which params are secrets (API keys, passwords, tokens)
- [ ] **Check dependencies** - Ensure `jq` available for bash actions
- [ ] **Backup original files** - Copy action scripts before modifying
- [ ] **Read reference docs** - Review `attune/docs/QUICKREF-action-parameters.md`

---

## YAML Configuration Updates

- [ ] **Add parameter delivery config** to action YAML:
  ```yaml
  # Parameter delivery: stdin for secure parameter passing (no env vars)
  parameter_delivery: stdin
  parameter_format: json
  ```

- [ ] **Mark sensitive parameters** with `secret: true`:
  ```yaml
  parameters:
    properties:
      api_key:
        type: string
        secret: true  # ← Add this
  ```

- [ ] **Validate YAML syntax** - Run: `python3 -c "import yaml; yaml.safe_load(open('action.yaml'))"`

### Add Output Format Configuration

- [ ] **Add `output_format` field** to action YAML:
  ```yaml
  # Output format: text, json, or yaml
  output_format: text  # or json, or yaml
  ```

- [ ] **Choose appropriate format:**
  - `text` - Plain text output (simple messages, logs, unstructured data)
  - `json` - JSON structured data (API responses, complex results)
  - `yaml` - YAML structured data (human-readable configuration)

### Update Output Schema

- [ ] **Remove execution metadata** from output schema:
  ```yaml
  # DELETE these from output_schema:
  stdout:       # ❌ Automatically captured
    type: string
  stderr:       # ❌ Automatically captured
    type: string
  exit_code:    # ❌ Automatically captured
    type: integer
  ```

- [ ] **For text format actions** - Remove or simplify output schema:
  ```yaml
  output_format: text
  # Output schema: not applicable for text output format
  # The action outputs plain text to stdout
  ```

- [ ] **For json/yaml format actions** - Keep schema describing actual data:
  ```yaml
  output_format: json
  # Output schema: describes the JSON structure written to stdout
  output_schema:
    type: object
    properties:
      count:
        type: integer
      items:
        type: array
        items:
          type: string
      # No stdout/stderr/exit_code
  ```

---

## Bash/Shell Script Migration

### Remove Environment Variable Reading

- [ ] **Delete all `ATTUNE_ACTION_*` references**:
  ```bash
  # DELETE these lines:
  MESSAGE="${ATTUNE_ACTION_MESSAGE:-default}"
  COUNT="${ATTUNE_ACTION_COUNT:-1}"
  API_KEY="${ATTUNE_ACTION_API_KEY}"
  ```

### Add Stdin JSON Reading

- [ ] **Add stdin input reading** at script start:
  ```bash
  #!/bin/bash
  set -e
  set -o pipefail
  
  # Read JSON parameters from stdin
  INPUT=$(cat)
  ```

- [ ] **Parse parameters with jq**:
  ```bash
  MESSAGE=$(echo "$INPUT" | jq -r '.message // "default"')
  COUNT=$(echo "$INPUT" | jq -r '.count // 1')
  API_KEY=$(echo "$INPUT" | jq -r '.api_key // ""')
  ```

### Handle Optional Parameters

- [ ] **Add null checks for optional params**:
  ```bash
  if [ -n "$API_KEY" ] && [ "$API_KEY" != "null" ]; then
      # Use API key
  fi
  ```

### Boolean Parameters

- [ ] **Handle boolean values correctly** (jq outputs lowercase):
  ```bash
  ENABLED=$(echo "$INPUT" | jq -r '.enabled // false')
  if [ "$ENABLED" = "true" ]; then
      # Feature enabled
  fi
  ```

### Array Parameters

- [ ] **Parse arrays with jq -c**:
  ```bash
  ITEMS=$(echo "$INPUT" | jq -c '.items // []')
  ITEM_COUNT=$(echo "$ITEMS" | jq 'length')
  ```

---

## Python Script Migration

### Remove Environment Variable Reading

- [ ] **Delete `os.environ` references**:
  ```python
  # DELETE these lines:
  import os
  message = os.environ.get('ATTUNE_ACTION_MESSAGE', 'default')
  ```

- [ ] **Remove environment helper functions** like `get_env_param()`, `parse_json_param()`, etc.

### Add Stdin JSON Reading

- [ ] **Add parameter reading function**:
  ```python
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
  ```

- [ ] **Call reading function in main()**:
  ```python
  def main():
      params = read_parameters()
      message = params.get('message', 'default')
      count = params.get('count', 1)
  ```

### Update Parameter Access

- [ ] **Replace all parameter reads** with `.get()`:
  ```python
  # OLD: get_env_param('message', 'default')
  # NEW: params.get('message', 'default')
  ```

- [ ] **Update required parameter validation**:
  ```python
  if not params.get('url'):
      print("ERROR: 'url' parameter is required", file=sys.stderr)
      sys.exit(1)
  ```

---

## Node.js Script Migration

### Remove Environment Variable Reading

- [ ] **Delete `process.env` references**:
  ```javascript
  // DELETE these lines:
  const message = process.env.ATTUNE_ACTION_MESSAGE || 'default';
  ```

### Add Stdin JSON Reading

- [ ] **Add parameter reading function**:
  ```javascript
  const readline = require('readline');
  
  async function readParameters() {
      const rl = readline.createInterface({
          input: process.stdin,
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
  ```

- [ ] **Update main function** to use async/await:
  ```javascript
  async function main() {
      const params = await readParameters();
      const message = params.message || 'default';
  }
  
  main().catch(err => {
      console.error('ERROR:', err.message);
      process.exit(1);
  });
  ```

---

## Testing

### Local Testing

- [ ] **Test with specific parameters**:
  ```bash
  echo '{"message": "test", "count": 5}' | ./action.sh
  ```

- [ ] **Test with empty JSON (defaults)**:
  ```bash
  echo '{}' | ./action.sh
  ```

- [ ] **Test with file input**:
  ```bash
  cat test-params.json | ./action.sh
  ```

- [ ] **Test required parameters** - Verify error when missing:
  ```bash
  echo '{"count": 5}' | ./action.sh  # Should fail if 'message' required
  ```

- [ ] **Test optional parameters** - Verify defaults work:
  ```bash
  echo '{"message": "test"}' | ./action.sh  # count should use default
  ```

- [ ] **Test null handling**:
  ```bash
  echo '{"message": "test", "api_key": null}' | ./action.sh
  ```

### Integration Testing

- [ ] **Test via Attune API** - Execute action through API endpoint
- [ ] **Test in workflow** - Run action as part of a workflow
- [ ] **Test with secrets** - Verify secret parameters are not exposed
- [ ] **Verify no env var exposure** - Check `ps` output during execution

---

## Security Review

- [ ] **No secrets in logs** - Ensure sensitive params aren't printed
- [ ] **No parameter echoing** - Don't include input JSON in error messages
- [ ] **Generic error messages** - Don't expose parameter values in errors
- [ ] **Marked all secrets** - All sensitive parameters have `secret: true`

---

## Documentation

- [ ] **Update action README** - Document parameter changes if exists
- [ ] **Add usage examples** - Show how to call action with new format
- [ ] **Update pack CHANGELOG** - Note breaking change from env vars to stdin
- [ ] **Document default values** - List all parameter defaults

---

## Post-Migration Cleanup

- [ ] **Remove old helper functions** - Delete unused env var parsers
- [ ] **Remove unused imports** - Clean up `os` import in Python if not needed
- [ ] **Update comments** - Fix any comments mentioning environment variables
- [ ] **Validate YAML again** - Final check of action.yaml syntax
- [ ] **Run linters** - `shellcheck` for bash, `pylint`/`flake8` for Python
- [ ] **Commit changes** - Commit with clear message about stdin migration

---

## Verification

- [ ] **Script runs with stdin** - Basic execution works
- [ ] **Defaults work correctly** - Empty JSON triggers default values
- [ ] **Required params validated** - Missing required params cause error
- [ ] **Optional params work** - Optional params with null/missing handled
- [ ] **Exit codes correct** - Success = 0, errors = non-zero
- [ ] **Output format unchanged** - Stdout/stderr output still correct
- [ ] **No breaking changes to output** - JSON output schema maintained

---

## Example: Complete Migration

### Before (Environment Variables)

```bash
#!/bin/bash
set -e

MESSAGE="${ATTUNE_ACTION_MESSAGE:-Hello}"
COUNT="${ATTUNE_ACTION_COUNT:-1}"

echo "Message: $MESSAGE (repeated $COUNT times)"
```

### After (Stdin JSON)

```bash
#!/bin/bash
set -e
set -o pipefail

# Read JSON parameters from stdin
INPUT=$(cat)

# Parse parameters with defaults
MESSAGE=$(echo "$INPUT" | jq -r '.message // "Hello"')
COUNT=$(echo "$INPUT" | jq -r '.count // 1')

# Validate required parameters
if ! [[ "$COUNT" =~ ^[0-9]+$ ]]; then
    echo "ERROR: count must be a positive integer" >&2
    exit 1
fi

echo "Message: $MESSAGE (repeated $COUNT times)"
```

---

## References

- [Quick Reference: Action Parameters](./QUICKREF-action-parameters.md)
- [Quick Reference: Action Output Format](./QUICKREF-action-output-format.md)
- [Core Pack Actions README](../packs/core/actions/README.md)
- [Worker Service Architecture](./architecture/worker-service.md)

---

## Common Issues

### Issue: `jq: command not found`
**Solution:** Ensure `jq` is installed in worker container/environment

### Issue: Parameters showing as `null`
**Solution:** Check for both empty string and "null" literal:
```bash
if [ -n "$PARAM" ] && [ "$PARAM" != "null" ]; then
```

### Issue: Boolean not working as expected
**Solution:** jq outputs lowercase "true"/"false", compare as strings:
```bash
if [ "$ENABLED" = "true" ]; then
```

### Issue: Array not parsing correctly
**Solution:** Use `jq -c` for compact JSON output:
```bash
ITEMS=$(echo "$INPUT" | jq -c '.items // []')
```

### Issue: Action hangs waiting for input
**Solution:** Ensure JSON is being passed to stdin, or pass empty object:
```bash
echo '{}' | ./action.sh
```

---

## Success Criteria

✅ **Migration complete when:**
- Action reads ALL parameters from stdin JSON
- NO environment variables used for parameters
- All tests pass with new parameter format
- YAML updated with `parameter_delivery: stdin`
- YAML includes `output_format: text|json|yaml`
- Output schema describes data structure only (no stdout/stderr/exit_code)
- Sensitive parameters marked with `secret: true`
- Documentation updated
- Local testing confirms functionality
