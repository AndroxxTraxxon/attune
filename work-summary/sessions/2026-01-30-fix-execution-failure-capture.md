# Fix: Execution Failure Detection and Error Capture

**Date:** 2026-01-30  
**Issue:** Executions occasionally fail with "Execution failed during preparation" error even though stdout.log shows the action ran successfully  
**Status:** Fixed

---

## Problem Description

Users reported occasional execution failures with the following characteristics:
- Error message: `"Execution failed during preparation"`
- Result JSON shows `"succeeded": false`
- The `stdout.log` file exists and contains output from the action
- The action appears to have run, but the system failed to capture the success

### Example Error
```json
{
  "error": "Execution failed during preparation",
  "stdout_log": "/tmp/attune/artifacts/execution_10172/stdout.log",
  "succeeded": false
}
```

---

## Root Cause Analysis

The issue was identified in the worker's execution flow, specifically in how runtime errors are handled:

### 1. **Process Wait Failures**
In `shell.rs` (`execute_with_streaming` method), if `child.wait()` fails after the process has already started and written output:
```rust
Ok(Err(e)) => {
    return Err(RuntimeError::ProcessError(format!(
        "Process wait failed: {}",
        e
    )));
}
```

This returns an `Err` even though:
- The child process ran successfully
- Output was captured to stdout/stderr
- The process may have completed normally

### 2. **Stdin Write Failures**
Writing secrets to stdin could fail after the process spawned:
```rust
let secrets_json = serde_json::to_string(secrets)?;
stdin.write_all(secrets_json.as_bytes()).await?;
```

The `?` operator would propagate the error up, discarding captured output.

### 3. **Error Propagation in Executor**
In `executor.rs`, when `execute_action()` returns an `Err`:
```rust
let result = match self.execute_action(context).await {
    Ok(result) => result,
    Err(e) => {
        error!("Action execution failed: {}", e);
        self.handle_execution_failure(execution_id, None).await?;  // None = no result
        return Err(e);
    }
};
```

Passing `None` to `handle_execution_failure` triggers the "Execution failed during preparation" message, even though logs exist.

### 4. **Poor Error Messages**
When exit code was non-zero, the entire stderr was used as the error message, which could be very long and unhelpful.

---

## Solution Implemented

### Changes to `shell.rs`

#### 1. **Graceful Stdin Write Handling**
```rust
let stdin_write_error = if let Some(mut stdin) = child.stdin.take() {
    match serde_json::to_string(secrets) {
        Ok(secrets_json) => {
            if let Err(e) = stdin.write_all(secrets_json.as_bytes()).await {
                Some(format!("Failed to write secrets to stdin: {}", e))
            } else if let Err(e) = stdin.write_all(b"\n").await {
                Some(format!("Failed to write newline to stdin: {}", e))
            } else {
                drop(stdin);
                None
            }
        }
        Err(e) => Some(format!("Failed to serialize secrets: {}", e)),
    }
} else {
    None
};
```

- Capture stdin write errors instead of propagating them
- Continue execution to capture output
- Include error in ExecutionResult

#### 2. **Process Wait Error Recovery**
```rust
let (exit_code, process_error) = match wait_result {
    Ok(Ok(status)) => (status.code().unwrap_or(-1), None),
    Ok(Err(e)) => {
        // Process wait failed, but we have the output - return it with an error
        warn!("Process wait failed but captured output: {}", e);
        (-1, Some(format!("Process wait failed: {}", e)))
    }
    Err(_) => {
        // Timeout occurred - return captured output
        return Ok(ExecutionResult {
            exit_code: -1,
            stdout: stdout_result.content.clone(),
            stderr: stderr_result.content.clone(),
            // ... include truncation info
        });
    }
};
```

- Always return `Ok(ExecutionResult)` when we have captured output
- Include process wait errors in the result's `error` field
- Preserve stdout/stderr even on timeout

#### 3. **Improved Error Messages**
```rust
let error = if let Some(proc_err) = process_error {
    Some(proc_err)
} else if let Some(stdin_err) = stdin_write_error {
    Some(stdin_err)
} else if exit_code != 0 {
    Some(if stderr_result.content.is_empty() {
        format!("Command exited with code {}", exit_code)
    } else {
        // Use last line of stderr as error, or full stderr if short
        if stderr_result.content.lines().count() > 5 {
            stderr_result.content.lines().last().unwrap_or("").to_string()
        } else {
            stderr_result.content.clone()
        }
    })
} else {
    None
};
```

- Prioritize specific error sources
- Use last line of stderr for concise error messages
- Full stderr only if short (≤5 lines)

### Changes to `executor.rs`

#### 1. **Better Documentation**
```rust
// Note: execute_action should rarely return Err - most failures should be
// captured in ExecutionResult with non-zero exit codes
let result = match self.execute_action(context).await {
    Ok(result) => result,
    Err(e) => {
        error!("Action execution failed catastrophically: {}", e);
        // This should only happen for unrecoverable errors like runtime not found
```

Clarified that returning `Err` should be rare.

#### 2. **Enhanced Failure Handling**
When `result` is `None` (early failure), now attempts to read logs from disk:

```rust
// Check if stdout log exists from artifact storage
let stdout_path = exec_dir.join("stdout.log");
if stdout_path.exists() {
    result_data["stdout_log"] = serde_json::json!(stdout_path.to_string_lossy());
    // Try to read a preview if file exists
    if let Ok(contents) = tokio::fs::read_to_string(&stdout_path).await {
        let preview = if contents.len() > 1000 {
            format!("{}...", &contents[..1000])
        } else {
            contents
        };
        result_data["stdout"] = serde_json::json!(preview);
    }
}
```

This provides better diagnostics even for catastrophic failures.

#### 3. **Truncation Metadata**
Added truncation information to failure results:
```rust
if exec_result.stdout_truncated {
    result_data["stdout_truncated"] = serde_json::json!(true);
    result_data["stdout_bytes_truncated"] = 
        serde_json::json!(exec_result.stdout_bytes_truncated);
}
```

---

## Impact

### Before
- **Intermittent "preparation" failures** even when actions ran successfully
- **Lost output** from partially-completed executions
- **Verbose error messages** (entire stderr dump)
- **Difficult debugging** due to missing context

### After
- **Always capture output** when process runs, regardless of wait() status
- **Specific error messages** identifying the actual failure point
- **Concise error summaries** (last line of stderr)
- **Better diagnostics** with truncation metadata
- **Graceful degradation** for stdin write failures

---

## Testing Recommendations

1. **Process Termination Scenarios**
   - Actions that crash or are killed
   - Zombie processes
   - Process that exit before we can wait()

2. **Resource Exhaustion**
   - Very large stdout/stderr (test truncation)
   - Many concurrent executions
   - Slow process cleanup

3. **Stdin Write Failures**
   - Processes that close stdin immediately
   - Broken pipe scenarios
   - Large secret payloads

4. **Edge Cases**
   - Timeout with partial output
   - Exit code 0 but stderr present
   - No output but successful exit

---

## Files Modified

- `attune/crates/worker/src/runtime/shell.rs` - Improved error handling and output capture
- `attune/crates/worker/src/executor.rs` - Enhanced failure diagnostics

---

## Notes

- This fix makes the system more resilient to transient process management issues
- The "Execution failed during preparation" error should now be extremely rare
- When it does occur, the result will include any available logs
- Error messages are now more actionable and concise
- All changes are backward compatible - existing executions unaffected

---

## Related Documentation

- `attune/docs/worker-service.md` - Worker architecture
- `attune/docs/running-tests.md` - Testing guidelines