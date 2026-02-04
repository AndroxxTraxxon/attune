# Work Summary: Native Runtime Support Implementation

**Date**: January 29, 2025  
**Author**: AI Assistant  
**Status**: Complete

## Overview

Implemented native runtime support for Attune, enabling the execution of compiled binaries (Rust, Go, C/C++, etc.) directly without requiring language interpreters or shell wrappers. This is a critical feature for running the timer sensor (`attune-core-timer-sensor`) and future high-performance actions.

## Changes Made

### 1. Database Runtime Entries

**File**: `attune/scripts/seed_runtimes.sql`

- Added `core.action.native` runtime entry for native action execution
- Added `core.sensor.native` runtime entry for native sensor execution
- Both entries use `runtime_type_enum` values ('action' and 'sensor')
- Installation method set to "binary" (no runtime installation required)

Runtime entries inserted:
```sql
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
```

### 2. Native Runtime Implementation

**New File**: `attune/crates/worker/src/runtime/native.rs`

Created a complete `NativeRuntime` implementation with:

- **Binary execution**: Spawns native executables directly using `tokio::process::Command`
- **Parameter passing**: Converts parameters to environment variables with `ATTUNE_ACTION_` prefix
- **Secret handling**: Passes secrets via stdin as JSON
- **Output capture**: Bounded stdout/stderr capture with size limits and truncation notices
- **Timeout support**: Kills process if execution exceeds timeout
- **Permission checks**: Validates binary is executable (Unix systems)
- **Concurrent I/O**: Reads stdout and stderr concurrently without deadlock
- **Error handling**: Comprehensive error types and validation

Key features:
- Implements `Runtime` trait for seamless integration
- Supports `can_execute()` heuristics (checks for script extensions)
- Prefers explicit `runtime_name = "native"` for deterministic selection
- Validates binary exists and has execute permissions before running

### 3. Integration with LocalRuntime

**File**: `attune/crates/worker/src/runtime/local.rs`

- Added `NativeRuntime` to the composite `LocalRuntime`
- Updated runtime selection logic to check native runtime first
- Added native runtime to setup/cleanup/validate lifecycle methods
- Fixed signature of `with_runtimes()` to include `NativeRuntime` parameter

Selection priority: Native → Python → Shell

### 4. Module Exports

**File**: `attune/crates/worker/src/runtime/mod.rs`
- Added `pub mod native` declaration
- Exported `NativeRuntime` type

**File**: `attune/crates/worker/src/lib.rs`
- Added `NativeRuntime` to public re-exports

### 5. Worker Capabilities

**File**: `attune/crates/worker/src/registration.rs`

Updated worker capabilities to advertise native runtime support:
```json
{
  "runtimes": ["native", "python", "shell", "node"]
}
```

### 6. Timer Sensor Configuration

**File**: `attune/packs/core/sensors/interval_timer_sensor.yaml`

- Changed `runner_type` from `python` to `native`
- Updated `entry_point` from `interval_timer_sensor.py` to `attune-core-timer-sensor`
- Sensor now references the compiled Rust binary

### 7. Binary Deployment

- Built release binary: `cargo build --package attune-core-timer-sensor --release`
- Copied to pack directory: `target/release/attune-core-timer-sensor` → `packs/core/sensors/attune-core-timer-sensor`
- Binary is now ready for native execution by the worker

### 8. Documentation

**New File**: `attune/docs/native-runtime.md`

Comprehensive documentation covering:
- Runtime configuration and database entries
- Action and sensor YAML definitions for native binaries
- Binary requirements (parameters, secrets, output format)
- Runtime selection logic
- Execution details (env vars, stdin, stdout/stderr, timeouts)
- Building native binaries (Rust and Go examples)
- Advantages and limitations
- Best practices and troubleshooting

## Technical Details

### Runtime Selection Logic

The worker selects native runtime when:
1. Execution context explicitly sets `runtime_name: "native"`, OR
2. The binary path has no common script extension (.py, .js, .sh, etc.) and file exists

### Parameter Passing

Parameters are converted to environment variables:
- Format: `ATTUNE_ACTION_{PARAM_NAME_UPPERCASE}`
- Example: `input_data` → `ATTUNE_ACTION_INPUT_DATA`
- Complex types serialized to JSON strings

### Output Handling

- Stdout and stderr captured with configurable size limits (default 10MB each)
- Truncation notices added when limits exceeded
- Exit code 0 = success, non-zero = failure
- JSON parsing attempted on stdout for result extraction

### Process Management

- Async execution using `tokio::process::Command`
- Concurrent stdout/stderr reading prevents deadlocks
- Timeout enforcement with process kill (SIGKILL)
- Graceful handling of process exit codes

## Testing

### Compilation Verification

```bash
cargo check --package attune-worker  # ✓ Success
cargo build --package attune-worker  # ✓ Success
cargo build --package attune-core-timer-sensor --release  # ✓ Success
```

### Binary Verification

```bash
./target/release/attune-core-timer-sensor --help  # ✓ Displays usage
ls -la packs/core/sensors/attune-core-timer-sensor  # ✓ Executable present
```

### Database Seeding

```bash
# Native runtime entries inserted successfully
SELECT ref, name, runtime_type FROM runtime WHERE ref LIKE '%native%';
```

Result:
```
        ref         |         name          | runtime_type
--------------------+-----------------------+--------------
 core.action.native | Native Action Runtime | action
 core.sensor.native | Native Sensor Runtime | sensor
```

## Impact

### Immediate Benefits

1. **Timer Sensor**: Can now run as compiled Rust binary instead of Python script
   - Better performance (no interpreter overhead)
   - Lower memory footprint
   - No Python runtime dependencies

2. **Future Actions**: Enables high-performance actions in Rust/Go/C++
   - Compute-intensive operations
   - System integration tools
   - Performance-critical workflows

3. **Security**: Compiled binaries reduce script injection attack surface

### Architecture

- Clean separation of concerns (NativeRuntime is standalone)
- Follows existing Runtime trait pattern
- Integrates seamlessly with worker's runtime registry
- No breaking changes to existing Python/Shell runtimes

## Breaking Changes

**None** - This is a purely additive feature. Existing actions and sensors continue to work unchanged.

## Future Enhancements

Potential improvements identified but not implemented:

1. **Cross-compilation support**: Tooling to build binaries for multiple platforms
2. **Binary versioning**: Track binary versions in database
3. **Health checks**: Validate binary compatibility before execution
4. **Metrics collection**: Native runtime execution statistics
5. **Sandboxing**: Container or seccomp isolation for native binaries

## Files Modified

- `attune/scripts/seed_runtimes.sql` - Added native runtime entries
- `attune/crates/worker/src/runtime/mod.rs` - Added native module
- `attune/crates/worker/src/runtime/local.rs` - Integrated NativeRuntime
- `attune/crates/worker/src/lib.rs` - Exported NativeRuntime
- `attune/crates/worker/src/registration.rs` - Updated capabilities
- `attune/packs/core/sensors/interval_timer_sensor.yaml` - Changed to native runtime

## Files Created

- `attune/crates/worker/src/runtime/native.rs` - NativeRuntime implementation (438 lines)
- `attune/docs/native-runtime.md` - Comprehensive documentation (334 lines)
- `attune/packs/core/sensors/attune-core-timer-sensor` - Compiled binary (6.6 MB)

## Verification Steps

To verify the implementation:

1. **Check runtime entries exist**:
   ```bash
   psql -d attune -c "SELECT * FROM runtime WHERE ref LIKE '%native%';"
   ```

2. **Verify worker advertises native runtime**:
   ```bash
   # Start worker and check registration
   cargo run --package attune-worker
   # Check worker.capabilities in database
   ```

3. **Test timer sensor binary**:
   ```bash
   ./packs/core/sensors/attune-core-timer-sensor --help
   ```

4. **Create and execute a native action** (future test):
   - Define action with `runner_type: native`
   - Place compiled binary in pack directory
   - Execute via API and verify output

## Conclusion

Native runtime support is now fully implemented and ready for use. The timer sensor can be deployed as a native binary, and future actions can leverage compiled languages for performance-critical operations. The implementation follows Attune's architecture patterns and requires no changes to existing functionality.

## Related Documentation

- [Native Runtime Documentation](../docs/native-runtime.md)
- [Worker Service Architecture](../docs/worker-service.md)
- [Timer Sensor README](../crates/sensor-timer/README.md)
- [Runtime Selection Logic](../crates/worker/src/runtime/mod.rs)
