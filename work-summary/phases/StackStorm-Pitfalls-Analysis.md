# StackStorm Pitfalls Analysis: Current Implementation Review

**Date:** 2024-01-02  
**Status:** Analysis Complete - Action Items Identified

## Executive Summary

This document analyzes the current Attune implementation against the StackStorm lessons learned to identify replicated pitfalls and propose solutions. The analysis reveals **3 critical issues** and **2 moderate concerns** that need to be addressed before production deployment.

---

## 1. HIGH COUPLING WITH CUSTOM ACTIONS ✅ AVOIDED

### StackStorm Problem
- Custom actions are tightly coupled to st2 services
- Minimal documentation around action/sensor service interfaces
- Actions must import st2 libraries and inherit from st2 classes

### Current Attune Status: **GOOD**
- ✅ Actions are executed as standalone processes via `tokio::process::Command`
- ✅ No Attune-specific imports or base classes required
- ✅ Runtime abstraction layer in `worker/src/runtime/` is well-designed
- ✅ Actions receive data via environment variables and stdin (code execution)

### Recommendations
- **Keep current approach** - the runtime abstraction is solid
- Consider documenting the runtime interface contract for pack developers
- Add examples of "pure" Python/Shell/Node.js actions that work without any Attune dependencies

---

## 2. TYPE SAFETY AND DOCUMENTATION ✅ AVOIDED

### StackStorm Problem
- Python with minimal type hints
- Runtime property injection makes types hard to determine
- Poor documentation of service interfaces

### Current Attune Status: **EXCELLENT**
- ✅ Built in Rust with full compile-time type checking
- ✅ All models in `common/src/models.rs` are strongly typed with SQLx
- ✅ Clear type definitions for `ExecutionContext`, `ExecutionResult`, `RuntimeError`
- ✅ Repository pattern enforces type contracts

### Recommendations
- **No changes needed** - Rust's type system provides the safety we need
- Continue documenting public APIs in `docs/` folder
- Consider generating OpenAPI specs from Axum routes for external consumers

---

## 3. LIMITED LANGUAGE ECOSYSTEM SUPPORT ⚠️ PARTIALLY ADDRESSED

### StackStorm Problem
- Only Python packs natively supported
- Other languages require custom installation logic
- No standard way to declare dependencies per language ecosystem

### Current Attune Status: **NEEDS WORK**

#### What's Good
- ✅ Runtime abstraction supports multiple languages (Python, Shell, Node.js planned)
- ✅ `Pack` model has `runtime_deps: Vec<String>` field for dependencies
- ✅ `Runtime` table has `distributions` JSONB and `installation` JSONB fields

#### Problems Identified

**Problem 3.1: No Dependency Installation Implementation**
```rust
// In crates/common/src/models.rs
pub struct Pack {
    // ...
    pub runtime_deps: Vec<String>,  // ← DEFINED BUT NOT USED
    // ...
}

pub struct Runtime {
    // ...
    pub distributions: JsonDict,     // ← NO INSTALLATION LOGIC
    pub installation: Option<JsonDict>,
    // ...
}
```

**Problem 3.2: No Pack Installation/Setup Service**
- No code exists to process `runtime_deps` field
- No integration with pip, npm, cargo, etc.
- No isolation of dependencies between packs

**Problem 3.3: Runtime Detection is Naive**
```rust
// In crates/worker/src/runtime/python.rs:279
fn can_execute(&self, context: &ExecutionContext) -> bool {
    // Only checks file extension - doesn't verify runtime availability
    context.action_ref.contains(".py")
        || context.entry_point.ends_with(".py")
        // ...
}
```

### Recommendations

**IMMEDIATE (Before Production):**
1. **Implement Pack Installation Service**
   - Create `attune-packman` service or add to `attune-api`
   - Support installing Python deps via `pip install -r requirements.txt`
   - Support installing Node.js deps via `npm install`
   - Store pack code in isolated directories: `/var/lib/attune/packs/{pack_ref}/`

2. **Enhance Runtime Model**
   - Add `installation_status` enum: `not_installed`, `installing`, `installed`, `failed`
   - Add `installed_at` timestamp
   - Add `installation_log` field for troubleshooting

3. **Implement Dependency Isolation**
   - Python: Use `venv` per pack in `/var/lib/attune/packs/{pack_ref}/.venv/`
   - Node.js: Use local `node_modules` per pack
   - Document in pack schema: how to declare dependencies

**FUTURE (v2.0):**
4. **Container-based Runtime**
   - Each pack gets its own container image
   - Dependencies baked into image
   - Complete isolation from Attune system

---

## 4. DEPENDENCY HELL AND SYSTEM COUPLING 🔴 CRITICAL ISSUE

### StackStorm Problem
- st2 services run on Python 2.7/3.6 (EOL)
- Upgrading st2 system breaks user actions
- User actions are coupled to st2 Python version

### Current Attune Status: **VULNERABLE**

#### Problems Identified

**Problem 4.1: Shared System Python Runtime**
```rust
// In crates/worker/src/runtime/python.rs:19
pub fn new() -> Self {
    Self {
        python_path: PathBuf::from("python3"),  // ← SYSTEM PYTHON!
        // ...
    }
}
```
- Currently uses system-wide `python3`
- If Attune upgrades system Python, user actions may break
- No version pinning or isolation

**Problem 4.2: No Runtime Version Management**
- No way to specify Python 3.9 vs 3.11 vs 3.12
- Runtime table has `name` field but it's not used for version selection
- Shell runtime hardcoded to `/bin/bash`

**Problem 4.3: Attune System Dependencies Could Conflict**
- If Attune worker needs a Python library (e.g., for parsing), it could conflict with action deps
- No separation between "Attune system dependencies" and "action dependencies"

### Recommendations

**CRITICAL (Must Fix Before v1.0):**

1. **Implement Per-Pack Virtual Environments**
   ```rust
   // Pseudocode for python.rs enhancement
   pub struct PythonRuntime {
       python_path: PathBuf,          // System python3 for venv creation
       venv_base: PathBuf,             // /var/lib/attune/packs/
       default_python_version: String, // "3.11"
   }
   
   impl PythonRuntime {
       async fn get_or_create_venv(&self, pack_ref: &str) -> Result<PathBuf> {
           let venv_path = self.venv_base.join(pack_ref).join(".venv");
           if !venv_path.exists() {
               self.create_venv(&venv_path).await?;
               self.install_pack_deps(pack_ref, &venv_path).await?;
           }
           Ok(venv_path.join("bin/python"))
       }
   }
   ```

2. **Support Multiple Runtime Versions**
   - Store available Python versions: `/opt/attune/runtimes/python-3.9/`, `.../python-3.11/`
   - Pack declares required version in metadata: `"runtime_version": "3.11"`
   - Worker selects appropriate runtime based on pack requirements

3. **Decouple Attune System from Action Execution**
   - Attune services (API, executor, worker) remain in Rust - no Python coupling
   - Actions run in isolated environments
   - Clear boundary: Attune communicates with actions only via stdin/stdout/env/files

**DESIGN PRINCIPLE:**
> "Upgrading Attune system dependencies should NEVER break existing user actions."

---

## 5. INSECURE SECRET PASSING 🔴 CRITICAL SECURITY ISSUE

### StackStorm Problem
- Secrets passed as environment variables or CLI arguments
- Visible to all users with login access via `ps`, `/proc/{pid}/environ`
- Major security vulnerability

### Current Attune Status: **VULNERABLE**

#### Problems Identified

**Problem 5.1: Secrets Exposed in Environment Variables**
```rust
// In crates/worker/src/secrets.rs:142
pub fn prepare_secret_env(&self, secrets: &HashMap<String, String>) 
    -> HashMap<String, String> {
    secrets
        .iter()
        .map(|(name, value)| {
            let env_name = format!("SECRET_{}", name.to_uppercase().replace('-', "_"));
            (env_name, value.clone())  // ← EXPOSED IN PROCESS ENV!
        })
        .collect()
}

// In crates/worker/src/executor.rs:228
env.extend(secret_env);  // ← Secrets added to env vars
```

**Problem 5.2: Secrets Visible in Process Table**
```rust
// In crates/worker/src/runtime/python.rs:122
let mut cmd = Command::new(&self.python_path);
cmd.arg("-c").arg(&script)
    .stdin(Stdio::null())  // ← NOT USING STDIN!
    // ...
for (key, value) in env {
    cmd.env(key, value);  // ← Secrets visible via /proc/{pid}/environ
}
```

**Problem 5.3: Parameters Also Exposed (Lower Risk)**
```rust
// In crates/worker/src/runtime/shell.rs:49
for (key, value) in &context.parameters {
    script.push_str(&format!(
        "export PARAM_{}='{}'\n",  // ← Parameters visible in env
        key.to_uppercase(),
        value_str
    ));
}
```

### Security Impact
- **HIGH**: Any user with shell access can view secrets via:
  - `ps auxwwe` - shows environment variables
  - `cat /proc/{pid}/environ` - shows full environment
  - `strings /proc/{pid}/environ` - extracts secret values
- **MEDIUM**: Short-lived processes reduce exposure window, but still vulnerable

### Recommendations

**CRITICAL (Must Fix Before v1.0):**

1. **Pass Secrets via Stdin (Preferred Method)**
   ```rust
   // Enhanced approach for python.rs
   async fn execute_python_code(
       &self,
       script: String,
       secrets: &HashMap<String, String>,
       parameters: &HashMap<String, serde_json::Value>,
       env: &HashMap<String, String>,  // Only non-secret env vars
       timeout_secs: Option<u64>,
   ) -> RuntimeResult<ExecutionResult> {
       // Create secrets JSON file
       let secrets_json = serde_json::to_string(&serde_json::json!({
           "secrets": secrets,
           "parameters": parameters,
       }))?;
       
       let mut cmd = Command::new(&self.python_path);
       cmd.arg("-c").arg(&script)
           .stdin(Stdio::piped())   // ← Use stdin!
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
       
       // Only add non-secret env vars
       for (key, value) in env {
           if !key.starts_with("SECRET_") {
               cmd.env(key, value);
           }
       }
       
       let mut child = cmd.spawn()?;
       
       // Write secrets to stdin and close
       if let Some(mut stdin) = child.stdin.take() {
           stdin.write_all(secrets_json.as_bytes()).await?;
           drop(stdin);  // Close stdin
       }
       
       let output = child.wait_with_output().await?;
       // ...
   }
   ```

2. **Alternative: Use Temporary Secret Files**
   ```rust
   // Create secure temporary file (0600 permissions)
   let secrets_file = format!("/tmp/attune-secrets-{}-{}.json", 
                               execution_id, uuid::Uuid::new_v4());
   let mut file = OpenOptions::new()
       .create_new(true)
       .write(true)
       .mode(0o600)  // Read/write for owner only
       .open(&secrets_file).await?;
   
   file.write_all(serde_json::to_string(secrets)?.as_bytes()).await?;
   file.sync_all().await?;
   drop(file);
   
   // Pass file path via env (not the secrets themselves)
   cmd.env("ATTUNE_SECRETS_FILE", &secrets_file);
   
   // Clean up after execution
   tokio::fs::remove_file(&secrets_file).await?;
   ```

3. **Update Python Wrapper Script**
   ```python
   # Modified wrapper script generator
   def main():
       import sys, json
       
       # Read secrets and parameters from stdin
       input_data = json.load(sys.stdin)
       secrets = input_data.get('secrets', {})
       parameters = input_data.get('parameters', {})
       
       # Secrets available in code but not in environment
       # ...
   ```

4. **Document Secure Secret Access Pattern**
   - Create `docs/secure-secret-handling.md`
   - Provide action templates that read from stdin
   - Add security best practices guide for pack developers

**IMPLEMENTATION PRIORITY: IMMEDIATE**
- This is a security vulnerability that must be fixed before any production use
- Should be addressed in Phase 3 (Worker Service completion)

---

## 6. STDERR DATABASE STORAGE CAUSING FAILURES ⚠️ MODERATE ISSUE

### StackStorm Problem
- stderr output stored directly in database
- Excessive logging can exceed database field limits
- Jobs fail unexpectedly due to log size

### Current Attune Status: **GOOD APPROACH, NEEDS LIMITS**

#### What's Good
✅ **Attune uses filesystem storage for logs**
```rust
// In crates/worker/src/artifacts.rs:72
pub async fn store_logs(
    &self,
    execution_id: i64,
    stdout: &str,
    stderr: &str,
) -> Result<Vec<Artifact>> {
    // Stores to files: /tmp/attune/artifacts/execution_{id}/stdout.log
    //                  /tmp/attune/artifacts/execution_{id}/stderr.log
    // NOT stored in database!
}
```

✅ **Database only stores result JSON**
```rust
// In crates/worker/src/executor.rs:331
let input = UpdateExecutionInput {
    status: Some(ExecutionStatus::Completed),
    result: result.result.clone(),  // ← Only structured result, not logs
    executor: None,
};
```

#### Problems Identified

**Problem 6.1: No Size Limits on Log Files**
```rust
// In artifacts.rs - no size checks!
file.write_all(stdout.as_bytes()).await?;  // ← Could be gigabytes!
```

**Problem 6.2: No Log Rotation**
- Single file per execution
- If action produces GB of logs, file grows unbounded
- Could fill disk

**Problem 6.3: In-Memory Log Collection**
```rust
// In python.rs and shell.rs
let output = execution_future.await?;
let stdout = String::from_utf8_lossy(&output.stdout).to_string();  // ← ALL in memory!
let stderr = String::from_utf8_lossy(&output.stderr).to_string();
```
- If action produces 1GB of output, worker could OOM

### Recommendations

**HIGH PRIORITY (Before Production):**

1. **Implement Streaming Log Collection**
   ```rust
   // Replace `.output()` with streaming approach
   use tokio::io::{AsyncBufReadExt, BufReader};
   
   async fn execute_with_streaming_logs(
       &self,
       mut cmd: Command,
       execution_id: i64,
       max_log_size: usize,  // e.g., 10MB
   ) -> RuntimeResult<ExecutionResult> {
       let mut child = cmd.spawn()?;
       
       // Stream stdout to file with size limit
       if let Some(stdout) = child.stdout.take() {
           let reader = BufReader::new(stdout);
           let mut lines = reader.lines();
           let mut total_size = 0;
           let log_file = /* open stdout.log */;
           
           while let Some(line) = lines.next_line().await? {
               total_size += line.len();
               if total_size > max_log_size {
                   // Truncate and add warning
                   write!(log_file, "\n[TRUNCATED: Log exceeded {}MB]", 
                          max_log_size / 1024 / 1024).await?;
                   break;
               }
               writeln!(log_file, "{}", line).await?;
           }
       }
       
       // Similar for stderr
       // ...
   }
   ```

2. **Add Configuration Limits**
   ```yaml
   # config.yaml
   worker:
     log_limits:
       max_stdout_size: 10485760  # 10MB
       max_stderr_size: 10485760  # 10MB
       max_total_size: 20971520   # 20MB
       truncate_on_exceed: true
   ```

3. **Implement Log Rotation Per Execution**
   ```
   /var/lib/attune/artifacts/
     execution_123/
       stdout.0.log      (first 10MB)
       stdout.1.log      (next 10MB)
       stdout.2.log      (final chunk)
       stderr.0.log
       result.json
   ```

4. **Add Log Streaming API Endpoint**
   - API endpoint: `GET /api/v1/executions/{id}/logs/stdout?follow=true`
   - Stream logs to client as execution progresses
   - Similar to `docker logs --follow`

**MEDIUM PRIORITY (v1.1):**

5. **Implement Log Compression**
   - Compress logs after execution completes
   - Save disk space for long-term retention
   - Decompress on-demand for viewing

---

## 7. POLICY EXECUTION ORDERING 🔴 CRITICAL ISSUE

### Problem Statement
When multiple executions are delayed due to policy enforcement (e.g., concurrency limits), there is no guaranteed ordering for when they will be scheduled once resources become available.

### Current Implementation Status: **MISSING CRITICAL FEATURE**

#### What Exists
✅ **Policy enforcement framework**
```rust
// In crates/executor/src/policy_enforcer.rs:428
pub async fn wait_for_policy_compliance(
    &self,
    action_id: Id,
    pack_id: Option<Id>,
    max_wait_seconds: u32,
) -> Result<bool> {
    // Polls until policies allow execution
    // BUT: No queue management!
}
```

✅ **Concurrency and rate limiting**
```rust
// Can detect when limits are exceeded
PolicyViolation::ConcurrencyLimitExceeded { limit: 5, current_count: 7 }
```

#### Problems Identified

**Problem 7.1: Non-Deterministic Scheduling Order**

**Scenario:**
```
Action has concurrency limit: 2
Time 0: E1 requested → starts (slot 1/2)
Time 1: E2 requested → starts (slot 2/2)
Time 2: E3 requested → DELAYED (no slots)
Time 3: E4 requested → DELAYED (no slots)
Time 4: E5 requested → DELAYED (no slots)
Time 5: E1 completes → which delayed execution runs?

Current behavior: UNDEFINED ORDER (possibly E5, then E3, then E4)
Expected behavior: FIFO - E3, then E4, then E5
```

**Problem 7.2: No Queue Data Structure**
```rust
// Current implementation in policy_enforcer.rs
// Only polls for compliance - no queue!
loop {
    if self.check_policies(action_id, pack_id).await?.is_none() {
        return Ok(true);  // ← Just returns true, no coordination
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

**Problem 7.3: Race Conditions**
- Multiple delayed executions poll simultaneously
- When slot opens, multiple executions might see it
- First to update wins, others keep waiting
- No fairness guarantee

**Problem 7.4: No Visibility into Queue**
- Can't see how many executions are waiting
- Can't see position in queue
- No way to estimate wait time
- Difficult to debug policy issues

### Business Impact

**Fairness Issues:**
- Later requests might execute before earlier ones
- Violates user expectations (FIFO is standard)
- Unpredictable execution order

**Workflow Dependencies:**
- Workflow step B requested after step A
- Step B might execute before A completes
- Data dependencies violated
- Incorrect results or failures

**Testing/Debugging:**
- Non-deterministic behavior hard to reproduce
- Integration tests become flaky
- Production issues difficult to diagnose

**Performance:**
- Polling wastes CPU cycles
- Multiple executions wake up unnecessarily
- Database load from repeated policy checks

### Recommendations

**CRITICAL (Must Fix Before v1.0):**

1. **Implement Per-Action Execution Queue**
   ```rust
   // New file: crates/executor/src/execution_queue.rs
   
   use std::collections::{HashMap, VecDeque};
   use tokio::sync::{Mutex, Notify};
   
   /// Manages FIFO queues of delayed executions per action
   pub struct ExecutionQueueManager {
       /// Queue per action_id
       queues: Arc<Mutex<HashMap<i64, ActionQueue>>>,
   }
   
   struct ActionQueue {
       /// FIFO queue of waiting execution IDs
       waiting: VecDeque<i64>,
       /// Notify when slot becomes available
       notify: Arc<Notify>,
       /// Current running count
       running_count: u32,
       /// Concurrency limit for this action
       limit: u32,
   }
   
   impl ExecutionQueueManager {
       /// Enqueue an execution (returns position in queue)
       pub async fn enqueue(&self, action_id: i64, execution_id: i64) -> usize {
           let mut queues = self.queues.lock().await;
           let queue = queues.entry(action_id).or_insert_with(ActionQueue::new);
           queue.waiting.push_back(execution_id);
           queue.waiting.len()
       }
       
       /// Wait for turn (blocks until this execution can proceed)
       pub async fn wait_for_turn(&self, action_id: i64, execution_id: i64) -> Result<()> {
           loop {
               // Check if it's our turn
               let notify = {
                   let mut queues = self.queues.lock().await;
                   let queue = queues.get_mut(&action_id).unwrap();
                   
                   // Are we at the front AND is there capacity?
                   if queue.waiting.front() == Some(&execution_id) 
                       && queue.running_count < queue.limit {
                       // It's our turn!
                       queue.waiting.pop_front();
                       queue.running_count += 1;
                       return Ok(());
                   }
                   
                   queue.notify.clone()
               };
               
               // Not our turn, wait for notification
               notify.notified().await;
           }
       }
       
       /// Mark execution as complete (frees up slot)
       pub async fn complete(&self, action_id: i64, execution_id: i64) {
           let mut queues = self.queues.lock().await;
           if let Some(queue) = queues.get_mut(&action_id) {
               queue.running_count = queue.running_count.saturating_sub(1);
               queue.notify.notify_one();  // Wake next waiting execution
           }
       }
       
       /// Get queue stats for monitoring
       pub async fn get_queue_stats(&self, action_id: i64) -> QueueStats {
           let queues = self.queues.lock().await;
           if let Some(queue) = queues.get(&action_id) {
               QueueStats {
                   waiting: queue.waiting.len(),
                   running: queue.running_count as usize,
                   limit: queue.limit as usize,
               }
           } else {
               QueueStats::default()
           }
       }
   }
   ```

2. **Integrate with PolicyEnforcer**
   ```rust
   // Update policy_enforcer.rs
   pub struct PolicyEnforcer {
       pool: PgPool,
       queue_manager: Arc<ExecutionQueueManager>,  // ← NEW
       // ... existing fields
   }
   
   pub async fn enforce_and_wait(
       &self,
       action_id: Id,
       execution_id: Id,
       pack_id: Option<Id>,
   ) -> Result<()> {
       // Check if policy would be violated
       if let Some(violation) = self.check_policies(action_id, pack_id).await? {
           match violation {
               PolicyViolation::ConcurrencyLimitExceeded { .. } => {
                   // Enqueue and wait for turn
                   let position = self.queue_manager.enqueue(action_id, execution_id).await;
                   info!("Execution {} queued at position {}", execution_id, position);
                   
                   self.queue_manager.wait_for_turn(action_id, execution_id).await?;
                   
                   info!("Execution {} proceeding after queue wait", execution_id);
               }
               _ => {
                   // Other policy types: retry with backoff
                   self.retry_with_backoff(action_id, pack_id).await?;
               }
           }
       }
       Ok(())
   }
   ```

3. **Update Scheduler to Use Queue**
   ```rust
   // In scheduler.rs
   async fn process_execution_requested(
       pool: &PgPool,
       publisher: &Publisher,
       policy_enforcer: &PolicyEnforcer,  // ← NEW parameter
       envelope: &MessageEnvelope<ExecutionRequestedPayload>,
   ) -> Result<()> {
       let execution_id = envelope.payload.execution_id;
       let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;
       let action = Self::get_action_for_execution(pool, &execution).await?;
       
       // Enforce policies with queueing
       policy_enforcer.enforce_and_wait(
           action.id,
           execution_id,
           Some(action.pack),
       ).await?;
       
       // Now proceed with scheduling
       let worker = Self::select_worker(pool, &action).await?;
       // ...
   }
   ```

4. **Add Completion Notification**
   ```rust
   // Worker must notify when execution completes
   // In worker/src/executor.rs
   
   async fn handle_execution_success(
       &self,
       execution_id: i64,
       action_id: i64,
       result: &ExecutionResult,
   ) -> Result<()> {
       // Update database
       ExecutionRepository::update(...).await?;
       
       // Notify queue manager (via message queue)
       let payload = ExecutionCompletedPayload {
           execution_id,
           action_id,
           status: ExecutionStatus::Completed,
       };
       self.publisher.publish("execution.completed", payload).await?;
       
       Ok(())
   }
   ```

5. **Add Queue Monitoring API**
   ```rust
   // New endpoint in API service
   /// GET /api/v1/actions/:id/queue-stats
   async fn get_action_queue_stats(
       State(state): State<Arc<AppState>>,
       Path(action_id): Path<i64>,
   ) -> Result<Json<ApiResponse<QueueStats>>> {
       let stats = state.queue_manager.get_queue_stats(action_id).await;
       Ok(Json(ApiResponse::success(stats)))
   }
   
   #[derive(Serialize)]
   pub struct QueueStats {
       pub waiting: usize,
       pub running: usize,
       pub limit: usize,
       pub avg_wait_time_seconds: Option<f64>,
   }
   ```

**IMPLEMENTATION PRIORITY: CRITICAL**
- This affects correctness and fairness of the system
- Must be implemented before production use
- Should be addressed in Phase 3 (Executor Service completion)

### Testing Requirements

**Unit Tests:**
- [ ] Queue maintains FIFO order
- [ ] Multiple executions enqueue correctly
- [ ] Dequeue happens in order
- [ ] Notify wakes correct waiting execution
- [ ] Concurrent enqueue/dequeue operations safe

**Integration Tests:**
- [ ] End-to-end execution ordering with policies
- [ ] Three executions with limit=1 execute in order
- [ ] Queue stats reflect actual state
- [ ] Worker completion notification releases queue slot

**Load Tests:**
- [ ] 1000 concurrent delayed executions
- [ ] Correct ordering maintained under load
- [ ] No missed notifications or deadlocks

---

## Summary of Critical Issues

| Issue | Severity | Status | Must Fix Before v1.0 |
|-------|----------|--------|---------------------|
| 1. Action Coupling | ✅ Good | Avoided | No |
| 2. Type Safety | ✅ Excellent | Avoided | No |
| 3. Language Ecosystems | ⚠️ Moderate | Partial | **Yes** - Implement pack installation |
| 4. Dependency Hell | 🔴 Critical | Vulnerable | **Yes** - Implement venv isolation |
| 5. Secret Security | 🔴 Critical | Vulnerable | **Yes** - Use stdin/files for secrets |
| 6. Log Storage | ⚠️ Moderate | Good Design | **Yes** - Add size limits |
| 7. Policy Execution Order | 🔴 Critical | Missing | **Yes** - Implement FIFO queue |

---

## Recommended Implementation Order

### Phase 1: Security & Correctness Fixes (Sprint 1 - Week 1-3)
**Priority: CRITICAL - Block All Other Work**

1. Fix secret passing vulnerability (Issue 5)
   - Implement stdin-based secret injection
   - Remove secrets from environment variables
   - Update Python/Shell runtime wrappers
   - Add security documentation

2. Implement execution queue for policies (Issue 7) **NEW**
   - FIFO queue per action
   - Notify mechanism for slot availability
   - Integration with PolicyEnforcer
   - Queue monitoring API

### Phase 2: Runtime Isolation (Sprint 2 - Week 4-5)
**Priority: HIGH - Required for Production**

3. Implement per-pack virtual environments (Issue 4)
   - Python venv creation per pack
   - Dependency installation service
   - Runtime version management

4. Add pack installation service (Issue 3)
   - Pack setup/teardown lifecycle
   - Dependency resolution
   - Installation status tracking

### Phase 3: Operational Hardening (Sprint 3 - Week 6-7)
**Priority: MEDIUM - Quality of Life**

5. Implement log size limits (Issue 6)
   - Streaming log collection
   - Size-based truncation
   - Configuration options

6. Add log rotation and compression
   - Multi-file logs
   - Automatic compression
   - Retention policies

### Phase 4: Advanced Features (v1.1+)
**Priority: LOW - Future Enhancement**

6. Container-based runtimes
7. Multi-version runtime support
8. Advanced dependency management
9. Log streaming API
10. Pack marketplace/registry

---

## Testing Checklist

Before marking issues as resolved, verify:

### Issue 5 (Secret Security)
- [ ] Secrets not visible in `ps auxwwe`
- [ ] Secrets not readable from `/proc/{pid}/environ`
- [ ] Actions can successfully read secrets from stdin/file
- [ ] Python wrapper script reads secrets securely
- [ ] Shell wrapper script reads secrets securely
- [ ] Documentation updated with secure patterns

### Issue 7 (Policy Execution Order) **NEW**
- [ ] Execution queue maintains FIFO order
- [ ] Three executions with limit=1 execute in correct order
- [ ] Queue stats API returns accurate counts
- [ ] Worker completion notification releases queue slot
- [ ] No race conditions under concurrent load
- [ ] Correct ordering with 1000 delayed executions

### Issue 4 (Dependency Isolation)
- [ ] Each pack gets isolated venv
- [ ] Installing pack A dependencies doesn't affect pack B
- [ ] Upgrading system Python doesn't break existing packs
- [ ] Runtime version can be specified per pack
- [ ] Multiple Python versions can coexist

### Issue 3 (Language Support)
- [ ] Python packs can declare dependencies in metadata
- [ ] `pip install` runs during pack installation
- [ ] Node.js packs supported with npm install
- [ ] Pack installation status tracked
- [ ] Failed installations reported with logs

### Issue 6 (Log Limits)
- [ ] Logs truncated at configured size limit
- [ ] Worker doesn't OOM on large output
- [ ] Truncation is clearly marked in logs
- [ ] Multiple log files created for rotation
- [ ] Old logs cleaned up per retention policy

---

## Architecture Decision Records

### ADR-001: Use Stdin for Secret Injection
**Decision:** Pass secrets via stdin as JSON instead of environment variables.

**Rationale:**
- Environment variables visible in `/proc/{pid}/environ`
- stdin content not exposed to other processes
- Follows principle of least privilege
- Industry best practice (used by Kubernetes, HashiCorp Vault)

**Consequences:**
- Requires wrapper script modifications
- Actions must explicitly read from stdin
- Slight increase in complexity
- **Major security improvement**

### ADR-002: Per-Pack Virtual Environments
**Decision:** Each pack gets isolated Python virtual environment.

**Rationale:**
- Prevents dependency conflicts between packs
- Allows different Python versions per pack
- Protects against system Python upgrades
- Standard practice in Python ecosystem

**Consequences:**
- Increased disk usage (one venv per pack)
- Pack installation takes longer
- Worker must manage venv lifecycle
- **Eliminates dependency hell**

### ADR-003: Filesystem-Based Log Storage
**Decision:** Store logs in filesystem, not database.

**Rationale:**
- Database not designed for large blob storage
- Filesystem handles large files efficiently
- Easy to implement rotation and compression
- Can stream logs without loading entire file

**Consequences:**
- Logs separate from structured execution data
- Need backup strategy for log directory
- Cleanup/retention requires separate process
- **Avoids database bloat and failures**

---

## References

- StackStorm Lessons Learned: `work-summary/StackStorm-Lessons-Learned.md`
- Current Worker Implementation: `crates/worker/src/`
- Runtime Abstraction: `crates/worker/src/runtime/`
- Secret Management: `crates/worker/src/secrets.rs`
- Artifact Storage: `crates/worker/src/artifacts.rs`
- Database Schema: `migrations/20240101000004_create_runtime_worker.sql`

---

## Next Steps

1. **Review this analysis with team** - Discuss priorities and timeline
2. **Create GitHub issues** - One issue per critical problem
3. **Update TODO.md** - Add tasks from Implementation Order section
4. **Begin Phase 1** - Security fixes first, before any other work
5. **Schedule security review** - After Phase 1 completion

---

**Document Status:** Complete - Ready for Review  
**Author:** AI Assistant  
**Reviewers Needed:** Security Team, Architecture Team, DevOps Lead