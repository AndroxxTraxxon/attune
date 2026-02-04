# Worker Runtime Configuration Fix - February 2026

## Problem

The `attune-worker-node` container was attempting to validate the Python runtime installation, even though it was only configured to support Node.js and Shell runtimes. This caused the worker to crash on startup with the error:

```
Error: Internal error: Failed to validate runtimes: Setup error: Python validation failed: No such file or directory (os error 2)
```

The issue affected all specialized worker containers that were configured for specific runtimes via the `ATTUNE_WORKER_RUNTIMES` environment variable.

## Root Causes

1. **Hardcoded Runtime Registration**: The worker service was hardcoding which runtimes to register in the `RuntimeRegistry`, always registering both Python and Shell regardless of configuration:

```rust
// OLD CODE - Always registered Python
let python_runtime = PythonRuntime::with_dependency_manager(...);
runtime_registry.register(Box::new(python_runtime));
runtime_registry.register(Box::new(ShellRuntime::new()));
```

2. **Unconditional Dependency Manager Setup**: The Python virtual environment manager was initialized unconditionally, even when Python wasn't needed.

3. **LocalRuntime Fallback Issue**: The `LocalRuntime` was always registered as a fallback, but it internally creates and validates Python, Shell, and Native runtimes, causing validation failures when Python wasn't installed.

## Solution

Implemented dynamic runtime registration based on the `ATTUNE_WORKER_RUNTIMES` environment variable:

### Changes Made

1. **Dynamic Runtime Registration** (`crates/worker/src/service.rs`)
   - Read `ATTUNE_WORKER_RUNTIMES` environment variable (e.g., "shell,node")
   - Parse into list of runtime names
   - Only register the runtimes that are configured
   - Log warnings for unimplemented runtimes (e.g., Node.js)

2. **Conditional Dependency Manager Setup**
   - Only initialize Python virtual environment manager when Python runtime is in the configured list
   - Reduces startup overhead for workers that don't need Python

3. **Smart LocalRuntime Registration**
   - Only register `LocalRuntime` as fallback when no specific runtimes are configured
   - Prevents validation failures from LocalRuntime's embedded Python/Shell/Native runtimes

4. **Added Runtime Imports**
   - Added `NativeRuntime` import for when native runtime is explicitly configured

### Code Structure

```rust
// Determine configured runtimes
let configured_runtimes = if let Ok(runtimes_env) = std::env::var("ATTUNE_WORKER_RUNTIMES") {
    runtimes_env.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
} else {
    // Fallback to defaults if not configured
    vec!["shell".to_string(), "python".to_string(), "native".to_string()]
};

// Register only configured runtimes
for runtime_name in &configured_runtimes {
    match runtime_name.as_str() {
        "python" => { /* Register Python */ }
        "shell" => { /* Register Shell */ }
        "native" => { /* Register Native */ }
        "node" => { warn!("Node.js runtime not yet implemented"); }
        _ => { warn!("Unknown runtime type '{}'", runtime_name); }
    }
}
```

## Testing Results

### Worker-Node Container (shell,node)
```
✅ Reads ATTUNE_WORKER_RUNTIMES=shell,node
✅ Registers only Shell runtime (Node.js logs warning)
✅ Skips Python dependency manager initialization
✅ Validates only Shell runtime
✅ Starts successfully without Python validation errors
✅ Worker registered with ID: 3
```

### Worker-Python Container (shell,python)
```
✅ Reads ATTUNE_WORKER_RUNTIMES=shell,python
✅ Initializes Python dependency manager
✅ Registers both Shell and Python runtimes
✅ Validates both runtimes successfully
✅ Starts successfully
```

## Configuration

Workers advertise their runtime capabilities to the database via the `worker.capabilities` JSONB field:

```json
{
  "runtimes": ["shell", "node"],
  "max_concurrent_executions": 10,
  "worker_version": "0.1.0"
}
```

This information is used by the executor service to route executions to appropriate workers.

## Docker Compose Configuration

Example worker configurations:

```yaml
worker-node:
  environment:
    ATTUNE_WORKER_RUNTIMES: shell,node
    ATTUNE_WORKER_NAME: worker-node-01

worker-python:
  environment:
    ATTUNE_WORKER_RUNTIMES: shell,python
    ATTUNE_WORKER_NAME: worker-python-01

worker-full:
  environment:
    ATTUNE_WORKER_RUNTIMES: shell,python,node,native
    ATTUNE_WORKER_NAME: worker-full-01
```

## Architecture

**Before:**
- All workers registered all runtimes regardless of container configuration
- Python validation failed on containers without Python installed
- Wasted resources initializing unused dependency managers

**After:**
- Workers register only the runtimes they're configured to support
- Runtime validation only runs for registered runtimes
- Dependency managers (e.g., Python venv) only initialized when needed
- Proper separation of concerns between specialized worker containers

## Files Modified

- `crates/worker/src/service.rs` (115 lines changed)
  - Added dynamic runtime registration logic
  - Conditional Python dependency manager setup
  - Smart LocalRuntime fallback registration
  - Added NativeRuntime import

## Benefits

1. **Correct Behavior**: Workers only validate runtimes they're supposed to support
2. **Faster Startup**: No unnecessary dependency manager initialization
3. **Container Specialization**: Workers can be optimized for specific runtime types
4. **Clear Configuration**: Runtime support explicitly declared via environment variable
5. **Better Logging**: Clear messages about which runtimes are registered and validated

## Future Enhancements

1. **Node.js Runtime Implementation**: Complete the Node.js runtime implementation (currently logs warning)
2. **Runtime Hot-Reload**: Allow adding/removing runtimes without worker restart
3. **Runtime Health Checks**: Periodic validation of runtime availability
4. **Per-Runtime Configuration**: Allow runtime-specific configuration (e.g., Python version, Node.js flags)

## Related Components

- **WorkerRegistration** (`crates/worker/src/registration.rs`): Already reads `ATTUNE_WORKER_RUNTIMES` for capability advertisement
- **Executor Service**: Uses worker capabilities to route executions
- **Runtime Registry** (`crates/worker/src/runtime/mod.rs`): Manages runtime instances and validation

## Deployment Notes

- Existing deployments will continue to work (fallback to default runtimes)
- No database migrations required
- Existing worker registrations will update capabilities on next heartbeat
- Requires rebuild of Docker images to pick up changes