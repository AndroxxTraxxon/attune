# DOTENV Parameter Flattening Fix

**Date**: 2026-02-09
**Status**: Complete
**Impact**: Bug Fix - Critical

## Problem

The `core.http_request` action was failing when executed, even though the HTTP request succeeded (returned 200 status). Investigation revealed that the action was receiving incorrect parameter values - specifically, the `url` parameter received `"200"` instead of the actual URL like `"https://example.com"`.

### Root Cause

The issue was in how nested JSON objects were being converted to DOTENV format for stdin parameter delivery:

1. The action YAML specified `parameter_format: dotenv` for shell-friendly parameter passing
2. When execution parameters contained nested objects (like `headers: {}`, `query_params: {}`), the `format_dotenv()` function was serializing them as JSON strings
3. The shell script expected flattened dotted notation (e.g., `headers.Content-Type=application/json`)
4. This mismatch caused parameter parsing to fail in the shell script

**Example of the bug:**
```json
// Input parameters
{
  "url": "https://example.com",
  "headers": {"Content-Type": "application/json"},
  "query_params": {"page": "1"}
}
```

**Incorrect output (before fix):**
```bash
url='https://example.com'
headers='{"Content-Type":"application/json"}'
query_params='{"page":"1"}'
```

The shell script couldn't parse `headers='{...}'` and expected:
```bash
headers.Content-Type='application/json'
query_params.page='1'
```

## Solution

Modified `crates/worker/src/runtime/parameter_passing.rs` to flatten nested JSON objects before formatting as DOTENV:

### Key Changes

1. **Added `flatten_parameters()` function**: Recursively flattens nested objects using dot notation
2. **Modified `format_dotenv()`**: Now calls `flatten_parameters()` before formatting
3. **Empty object handling**: Empty objects (`{}`) are omitted entirely from output
4. **Array handling**: Arrays are still serialized as JSON strings (expected behavior)
5. **Sorted output**: Lines are sorted alphabetically for consistency

### Implementation Details

```rust
fn flatten_parameters(
    params: &HashMap<String, JsonValue>,
    prefix: &str,
) -> HashMap<String, String> {
    let mut flattened = HashMap::new();

    for (key, value) in params {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            JsonValue::Object(map) => {
                // Recursively flatten nested objects
                let nested = /* ... */;
                flattened.extend(nested);
            }
            // ... handle other types
        }
    }

    flattened
}
```

**Correct output (after fix):**
```bash
headers.Content-Type='application/json'
query_params.page='1'
url='https://example.com'
```

## Testing

### Unit Tests Added

1. `test_format_dotenv_nested_objects`: Verifies nested object flattening
2. `test_format_dotenv_empty_objects`: Verifies empty objects are omitted

All tests pass:
```
running 9 tests
test runtime::parameter_passing::tests::test_format_dotenv ... ok
test runtime::parameter_passing::tests::test_format_dotenv_empty_objects ... ok
test runtime::parameter_passing::tests::test_format_dotenv_escaping ... ok
test runtime::parameter_passing::tests::test_format_dotenv_nested_objects ... ok
test runtime::parameter_passing::tests::test_format_json ... ok
test runtime::parameter_passing::tests::test_format_yaml ... ok
test runtime::parameter_passing::tests::test_create_parameter_file ... ok
test runtime::parameter_passing::tests::test_prepare_parameters_stdin ... ok
test runtime::parameter_passing::tests::test_prepare_parameters_file ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured
```

### Code Cleanup

- Removed unused `value_to_string()` function
- Removed unused `OutputFormat` import from `local.rs`
- Zero compiler warnings after fix

## Files Modified

1. `crates/worker/src/runtime/parameter_passing.rs`
   - Added `flatten_parameters()` function
   - Modified `format_dotenv()` to use flattening
   - Removed unused `value_to_string()` function
   - Added unit tests

2. `crates/worker/src/runtime/local.rs`
   - Removed unused `OutputFormat` import

## Documentation Created

1. `docs/parameters/dotenv-parameter-format.md` - Comprehensive guide covering:
   - DOTENV format specification
   - Nested object flattening rules
   - Shell script parsing examples
   - Security considerations
   - Troubleshooting guide
   - Best practices

## Deployment

1. Rebuilt worker-shell Docker image with fix
2. Restarted worker-shell service
3. Fix is now live and ready for testing

## Impact

### Before Fix
- `core.http_request` action: **FAILED** with incorrect parameters
- Any action using `parameter_format: dotenv` with nested objects: **BROKEN**

### After Fix
- `core.http_request` action: Should work correctly with nested headers/query_params
- All dotenv-format actions: Properly receive flattened nested parameters
- Shell scripts: Can parse parameters without external dependencies (no `jq` needed)

## Verification Steps

To verify the fix works:

1. Execute `core.http_request` with nested parameters:
```bash
attune action execute core.http_request \
  --param url=https://example.com \
  --param method=GET \
  --param 'headers={"Content-Type":"application/json"}' \
  --param 'query_params={"page":"1"}'
```

2. Check execution logs - should see flattened parameters in stdin:
```
headers.Content-Type='application/json'
query_params.page='1'
url='https://example.com'
---ATTUNE_PARAMS_END---
```

3. Verify execution succeeds with correct HTTP request/response

## Related Issues

This fix resolves parameter passing for all shell actions using:
- `parameter_delivery: stdin`
- `parameter_format: dotenv`
- Nested object parameters

## Notes

- DOTENV format is recommended for shell actions due to security (no process list exposure) and simplicity (no external dependencies)
- JSON and YAML formats still work as before (no changes needed)
- This is a backward-compatible fix - existing actions continue to work
- The `core.http_request` action specifically benefits as it uses nested `headers` and `query_params` objects

## Next Steps

1. Test `core.http_request` action with various parameter combinations
2. Update any other core pack actions to use `parameter_format: dotenv` where appropriate
3. Consider adding integration tests for parameter passing formats