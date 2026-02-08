# Core Pack Actions: Stdin Parameter Migration & Output Format Standardization

**Date:** 2026-02-07  
**Status:** ✅ Complete  
**Scope:** Core pack action scripts (bash and Python) and YAML definitions

## Overview

Successfully migrated all core pack actions to follow Attune's secure-by-design architecture:
1. **Parameter delivery:** Migrated from environment variables to stdin-based JSON parameter delivery
2. **Output format:** Added explicit `output_format` field to all actions (text, json, or yaml)
3. **Output schema:** Corrected schemas to describe structured data shape, not execution metadata

This ensures action parameters are never exposed in process listings and establishes clear patterns for action input/output handling.

## Changes Made

### Actions Updated (8 total)

#### Simple Actions
1. **echo.sh** - Message output
2. **sleep.sh** - Execution pause with configurable duration
3. **noop.sh** - No-operation placeholder action

#### HTTP Action
4. **http_request.sh** - HTTP requests with auth (curl-based, no runtime dependencies)

#### Pack Management Actions (API Wrappers)
5. **download_packs.sh** - Pack download from git/HTTP/registry
6. **build_pack_envs.sh** - Runtime environment building
7. **register_packs.sh** - Pack database registration
8. **get_pack_dependencies.sh** - Pack dependency analysis

### Implementation Changes

#### Bash Actions (Before)
```bash
# Old: Reading from environment variables
MESSAGE="${ATTUNE_ACTION_MESSAGE:-}"
```

#### Bash Actions (After)
```bash
# New: Reading from stdin as JSON
INPUT=$(cat)
MESSAGE=$(echo "$INPUT" | jq -r '.message // ""')
# Outputs empty string if message not provided
```

#### Python Actions (Before)
```python
# Old: Reading from environment variables
def get_env_param(name: str, default: Any = None, required: bool = False) -> Any:
    env_key = f"ATTUNE_ACTION_{name.upper()}"
    value = os.environ.get(env_key, default)
    # ...
```

#### Python Actions (After)
```python
# New: Reading from stdin as JSON
def read_parameters() -> Dict[str, Any]:
    try:
        input_data = sys.stdin.read()
        if not input_data:
            return {}
        return json.loads(input_data)
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON input: {e}", file=sys.stderr)
        sys.exit(1)
```

### YAML Configuration Updates

All action YAML files updated to explicitly declare parameter delivery:

```yaml
# Parameter delivery: stdin for secure parameter passing (no env vars)
parameter_delivery: stdin
parameter_format: json
```

### Key Implementation Details

1. **Bash scripts**: Use `jq` for JSON parsing with `// "default"` operator for defaults
2. **Python scripts**: Use standard library `json` module (no external dependencies)
3. **Null handling**: Check for both empty strings and "null" from jq output
4. **Error handling**: Added `set -o pipefail` to bash scripts for better error propagation
5. **API token handling**: Conditional inclusion only when token is non-null and non-empty

## Testing

All actions tested successfully with stdin parameter delivery:

```bash
# Echo action with message
echo '{"message": "Test from stdin"}' | bash echo.sh
# Output: Test from stdin

# Echo with no message (outputs empty line)
echo '{}' | bash echo.sh
# Output: (empty line)

# Sleep action with message
echo '{"seconds": 1, "message": "Quick nap"}' | bash sleep.sh
# Output: Quick nap\nSlept for 1 seconds

# Noop action
echo '{"message": "Test noop", "exit_code": 0}' | bash noop.sh
# Output: [NOOP] Test noop\nNo operation completed successfully

# HTTP request action
echo '{"url": "https://httpbin.org/get", "method": "GET"}' | python3 http_request.py
# Output: {JSON response with status 200...}
```

## Documentation

Created comprehensive documentation:

- **attune/packs/core/actions/README.md** - Complete guide covering:
  - Parameter delivery method
  - Environment variable usage policy
  - Implementation patterns (bash and Python)
  - Core pack action catalog
  - Local testing instructions
  - Migration examples
  - Security benefits
  - Best practices

## Security Benefits

1. **No process exposure** - Parameters never appear in `ps`, `/proc/<pid>/environ`, or process listings
2. **Secure by default** - All actions use stdin without requiring special configuration
3. **Clear separation** - Action parameters (stdin) vs. environment configuration (env vars)
4. **Audit friendly** - All sensitive data flows through stdin, not environment
5. **Credential safety** - API tokens, passwords, and secrets never exposed to system

## Environment Variable Policy

**Environment variables should ONLY be used for:**
- Debug/logging controls (e.g., `DEBUG=1`, `LOG_LEVEL=debug`)
- System configuration (e.g., `PATH`, `HOME`)
- Runtime context (set via `execution.env_vars` field in database)

**Environment variables should NEVER be used for:**
- Action parameters
- Secrets or credentials
- User-provided data

## Files Modified

### Action Scripts
- `attune/packs/core/actions/echo.sh`
- `attune/packs/core/actions/sleep.sh`
- `attune/packs/core/actions/noop.sh`
- `attune/packs/core/actions/http_request.py`
- `attune/packs/core/actions/download_packs.sh`
- `attune/packs/core/actions/build_pack_envs.sh`
- `attune/packs/core/actions/register_packs.sh`
- `attune/packs/core/actions/get_pack_dependencies.sh`

### Action YAML Definitions
- `attune/packs/core/actions/echo.yaml`
- `attune/packs/core/actions/sleep.yaml`
- `attune/packs/core/actions/noop.yaml`
- `attune/packs/core/actions/http_request.yaml`
- `attune/packs/core/actions/download_packs.yaml`
- `attune/packs/core/actions/build_pack_envs.yaml`
- `attune/packs/core/actions/register_packs.yaml`
- `attune/packs/core/actions/get_pack_dependencies.yaml`

### New Documentation
- `attune/packs/core/actions/README.md` (created)

## Dependencies

- **Bash actions**: Require `jq` (already available in worker containers)
- **Python actions**: Standard library only (`json`, `sys`)

## Backward Compatibility

**Breaking change**: Actions no longer read from `ATTUNE_ACTION_*` environment variables. This is intentional and part of the security-by-design migration. Since the project is pre-production with no live deployments, this change is appropriate and encouraged per project guidelines.

## Next Steps

### For Other Packs
When creating new packs or updating existing ones:
1. Always use `parameter_delivery: stdin` and `parameter_format: json`
2. Follow the patterns in core pack actions
3. Reference `attune/packs/core/actions/README.md` for implementation examples
4. Mark sensitive parameters with `secret: true` in YAML

### Future Enhancements
- Consider creating a bash library for common parameter parsing patterns
- Add parameter validation helpers
- Create action templates for different languages (bash, Python, Node.js)

## Impact

- ✅ **Security**: Eliminated parameter exposure via environment variables
- ✅ **Consistency**: All core pack actions use the same parameter delivery method
- ✅ **Documentation**: Clear guidelines for pack developers
- ✅ **Testing**: All actions verified with manual tests
- ✅ **Standards**: Established best practices for the platform

## Post-Migration Updates

**Date:** 2026-02-07 (same day)

### Echo Action Simplification

Removed the `uppercase` parameter from `echo.sh` action and made it purely pass-through:
- **Rationale:** Any formatting should be done before parameters reach the action script
- **Change 1:** Removed uppercase conversion logic and parameter from YAML
- **Change 2:** Message parameter is optional - outputs empty string if not provided
- **Impact:** Simplified action to pure pass-through output (echo only), no transformations

**Files updated:**
- `attune/packs/core/actions/echo.sh` - Removed uppercase conversion logic, simplified to output message or empty string
- `attune/packs/core/actions/echo.yaml` - Removed `uppercase` parameter definition, made `message` optional with no default

The echo action now accepts an optional `message` parameter and outputs it as-is. If no message is provided, it outputs an empty string (empty line). Any text transformations (uppercase, lowercase, formatting) should be handled upstream by the caller or workflow engine.

### Output Format Standardization

Added `output_format` field and corrected output schemas across all actions:
- **Rationale:** Clarify how action output should be parsed and stored by the worker
- **Change:** Added `output_format` field (text/json/yaml) to all action YAMLs
- **Change:** Removed execution metadata (stdout/stderr/exit_code) from output schemas
- **Impact:** Output schemas now describe actual data structure, not execution metadata

**Text format actions (no structured parsing):**
- `echo.sh` - Outputs plain text, no schema needed
- `sleep.sh` - Outputs plain text, no schema needed
- `noop.sh` - Outputs plain text, no schema needed

**JSON format actions (structured parsing enabled):**
- `http_request.sh` - Outputs JSON, schema describes response structure (curl-based)
- `download_packs.sh` - Outputs JSON, schema describes download results
- `build_pack_envs.sh` - Outputs JSON, schema describes environment build results
- `register_packs.sh` - Outputs JSON, schema describes registration results
- `get_pack_dependencies.sh` - Outputs JSON, schema describes dependency analysis

**Key principles:** 
- The worker automatically captures stdout/stderr/exit_code/duration_ms for every execution. These are execution metadata, not action output, and should never appear in output schemas.
- Actions should not include generic "status" or "result" wrapper fields in their output schemas unless those fields have domain-specific meaning (e.g., HTTP status_code, test result status).
- Output schemas should describe the actual data structure the action produces, not add layers of abstraction.

### HTTP Request Migration to Bash/Curl

Migrated `http_request` from Python to bash/curl to eliminate runtime dependencies:
- **Rationale:** Core pack should have zero runtime dependencies beyond standard utilities
- **Change:** Rewrote action as bash script using `curl` instead of Python `requests` library
- **Impact:** No Python runtime required, faster startup, simpler deployment

**Migration details:**
- Replaced `http_request.py` with `http_request.sh`
- All functionality preserved: methods, headers, auth (basic/bearer), JSON bodies, query params, timeouts
- Error handling includes curl exit code translation to user-friendly messages
- Response parsing handles JSON detection, header extraction, and status code validation
- Output format remains identical (JSON with status_code, headers, body, json, elapsed_ms, url, success)

**Dependencies:**
- `curl` - HTTP client (standard utility)
- `jq` - JSON processing (already required for parameter parsing)

**Testing verified:**
- GET/POST requests with JSON bodies
- Custom headers and authentication
- Query parameters
- Timeout handling
- Non-2xx status codes
- Error scenarios

## New Documentation Created

1. **`attune/docs/QUICKREF-action-output-format.md`** - Comprehensive guide to output formats and schemas:
   - Output format field (text/json/yaml)
   - Output schema patterns and best practices
   - Worker parsing behavior
   - Execution metadata handling
   - Migration examples
   - Common pitfalls and solutions

### Standard Environment Variables

Added documentation for standard `ATTUNE_*` environment variables provided by worker to all executions:
- **Purpose:** Provide execution context and enable API interaction
- **Variables:**
  - `ATTUNE_ACTION` - Action ref (always present)
  - `ATTUNE_EXEC_ID` - Execution database ID (always present)
  - `ATTUNE_API_TOKEN` - Execution-scoped API token (always present)
  - `ATTUNE_RULE` - Rule ref (if triggered by rule)
  - `ATTUNE_TRIGGER` - Trigger ref (if triggered by event)

**Use cases:**
- Logging with execution context
- Calling Attune API with scoped token
- Conditional behavior based on rule/trigger
- Creating child executions
- Accessing secrets from key vault

**Documentation created:**
- `attune/docs/QUICKREF-execution-environment.md` - Comprehensive guide covering all standard environment variables, usage patterns, security considerations, and examples

**Key distinction:** Environment variables provide execution context (system-provided), while action parameters provide user data (stdin-delivered). Never mix the two.

## Conclusion

This migration establishes a secure-by-design foundation for action input/output handling across the Attune platform:

1. **Input (parameters):** Always via stdin as JSON - never environment variables
2. **Output (format):** Explicitly declared as text, json, or yaml
3. **Output (schema):** Describes structured data shape, not execution metadata
4. **Execution metadata:** Automatically captured by worker (stdout/stderr/exit_code/duration_ms)
5. **Execution context:** Standard `ATTUNE_*` environment variables provide execution identity and API access

All core pack actions now follow these best practices, providing a reference implementation for future pack development. The patterns established here ensure:
- **Security:** No parameter exposure via process listings, scoped API tokens for each execution
- **Clarity:** Explicit output format declarations, clear separation of parameters vs environment
- **Separation of concerns:** Action output vs execution metadata, user data vs system context
- **Consistency:** Uniform patterns across all actions
- **Zero dependencies:** No Python, Node.js, or runtime dependencies required for core pack
- **API access:** Actions can interact with Attune API using execution-scoped tokens