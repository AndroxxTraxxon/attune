# Secret Passing Fix - Implementation Plan

**Date:** 2025-01-XX  
**Priority:** P0 - BLOCKING (Security Critical)  
**Estimated Time:** 3-5 days  
**Status:** 🔄 IN PROGRESS

## Problem Statement

**Current Implementation:**
- Secrets are passed to actions via environment variables (using `prepare_secret_env()`)
- Environment variables are visible in `/proc/[pid]/environ` and `ps` output
- This is a **critical security vulnerability** - any user on the system can read secrets

**Example of the vulnerability:**
```bash
# Current behavior - INSECURE
$ ps auxe | grep python
user  1234  ... SECRET_API_KEY=sk_live_abc123 SECRET_DB_PASSWORD=super_secret ...

$ cat /proc/1234/environ
SECRET_API_KEY=sk_live_abc123
SECRET_DB_PASSWORD=super_secret
```

## Solution Design

**New Approach:**
- Pass secrets via **stdin as JSON** instead of environment variables
- Secrets never appear in process table or environment
- Wrapper scripts read JSON from stdin before executing action code

**Security Benefits:**
1. ✅ Secrets not visible in `ps` output
2. ✅ Secrets not visible in `/proc/[pid]/environ`
3. ✅ Secrets not visible in process monitoring tools
4. ✅ Secrets only accessible to the running process itself

## Implementation Steps

### Phase 1: Update Data Structures (1-2 hours)

#### 1.1 Update `ExecutionContext` struct
**File:** `crates/worker/src/runtime/mod.rs`

```rust
pub struct ExecutionContext {
    pub execution_id: i64,
    pub action_ref: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub env: HashMap<String, String>,
    
    // NEW: Separate secrets field
    pub secrets: HashMap<String, String>,  // ← ADD THIS
    
    pub timeout: Option<u64>,
    pub working_dir: Option<PathBuf>,
    pub entry_point: String,
    pub code: Option<String>,
    pub code_path: Option<PathBuf>,
    pub runtime_name: Option<String>,
}
```

**Changes:**
- Add `secrets: HashMap<String, String>` field
- Secrets stored separately from `env`
- No more mixing secrets with environment variables

#### 1.2 Update `ActionExecutor::prepare_execution_context()`
**File:** `crates/worker/src/executor.rs` (lines 166-308)

**Current code (INSECURE):**
```rust
// Fetch and inject secrets
match self.secret_manager.fetch_secrets_for_action(action).await {
    Ok(secrets) => {
        let secret_env = self.secret_manager.prepare_secret_env(&secrets);
        env.extend(secret_env);  // ← INSECURE: adds to env vars
    }
    // ...
}
```

**New code (SECURE):**
```rust
// Fetch secrets (but don't add to env)
let secrets = match self.secret_manager.fetch_secrets_for_action(action).await {
    Ok(secrets) => {
        debug!("Fetched {} secrets for action", secrets.len());
        secrets
    }
    Err(e) => {
        warn!("Failed to fetch secrets: {}", e);
        HashMap::new()
    }
};

// Add secrets to context (not env)
let context = ExecutionContext {
    execution_id: execution.id,
    action_ref: execution.action_ref.clone(),
    parameters,
    env,
    secrets,  // ← NEW: separate field
    timeout,
    working_dir: None,
    entry_point,
    code,
    code_path: None,
    runtime_name,
};
```

### Phase 2: Update Python Runtime (2-3 hours)

#### 2.1 Update Python wrapper script generation
**File:** `crates/worker/src/runtime/python.rs` (function `generate_wrapper_script`)

**Current wrapper (simplified):**
```python
#!/usr/bin/env python3
import sys
import json

# Parameters exported as env vars
# Secrets exported as env vars (INSECURE)

# Execute action code
```

**New wrapper (SECURE):**
```python
#!/usr/bin/env python3
import sys
import json
import os

# Read secrets from stdin BEFORE executing action
secrets_json = sys.stdin.readline().strip()
if secrets_json:
    secrets = json.loads(secrets_json)
    # Store in process-local dict, NOT in os.environ
    _attune_secrets = secrets
else:
    _attune_secrets = {}

# Helper function for action code to access secrets
def get_secret(name):
    """Get a secret value by name"""
    return _attune_secrets.get(name)

# Parameters (exported as usual)
# ... rest of wrapper code ...

# Execute action code
```

**Key points:**
- Read JSON from stdin FIRST (before action runs)
- Store in Python dict `_attune_secrets`, NOT `os.environ`
- Provide `get_secret()` helper function for action code
- Stdin is consumed, so action can't read it again (one-time use)

#### 2.2 Update `PythonRuntime::execute_python_code()`
**File:** `crates/worker/src/runtime/python.rs`

**Add stdin injection:**
```rust
async fn execute_python_code(
    &self,
    script: String,
    secrets: &HashMap<String, String>,  // ← NEW parameter
    env: &std::collections::HashMap<String, String>,
    timeout_secs: Option<u64>,
) -> RuntimeResult<ExecutionResult> {
    // ... setup code ...
    
    let mut cmd = Command::new(&self.python_path);
    cmd.arg(&script_file)
        .stdin(Stdio::piped())  // ← Enable stdin
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Add environment variables
    for (key, value) in env {
        cmd.env(key, value);
    }
    
    // Spawn process
    let mut child = cmd.spawn()?;
    
    // Write secrets to stdin as JSON
    if let Some(mut stdin) = child.stdin.take() {
        let secrets_json = serde_json::to_string(&secrets)?;
        stdin.write_all(secrets_json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        drop(stdin);  // Close stdin
    }
    
    // Wait for output
    let output = child.wait_with_output().await?;
    
    // ... process results ...
}
```

#### 2.3 Update `PythonRuntime::execute()` signature
```rust
async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
    // Generate wrapper with secret access helper
    let script = self.generate_wrapper_script(&context)?;
    
    // Pass secrets separately
    self.execute_python_code(
        script,
        &context.secrets,  // ← NEW: pass secrets
        &context.env,
        context.timeout,
    ).await
}
```

### Phase 3: Update Shell Runtime (2-3 hours)

#### 3.1 Update Shell wrapper script generation
**File:** `crates/worker/src/runtime/shell.rs` (function `generate_wrapper_script`)

**New wrapper approach:**
```bash
#!/bin/bash
set -e

# Read secrets from stdin into associative array
read -r ATTUNE_SECRETS_JSON
declare -A ATTUNE_SECRETS

if [ -n "$ATTUNE_SECRETS_JSON" ]; then
    # Parse JSON secrets (requires jq or Python)
    # Option A: Use Python to parse JSON
    eval "$(echo "$ATTUNE_SECRETS_JSON" | python3 -c "
import sys, json
secrets = json.load(sys.stdin)
for key, value in secrets.items():
    safe_value = value.replace(\"'\", \"'\\\\''\")
    print(f\"ATTUNE_SECRETS['{key}']='{safe_value}'\")
")"
fi

# Helper function to get secrets
get_secret() {
    echo "${ATTUNE_SECRETS[$1]}"
}

# Export parameters as environment variables
# ... (existing parameter export code) ...

# Execute action code
# ... (existing action code) ...
```

**Alternative (simpler but requires temp file):**
```bash
#!/bin/bash
set -e

# Read secrets from stdin into temp file
SECRETS_FILE=$(mktemp)
trap "rm -f $SECRETS_FILE" EXIT
cat > "$SECRETS_FILE"

# Helper function to get secrets (reads from temp file)
get_secret() {
    local name="$1"
    python3 -c "import sys, json; secrets=json.load(open('$SECRETS_FILE')); print(secrets.get('$name', ''))"
}

# Export parameters
# ... rest of wrapper ...
```

#### 3.2 Update `ShellRuntime::execute_shell_code()`
Similar pattern to Python runtime - pipe secrets to stdin as JSON.

### Phase 4: Remove Deprecated Method (30 minutes)

#### 4.1 Deprecate `SecretManager::prepare_secret_env()`
**File:** `crates/worker/src/secrets.rs`

```rust
/// Prepare secrets as environment variables
///
/// **DEPRECATED**: This method is insecure as it exposes secrets in the process environment.
/// Secrets should be passed via stdin instead.
#[deprecated(
    since = "0.2.0",
    note = "Use direct secret passing via stdin instead of environment variables"
)]
pub fn prepare_secret_env(&self, secrets: &HashMap<String, String>) -> HashMap<String, String> {
    // ... existing implementation ...
}
```

**Action:** Mark as deprecated, plan to remove in future version.

### Phase 5: Security Testing (1-2 hours)

#### 5.1 Create security test suite
**File:** `crates/worker/tests/security_tests.rs` (NEW FILE)

```rust
/// Test that secrets are NOT visible in process environment
#[tokio::test]
async fn test_secrets_not_in_process_environ() {
    // Create action with secret
    let context = ExecutionContext {
        secrets: {
            let mut s = HashMap::new();
            s.insert("api_key".to_string(), "super_secret_key_123".to_string());
            s
        },
        // ... other fields ...
    };
    
    // Execute action that spawns child process
    // Child process should write its /proc/self/environ to stdout
    
    let result = runtime.execute(context).await.unwrap();
    
    // Verify secret is NOT in environ output
    assert!(!result.stdout.contains("super_secret_key_123"));
    assert!(!result.stdout.contains("SECRET_API_KEY"));
}

/// Test that secrets ARE accessible to action code
#[tokio::test]
async fn test_secrets_accessible_in_action() {
    let context = ExecutionContext {
        secrets: {
            let mut s = HashMap::new();
            s.insert("api_key".to_string(), "test_key_456".to_string());
            s
        },
        code: Some("print(get_secret('api_key'))".to_string()),
        // ... other fields ...
    };
    
    let result = runtime.execute(context).await.unwrap();
    
    // Verify secret IS accessible via get_secret()
    assert!(result.stdout.contains("test_key_456"));
}

/// Test that ps output doesn't show secrets
#[tokio::test]
async fn test_secrets_not_in_ps_output() {
    // This test spawns a long-running action
    // While it's running, capture ps output
    // Verify secrets don't appear
    
    // Implementation requires:
    // 1. Spawn action with sleep
    // 2. Run `ps auxe` while action is running
    // 3. Verify secret not in output
    // 4. Wait for action to complete
}
```

#### 5.2 Test action code patterns
**File:** `crates/worker/tests/secret_access_tests.rs` (NEW FILE)

Test that action code can access secrets via helper functions:

**Python:**
```python
api_key = get_secret('api_key')
print(f"Using key: {api_key}")
```

**Shell:**
```bash
api_key=$(get_secret 'api_key')
echo "Using key: $api_key"
```

### Phase 6: Documentation (1-2 hours)

#### 6.1 Update action development guide
**File:** `docs/action-development.md` (NEW or UPDATE)

```markdown
## Accessing Secrets in Actions

### Python Actions

Secrets are available via the `get_secret()` function:

```python
def run(params):
    api_key = get_secret('api_key')
    db_password = get_secret('db_password')
    
    # Use secrets...
    return {"status": "success"}
```

**Important:** Do NOT access secrets via `os.environ` - they are not stored there
for security reasons.

### Shell Actions

Secrets are available via the `get_secret` function:

```bash
#!/bin/bash

api_key=$(get_secret 'api_key')
db_password=$(get_secret 'db_password')

# Use secrets...
echo "Connected successfully"
```

**Security Note:** Secrets are passed securely via stdin and never appear in
process listings or environment variables.
```

#### 6.2 Update security documentation
**File:** `docs/security.md` (UPDATE)

Document the security improvements and rationale.

### Phase 7: Migration Guide (1 hour)

#### 7.1 Create migration guide for existing packs
**File:** `docs/migrations/secret-access-migration.md` (NEW)

```markdown
# Migrating to Secure Secret Access

## What Changed

As of version 0.2.0, secrets are no longer passed via environment variables.
This improves security by preventing secrets from appearing in process listings.

## Migration Steps

### Before (Insecure)
```python
import os
api_key = os.environ.get('SECRET_API_KEY')
```

### After (Secure)
```python
api_key = get_secret('api_key')
```

### Backward Compatibility

For a transitional period, you can support both methods:

```python
api_key = get_secret('api_key') or os.environ.get('SECRET_API_KEY')
```

However, we recommend migrating fully to `get_secret()`.
```

## Testing Checklist

- [ ] Unit tests pass for ExecutionContext with secrets field
- [ ] Python runtime injects secrets via stdin
- [ ] Shell runtime injects secrets via stdin
- [ ] Actions can access secrets via `get_secret()`
- [ ] Secrets NOT in `/proc/[pid]/environ`
- [ ] Secrets NOT in `ps auxe` output
- [ ] Existing actions continue to work (backward compat)
- [ ] Documentation updated
- [ ] Migration guide created

## Success Criteria

1. ✅ All secrets passed via stdin (not environment)
2. ✅ Security tests confirm secrets not visible externally
3. ✅ Action code can still access secrets easily
4. ✅ No breaking changes for users (helper functions added)
5. ✅ Documentation complete
6. ✅ All tests passing

## Timeline

- **Day 1:** Phase 1-2 (Data structures + Python runtime)
- **Day 2:** Phase 3-4 (Shell runtime + deprecation)
- **Day 3:** Phase 5-7 (Testing + documentation)
- **Day 4-5:** Buffer for edge cases and refinement

## Risks & Mitigation

**Risk:** Breaking existing actions that access `os.environ['SECRET_*']`
**Mitigation:** Provide backward compatibility period and clear migration guide

**Risk:** Stdin approach may not work for all action types
**Mitigation:** Test with various action patterns, provide alternative temp file approach if needed

**Risk:** JSON parsing in shell may be fragile
**Mitigation:** Use Python for JSON parsing in shell wrapper (Python always available)

## Next Steps After Completion

1. Announce change to users
2. Provide migration examples
3. Set deprecation timeline for old method
4. Monitor for issues
5. Remove deprecated `prepare_secret_env()` in v0.3.0