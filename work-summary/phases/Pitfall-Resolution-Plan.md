# Pitfall Resolution Action Plan

**Date:** 2024-01-02  
**Status:** Action Plan - Ready for Implementation  
**Related Document:** StackStorm-Pitfalls-Analysis.md

---

## Overview

This document provides a detailed, step-by-step action plan for resolving the critical issues identified in the StackStorm pitfalls analysis. Each issue has been broken down into concrete implementation tasks with estimated effort and dependencies.

---

## Critical Issues Summary

| ID | Issue | Severity | Estimated Effort | Priority |
|----|-------|----------|------------------|----------|
| P3 | Limited Language Ecosystem Support | ⚠️ Moderate | 5-7 days | P2 |
| P4 | Dependency Hell & System Coupling | 🔴 Critical | 7-10 days | P1 |
| P5 | Insecure Secret Passing | 🔴 Critical | 3-5 days | P0 |
| P6 | Log Storage Size Limits | ⚠️ Moderate | 3-4 days | P1 |
| P7 | Policy Execution Ordering | 🔴 Critical | 4-6 days | P0 |

**Total Estimated Effort:** 22-32 days (4.5-6.5 weeks)

---

## PHASE 1: CRITICAL CORRECTNESS & SECURITY FIXES

**Priority:** P0 (BLOCKING)  
**Estimated Time:** 7-11 days  
**Must Complete Before:** Any production deployment or public testing

---

## PHASE 1A: POLICY EXECUTION ORDERING FIX

**Priority:** P0 (BLOCKING)  
**Estimated Time:** 4-6 days  
**Must Complete Before:** Any production deployment

### P7.1: Implement Execution Queue Manager

**Files to Create:**
- `crates/executor/src/execution_queue.rs`
- `crates/executor/tests/execution_queue_tests.rs`

#### Task P7.1.1: Create ExecutionQueueManager Module
**New File:** `crates/executor/src/execution_queue.rs`

```rust
//! Execution Queue Manager
//!
//! Manages FIFO queues of delayed executions per action to ensure
//! proper ordering when policy limits are enforced.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use chrono::{DateTime, Utc};
use tracing::{debug, info};

/// Manages FIFO queues of delayed executions per action
pub struct ExecutionQueueManager {
    /// Queue per action_id
    queues: Arc<Mutex<HashMap<i64, ActionQueue>>>,
}

struct ActionQueue {
    /// FIFO queue of waiting execution IDs with enqueue time
    waiting: VecDeque<QueueEntry>,
    /// Notify when slot becomes available
    notify: Arc<Notify>,
    /// Current running count
    running_count: u32,
    /// Concurrency limit for this action
    limit: u32,
}

struct QueueEntry {
    execution_id: i64,
    enqueued_at: DateTime<Utc>,
}

impl ExecutionQueueManager {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Set concurrency limit for an action
    pub async fn set_limit(&self, action_id: i64, limit: u32) {
        let mut queues = self.queues.lock().await;
        queues.entry(action_id)
            .or_insert_with(|| ActionQueue::new(limit))
            .limit = limit;
    }
    
    /// Enqueue an execution (returns position in queue)
    pub async fn enqueue(&self, action_id: i64, execution_id: i64) -> usize {
        let mut queues = self.queues.lock().await;
        let queue = queues.entry(action_id)
            .or_insert_with(|| ActionQueue::new(1));
        
        let entry = QueueEntry {
            execution_id,
            enqueued_at: Utc::now(),
        };
        queue.waiting.push_back(entry);
        
        let position = queue.waiting.len();
        info!("Execution {} enqueued for action {} at position {}", 
              execution_id, action_id, position);
        position
    }
    
    /// Wait for turn (blocks until this execution can proceed)
    pub async fn wait_for_turn(&self, action_id: i64, execution_id: i64) -> Result<(), String> {
        loop {
            // Check if it's our turn
            let notify = {
                let mut queues = self.queues.lock().await;
                let queue = queues.get_mut(&action_id)
                    .ok_or_else(|| format!("No queue for action {}", action_id))?;
                
                // Are we at the front AND is there capacity?
                if let Some(front) = queue.waiting.front() {
                    if front.execution_id == execution_id && queue.running_count < queue.limit {
                        // It's our turn!
                        queue.waiting.pop_front();
                        queue.running_count += 1;
                        
                        info!("Execution {} proceeding (running: {}/{})", 
                              execution_id, queue.running_count, queue.limit);
                        return Ok(());
                    }
                }
                
                queue.notify.clone()
            };
            
            // Not our turn, wait for notification
            debug!("Execution {} waiting for notification", execution_id);
            notify.notified().await;
        }
    }
    
    /// Mark execution as complete (frees up slot)
    pub async fn complete(&self, action_id: i64, execution_id: i64) {
        let mut queues = self.queues.lock().await;
        if let Some(queue) = queues.get_mut(&action_id) {
            queue.running_count = queue.running_count.saturating_sub(1);
            info!("Execution {} completed for action {} (running: {}/{})", 
                  execution_id, action_id, queue.running_count, queue.limit);
            queue.notify.notify_one();  // Wake next waiting execution
        }
    }
    
    /// Get queue stats for monitoring
    pub async fn get_queue_stats(&self, action_id: i64) -> QueueStats {
        let queues = self.queues.lock().await;
        if let Some(queue) = queues.get(&action_id) {
            let avg_wait_time = if !queue.waiting.is_empty() {
                let now = Utc::now();
                let total_wait: i64 = queue.waiting.iter()
                    .map(|e| (now - e.enqueued_at).num_seconds())
                    .sum();
                Some(total_wait as f64 / queue.waiting.len() as f64)
            } else {
                None
            };
            
            QueueStats {
                waiting: queue.waiting.len(),
                running: queue.running_count as usize,
                limit: queue.limit as usize,
                avg_wait_time_seconds: avg_wait_time,
            }
        } else {
            QueueStats::default()
        }
    }
}

impl ActionQueue {
    fn new(limit: u32) -> Self {
        Self {
            waiting: VecDeque::new(),
            notify: Arc::new(Notify::new()),
            running_count: 0,
            limit,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueStats {
    pub waiting: usize,
    pub running: usize,
    pub limit: usize,
    pub avg_wait_time_seconds: Option<f64>,
}

impl Default for QueueStats {
    fn default() -> Self {
        Self {
            waiting: 0,
            running: 0,
            limit: 1,
            avg_wait_time_seconds: None,
        }
    }
}
```

**Acceptance Criteria:**
- [ ] ExecutionQueueManager created with FIFO queue per action
- [ ] Enqueue method adds to back of queue
- [ ] wait_for_turn blocks until execution is at front AND capacity available
- [ ] complete method decrements counter and notifies next
- [ ] Unit tests verify FIFO ordering
- [ ] Thread-safe with tokio::sync::Mutex

**Estimated Time:** 6 hours

---

#### Task P7.1.2: Integrate with PolicyEnforcer
**File:** `crates/executor/src/policy_enforcer.rs`

```rust
// Add field to PolicyEnforcer
pub struct PolicyEnforcer {
    pool: PgPool,
    queue_manager: Arc<ExecutionQueueManager>,  // ← NEW
    global_policy: ExecutionPolicy,
    pack_policies: HashMap<Id, ExecutionPolicy>,
    action_policies: HashMap<Id, ExecutionPolicy>,
}

impl PolicyEnforcer {
    pub fn new(pool: PgPool, queue_manager: Arc<ExecutionQueueManager>) -> Self {
        Self {
            pool,
            queue_manager,
            global_policy: ExecutionPolicy::default(),
            pack_policies: HashMap::new(),
            action_policies: HashMap::new(),
        }
    }
    
    /// Enforce policies with queueing support
    pub async fn enforce_and_wait(
        &self,
        action_id: Id,
        execution_id: Id,
        pack_id: Option<Id>,
    ) -> Result<()> {
        // Check if policy would be violated
        if let Some(violation) = self.check_policies(action_id, pack_id).await? {
            match violation {
                PolicyViolation::ConcurrencyLimitExceeded { limit, .. } => {
                    // Set limit in queue manager
                    self.queue_manager.set_limit(action_id, limit).await;
                    
                    // Enqueue and wait for turn
                    let position = self.queue_manager.enqueue(action_id, execution_id).await;
                    info!("Execution {} queued at position {} due to concurrency limit", 
                          execution_id, position);
                    
                    self.queue_manager.wait_for_turn(action_id, execution_id)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    
                    info!("Execution {} proceeding after queue wait", execution_id);
                }
                PolicyViolation::RateLimitExceeded { .. } => {
                    // Rate limit: retry with backoff
                    self.retry_with_backoff(action_id, pack_id, 60).await?;
                }
                _ => {
                    return Err(anyhow::anyhow!("Policy violation: {}", violation));
                }
            }
        }
        Ok(())
    }
    
    async fn retry_with_backoff(
        &self,
        action_id: Id,
        pack_id: Option<Id>,
        max_wait_seconds: u32,
    ) -> Result<()> {
        let start = Utc::now();
        let mut backoff_seconds = 1;
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(backoff_seconds)).await;
            
            if self.check_policies(action_id, pack_id).await?.is_none() {
                return Ok(());
            }
            
            if (Utc::now() - start).num_seconds() > max_wait_seconds as i64 {
                return Err(anyhow::anyhow!("Policy compliance timeout"));
            }
            
            backoff_seconds = (backoff_seconds * 2).min(10);
        }
    }
}
```

**Acceptance Criteria:**
- [ ] PolicyEnforcer has queue_manager field
- [ ] enforce_and_wait method uses queue for concurrency limits
- [ ] Rate limits use exponential backoff
- [ ] Tests verify integration

**Estimated Time:** 4 hours

---

#### Task P7.1.3: Update Scheduler to Use Queue
**File:** `crates/executor/src/scheduler.rs`

```rust
// Update ExecutionScheduler to include policy enforcement
pub struct ExecutionScheduler {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
    policy_enforcer: Arc<PolicyEnforcer>,  // ← NEW
}

impl ExecutionScheduler {
    pub fn new(
        pool: PgPool,
        publisher: Arc<Publisher>,
        consumer: Arc<Consumer>,
        policy_enforcer: Arc<PolicyEnforcer>,
    ) -> Self {
        Self {
            pool,
            publisher,
            consumer,
            policy_enforcer,
        }
    }
    
    async fn process_execution_requested(
        pool: &PgPool,
        publisher: &Publisher,
        policy_enforcer: &PolicyEnforcer,
        envelope: &MessageEnvelope<ExecutionRequestedPayload>,
    ) -> Result<()> {
        let execution_id = envelope.payload.execution_id;
        let execution = ExecutionRepository::find_by_id(pool, execution_id).await?
            .ok_or_else(|| anyhow::anyhow!("Execution not found"))?;
        let action = Self::get_action_for_execution(pool, &execution).await?;
        
        // Enforce policies with queueing - this may block
        policy_enforcer.enforce_and_wait(
            action.id,
            execution_id,
            Some(action.pack),
        ).await?;
        
        // Now proceed with scheduling
        let worker = Self::select_worker(pool, &action).await?;
        // ... rest of scheduling logic
    }
}
```

**Acceptance Criteria:**
- [ ] Scheduler calls enforce_and_wait before scheduling
- [ ] Blocked executions wait in queue
- [ ] Executions proceed in FIFO order

**Estimated Time:** 3 hours

---

#### Task P7.1.4: Add Completion Notification
**File:** `crates/worker/src/executor.rs`

```rust
// Update ActionExecutor to notify queue on completion
pub struct ActionExecutor {
    pool: PgPool,
    runtime_registry: RuntimeRegistry,
    artifact_manager: ArtifactManager,
    secret_manager: SecretManager,
    publisher: Arc<Publisher>,  // ← NEW: for notifications
}

async fn handle_execution_success(
    &self,
    execution_id: i64,
    action_id: i64,
    result: &ExecutionResult,
) -> Result<()> {
    // Update database
    let input = UpdateExecutionInput {
        status: Some(ExecutionStatus::Completed),
        result: result.result.clone(),
        executor: None,
    };
    ExecutionRepository::update(&self.pool, execution_id, input).await?;
    
    // Notify queue manager via message queue
    let payload = ExecutionCompletedPayload {
        execution_id,
        action_id,
        status: ExecutionStatus::Completed,
    };
    
    let envelope = MessageEnvelope::new(
        MessageType::ExecutionCompleted,
        payload,
    ).with_source("worker");
    
    self.publisher.publish_envelope(&envelope).await?;
    
    Ok(())
}

async fn handle_execution_failure(&self, execution_id: i64, action_id: i64, error: String) -> Result<()> {
    // Similar notification on failure
    // ...
}
```

**Acceptance Criteria:**
- [ ] Worker publishes ExecutionCompleted message
- [ ] Includes action_id for queue lookup
- [ ] Published on both success and failure

**Estimated Time:** 3 hours

---

#### Task P7.1.5: Add Executor Completion Listener
**File:** `crates/executor/src/completion_listener.rs` (new file)

```rust
//! Listens for execution completion messages and updates queue

use anyhow::Result;
use attune_common::mq::{Consumer, MessageEnvelope};
use std::sync::Arc;
use tracing::{debug, error, info};
use super::execution_queue::ExecutionQueueManager;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecutionCompletedPayload {
    pub execution_id: i64,
    pub action_id: i64,
    pub status: String,
}

pub struct CompletionListener {
    consumer: Arc<Consumer>,
    queue_manager: Arc<ExecutionQueueManager>,
}

impl CompletionListener {
    pub fn new(consumer: Arc<Consumer>, queue_manager: Arc<ExecutionQueueManager>) -> Self {
        Self {
            consumer,
            queue_manager,
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting execution completion listener");
        
        let queue_manager = self.queue_manager.clone();
        
        self.consumer.consume_with_handler(
            move |envelope: MessageEnvelope<ExecutionCompletedPayload>| {
                let queue_manager = queue_manager.clone();
                
                async move {
                    debug!("Received execution completed: {:?}", envelope.payload);
                    
                    queue_manager.complete(
                        envelope.payload.action_id,
                        envelope.payload.execution_id,
                    ).await;
                    
                    Ok(())
                }
            }
        ).await?;
        
        Ok(())
    }
}
```

**Acceptance Criteria:**
- [ ] Listener subscribes to execution.completed messages
- [ ] Calls queue_manager.complete on each message
- [ ] Error handling and logging

**Estimated Time:** 2 hours

---

#### Task P7.1.6: Add Queue Monitoring API
**File:** `crates/api/src/routes/actions.rs`

```rust
/// GET /api/v1/actions/:id/queue-stats
#[utoipa::path(
    get,
    path = "/api/v1/actions/{id}/queue-stats",
    params(
        ("id" = i64, Path, description = "Action ID")
    ),
    responses(
        (status = 200, description = "Queue statistics", body = ApiResponse<QueueStats>),
        (status = 404, description = "Action not found")
    ),
    tag = "actions"
)]
async fn get_action_queue_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<Identity>,
    Path(action_id): Path<i64>,
) -> Result<Json<ApiResponse<QueueStats>>> {
    // Verify action exists
    ActionRepository::find_by_id(&state.pool, action_id)
        .await?
        .ok_or_else(|| Error::not_found("Action", "id", action_id.to_string()))?;
    
    let stats = state.queue_manager.get_queue_stats(action_id).await;
    Ok(Json(ApiResponse::success(stats)))
}
```

**Acceptance Criteria:**
- [ ] API endpoint returns queue stats
- [ ] Stats include waiting, running, limit, avg wait time
- [ ] Requires authentication
- [ ] OpenAPI documentation

**Estimated Time:** 2 hours

---

#### Task P7.1.7: Integration Tests
**New File:** `crates/executor/tests/execution_queue_tests.rs`

```rust
#[tokio::test]
async fn test_fifo_ordering() {
    // Test that three executions with limit=1 execute in order
}

#[tokio::test]
async fn test_concurrent_enqueue() {
    // Test 100 concurrent enqueues maintain order
}

#[tokio::test]
async fn test_completion_releases_slot() {
    // Test that completing execution wakes next in queue
}

#[tokio::test]
async fn test_queue_stats() {
    // Test stats accurately reflect queue state
}
```

**Acceptance Criteria:**
- [ ] FIFO ordering verified with multiple executions
- [ ] Concurrent operations safe and ordered
- [ ] Completion notification works end-to-end
- [ ] Stats API accurate

**Estimated Time:** 6 hours

---

### P7 Subtotal: 4-6 days (26 hours estimated)

---

## PHASE 1B: SECURITY CRITICAL - SECRET PASSING FIX

**Priority:** P0 (BLOCKING)  
**Estimated Time:** 3-5 days  
**Must Complete Before:** Any production deployment or public testing

### P5.1: Implement Stdin-Based Secret Injection

**Files to Modify:**
- `crates/worker/src/runtime/mod.rs`
- `crates/worker/src/runtime/python.rs`
- `crates/worker/src/runtime/shell.rs`
- `crates/worker/src/executor.rs`
- `crates/worker/src/secrets.rs`

#### Task P5.1.1: Enhance ExecutionContext Structure
**File:** `crates/worker/src/runtime/mod.rs`

```rust
// Add new field to ExecutionContext
pub struct ExecutionContext {
    // ... existing fields ...
    
    /// Secrets to be passed via stdin (NOT environment variables)
    pub secrets: HashMap<String, String>,
    
    /// Secret injection method
    pub secret_method: SecretInjectionMethod,
}

pub enum SecretInjectionMethod {
    /// Pass secrets via stdin as JSON
    Stdin,
    /// Write secrets to temporary file and pass file path
    TempFile,
    /// DEPRECATED: Use environment variables (insecure)
    #[deprecated(note = "Insecure - secrets visible in process table")]
    EnvironmentVariables,
}
```

**Acceptance Criteria:**
- [ ] ExecutionContext includes secrets field
- [ ] SecretInjectionMethod enum defined
- [ ] Default method is Stdin
- [ ] All tests pass

**Estimated Time:** 1 hour

---

#### Task P5.1.2: Update SecretManager to Not Add Secrets to Env
**File:** `crates/worker/src/secrets.rs`

```rust
// REMOVE or deprecate this method
// pub fn prepare_secret_env(...) -> HashMap<String, String>

// ADD new method
impl SecretManager {
    /// Prepare secrets for secure injection
    /// Returns secrets as a HashMap, ready to be passed via stdin
    pub fn prepare_secrets(&self, secrets: &HashMap<String, String>) 
        -> HashMap<String, String> {
        // Just return as-is, no env var prefix needed
        secrets.clone()
    }
    
    /// Serialize secrets to JSON for stdin injection
    pub fn serialize_secrets_for_stdin(
        &self,
        secrets: &HashMap<String, String>,
        parameters: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let payload = serde_json::json!({
            "secrets": secrets,
            "parameters": parameters,
        });
        serde_json::to_string(&payload)
            .map_err(|e| Error::Internal(format!("Failed to serialize secrets: {}", e)))
    }
}
```

**Acceptance Criteria:**
- [ ] prepare_secret_env method removed or deprecated
- [ ] New serialize_secrets_for_stdin method added
- [ ] Unit tests pass
- [ ] Secrets no longer added to environment variables

**Estimated Time:** 2 hours

---

#### Task P5.1.3: Update Executor to Pass Secrets Separately
**File:** `crates/worker/src/executor.rs`

```rust
async fn prepare_execution_context(
    &self,
    execution: &Execution,
    action: &Action,
) -> Result<ExecutionContext> {
    // ... existing parameter extraction ...
    
    // Fetch secrets but DO NOT add to env
    let secrets = match self.secret_manager.fetch_secrets_for_action(action).await {
        Ok(secrets) => secrets,
        Err(e) => {
            warn!("Failed to fetch secrets for action {}: {}", action.r#ref, e);
            HashMap::new()
        }
    };
    
    // Environment variables - NO SECRETS!
    let mut env = HashMap::new();
    env.insert("ATTUNE_EXECUTION_ID".to_string(), execution.id.to_string());
    env.insert("ATTUNE_ACTION_REF".to_string(), execution.action_ref.clone());
    // ... other non-secret env vars ...
    
    // DO NOT DO THIS ANYMORE:
    // env.extend(secret_env);  ← REMOVED
    
    let context = ExecutionContext {
        execution_id: execution.id,
        action_ref: execution.action_ref.clone(),
        parameters,
        env,
        secrets,  // ← NEW: Passed separately
        secret_method: SecretInjectionMethod::Stdin,  // ← NEW
        timeout,
        working_dir: None,
        entry_point,
        code: None,
        code_path: None,
    };
    
    Ok(context)
}
```

**Acceptance Criteria:**
- [ ] Secrets fetched but not added to env
- [ ] Secrets passed in ExecutionContext.secrets field
- [ ] Environment variables contain no SECRET_ prefixed keys
- [ ] Tests verify env vars don't contain secrets

**Estimated Time:** 1 hour

---

#### Task P5.1.4: Implement Stdin Injection in Python Runtime
**File:** `crates/worker/src/runtime/python.rs`

```rust
async fn execute_python_code(
    &self,
    script: String,
    context: &ExecutionContext,
) -> RuntimeResult<ExecutionResult> {
    let start = Instant::now();
    
    // Build command
    let mut cmd = Command::new(&self.python_path);
    cmd.arg("-c")
        .arg(&script)
        .stdin(Stdio::piped())  // ← CHANGED from Stdio::null()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Add environment variables (NO SECRETS)
    for (key, value) in &context.env {
        cmd.env(key, value);
    }
    
    // Spawn process
    let mut child = cmd.spawn()?;
    
    // Write secrets and parameters to stdin as JSON
    if let Some(mut stdin) = child.stdin.take() {
        let stdin_data = serde_json::json!({
            "secrets": context.secrets,
            "parameters": context.parameters,
        });
        let stdin_bytes = serde_json::to_vec(&stdin_data)?;
        
        stdin.write_all(&stdin_bytes).await
            .map_err(|e| RuntimeError::ProcessError(format!("Failed to write to stdin: {}", e)))?;
        
        // Close stdin to signal EOF
        drop(stdin);
    }
    
    // Wait for execution with timeout
    let output = if let Some(timeout_secs) = context.timeout {
        match timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
            Ok(output) => output?,
            Err(_) => {
                let _ = child.kill().await;
                return Ok(ExecutionResult::failure(
                    -1,
                    String::new(),
                    format!("Execution timed out after {} seconds", timeout_secs),
                    start.elapsed().as_millis() as u64,
                ));
            }
        }
    } else {
        child.wait_with_output().await?
    };
    
    // ... rest of result processing ...
}
```

**Acceptance Criteria:**
- [ ] Stdin is piped, not null
- [ ] Secrets and parameters written to stdin as JSON
- [ ] Stdin closed after writing
- [ ] No secrets in environment variables
- [ ] Tests verify stdin content

**Estimated Time:** 3 hours

---

#### Task P5.1.5: Update Python Wrapper Script Generator
**File:** `crates/worker/src/runtime/python.rs`

```rust
fn generate_wrapper_script(&self, context: &ExecutionContext) -> RuntimeResult<String> {
    // NEW: Don't embed parameters in script, read from stdin
    let wrapper = format!(
        r#"#!/usr/bin/env python3
import sys
import json
import traceback

def main():
    try:
        # Read input data from stdin
        input_data = json.load(sys.stdin)
        secrets = input_data.get('secrets', {{}})
        parameters = input_data.get('parameters', {{}})
        
        # Make secrets available as a dict (not env vars)
        # Actions access via: secrets['api_key']
        
        # Execute the action code
        action_code = '''{}'''
        
        # Create namespace with secrets and parameters
        namespace = {{
            '__name__': '__main__',
            'secrets': secrets,
            'parameters': parameters,
        }}
        
        exec(action_code, namespace)
        
        # Look for entry point function
        if '{}' in namespace:
            result = namespace['{}'](**parameters)
        elif 'run' in namespace:
            result = namespace['run'](parameters=parameters, secrets=secrets)
        elif 'main' in namespace:
            result = namespace['main'](parameters=parameters, secrets=secrets)
        else:
            result = {{'status': 'no_entry_point'}}
        
        # Output result as JSON
        if result is not None:
            print(json.dumps({{'result': result, 'status': 'success'}}))
        else:
            print(json.dumps({{'status': 'success'}}))
        
        sys.exit(0)
    
    except Exception as e:
        error_info = {{
            'status': 'error',
            'error': str(e),
            'error_type': type(e).__name__,
            'traceback': traceback.format_exc()
        }}
        print(json.dumps(error_info), file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
"#,
        context.code.as_deref().unwrap_or(""),
        context.entry_point,
        context.entry_point
    );
    
    Ok(wrapper)
}
```

**Acceptance Criteria:**
- [ ] Wrapper reads from stdin, not embedded JSON
- [ ] Secrets available as dict in action code
- [ ] Parameters passed to entry point function
- [ ] Error handling maintains security
- [ ] Tests verify wrapper works correctly

**Estimated Time:** 2 hours

---

#### Task P5.1.6: Implement Stdin Injection in Shell Runtime
**File:** `crates/worker/src/runtime/shell.rs`

```rust
fn generate_wrapper_script(&self, context: &ExecutionContext) -> RuntimeResult<String> {
    let mut script = String::new();
    
    script.push_str("#!/bin/bash\n");
    script.push_str("set -e\n\n");
    
    // Read secrets and parameters from stdin
    script.push_str("# Read input from stdin\n");
    script.push_str("INPUT_JSON=$(cat)\n\n");
    
    // Parse JSON and extract secrets
    script.push_str("# Extract secrets (requires jq)\n");
    script.push_str("if command -v jq &> /dev/null; then\n");
    script.push_str("  # Export each secret as SECRET_NAME\n");
    script.push_str("  for key in $(echo \"$INPUT_JSON\" | jq -r '.secrets | keys[]'); do\n");
    script.push_str("    value=$(echo \"$INPUT_JSON\" | jq -r \".secrets[\\\"$key\\\"]\")\n");
    script.push_str("    export \"SECRET_${key}\"=\"$value\"\n");
    script.push_str("  done\n");
    script.push_str("  \n");
    script.push_str("  # Export each parameter as PARAM_NAME\n");
    script.push_str("  for key in $(echo \"$INPUT_JSON\" | jq -r '.parameters | keys[]'); do\n");
    script.push_str("    value=$(echo \"$INPUT_JSON\" | jq -r \".parameters[\\\"$key\\\"]\")\n");
    script.push_str("    export \"PARAM_${key}\"=\"$value\"\n");
    script.push_str("  done\n");
    script.push_str("else\n");
    script.push_str("  echo 'ERROR: jq is required for shell actions' >&2\n");
    script.push_str("  exit 1\n");
    script.push_str("fi\n\n");
    
    // Add the action code
    script.push_str("# Action code\n");
    if let Some(code) = &context.code {
        script.push_str(code);
    }
    
    Ok(script)
}

async fn execute_shell_code(
    &self,
    script: String,
    context: &ExecutionContext,
) -> RuntimeResult<ExecutionResult> {
    // ... similar stdin injection as Python runtime ...
}
```

**Acceptance Criteria:**
- [ ] Shell wrapper reads from stdin
- [ ] Requires jq for JSON parsing
- [ ] Secrets exported as SECRET_ variables (but from stdin, not process env)
- [ ] Parameters exported as PARAM_ variables
- [ ] Tests verify functionality

**Estimated Time:** 3 hours

---

#### Task P5.1.7: Security Testing
**New File:** `crates/worker/tests/security_test.rs`

```rust
#[tokio::test]
async fn test_secrets_not_in_process_env() {
    // Create execution with secrets
    // Spawn process
    // Read /proc/{pid}/environ
    // Assert secrets not present
}

#[tokio::test]
async fn test_secrets_not_visible_in_ps() {
    // Similar test using ps command output
}

#[tokio::test]
async fn test_action_can_read_secrets_from_stdin() {
    // Action successfully accesses secrets
}
```

**Acceptance Criteria:**
- [ ] Test verifies secrets not in /proc/pid/environ
- [ ] Test verifies secrets not in ps output
- [ ] Test verifies action can read secrets
- [ ] All security tests pass

**Estimated Time:** 4 hours

---

#### Task P5.1.8: Documentation
**New File:** `docs/secure-secret-handling.md`

Content should include:
- Architecture decision: why stdin vs env vars
- How to access secrets in Python actions
- How to access secrets in Shell actions
- Security best practices for pack developers
- Migration guide from old env var approach

**Estimated Time:** 2 hours

---

### P5 Subtotal: 3-5 days (18 hours estimated)

---

### PHASE 1 TOTAL: 7-11 days (44 hours estimated for P7 + P5)

---

## PHASE 2: DEPENDENCY ISOLATION

**Priority:** P1 (HIGH)  
**Estimated Time:** 7-10 days  
**Depends On:** None (can run in parallel with P5)

### P4.1: Implement Per-Pack Virtual Environments

#### Task P4.1.1: Add Pack Storage Directory Structure
**New File:** `crates/common/src/pack_storage.rs`

```rust
/// Manages pack storage on filesystem
pub struct PackStorage {
    base_dir: PathBuf,  // /var/lib/attune/packs
}

impl PackStorage {
    pub fn get_pack_dir(&self, pack_ref: &str) -> PathBuf {
        self.base_dir.join(pack_ref)
    }
    
    pub fn get_pack_venv_dir(&self, pack_ref: &str) -> PathBuf {
        self.get_pack_dir(pack_ref).join(".venv")
    }
    
    pub fn get_pack_code_dir(&self, pack_ref: &str) -> PathBuf {
        self.get_pack_dir(pack_ref).join("actions")
    }
    
    pub async fn initialize_pack_dir(&self, pack_ref: &str) -> Result<()> {
        // Create directory structure
    }
}
```

**Estimated Time:** 2 hours

---

#### Task P4.1.2: Implement Python Venv Manager
**New File:** `crates/worker/src/runtime/venv.rs`

```rust
pub struct VenvManager {
    python_path: PathBuf,
    venv_base: PathBuf,
}

impl VenvManager {
    pub async fn create_venv(&self, pack_ref: &str) -> Result<PathBuf> {
        let venv_path = self.venv_base.join(pack_ref).join(".venv");
        
        if venv_path.exists() {
            return Ok(venv_path);
        }
        
        // Create venv using: python3 -m venv {path}
        let output = Command::new(&self.python_path)
            .arg("-m")
            .arg("venv")
            .arg(&venv_path)
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(Error::Internal(format!(
                "Failed to create venv: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        
        Ok(venv_path)
    }
    
    pub fn get_venv_python(&self, pack_ref: &str) -> PathBuf {
        self.venv_base
            .join(pack_ref)
            .join(".venv/bin/python")
    }
    
    pub async fn install_requirements(
        &self,
        pack_ref: &str,
        requirements: &[String],
    ) -> Result<()> {
        let python = self.get_venv_python(pack_ref);
        
        // Write requirements.txt
        let req_file = self.venv_base
            .join(pack_ref)
            .join("requirements.txt");
        
        tokio::fs::write(&req_file, requirements.join("\n")).await?;
        
        // Install: python -m pip install -r requirements.txt
        let output = Command::new(&python)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("-r")
            .arg(&req_file)
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(Error::Internal(format!(
                "Failed to install requirements: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        
        Ok(())
    }
}
```

**Estimated Time:** 4 hours

---

#### Task P4.1.3: Update PythonRuntime to Use Venv
**File:** `crates/worker/src/runtime/python.rs`

```rust
pub struct PythonRuntime {
    system_python: PathBuf,  // For venv creation
    venv_manager: VenvManager,
    work_dir: PathBuf,
}

impl PythonRuntime {
    async fn get_python_for_action(
        &self,
        action: &Action,
        pack_ref: &str,
    ) -> RuntimeResult<PathBuf> {
        // Check if venv exists for this pack
        let venv_python = self.venv_manager.get_venv_python(pack_ref);
        
        if venv_python.exists() {
            return Ok(venv_python);
        }
        
        // Create venv if it doesn't exist
        warn!("No venv found for pack {}, creating...", pack_ref);
        let venv_path = self.venv_manager.create_venv(pack_ref).await
            .map_err(|e| RuntimeError::SetupError(e.to_string()))?;
        
        Ok(venv_path.join("bin/python"))
    }
}
```

**Estimated Time:** 3 hours

---

#### Task P4.1.4: Add Pack Installation Endpoint
**File:** `crates/api/src/routes/packs.rs`

```rust
/// POST /api/v1/packs/:ref/install
async fn install_pack(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<Identity>,
    Path(pack_ref): Path<String>,
) -> Result<Json<ApiResponse<InstallationStatus>>> {
    // Trigger pack installation
    // - Create venv
    // - Install dependencies from pack.runtime_deps
    // - Update pack status
}

/// GET /api/v1/packs/:ref/installation-status
async fn get_installation_status(
    State(state): State<Arc<AppState>>,
    Path(pack_ref): Path<String>,
) -> Result<Json<ApiResponse<InstallationStatus>>> {
    // Return installation status
}
```

**Estimated Time:** 4 hours

---

#### Task P4.1.5: Database Schema Updates
**New Migration:** `migrations/20240103000001_add_pack_installation_status.sql`

```sql
-- Add installation tracking to pack table
ALTER TABLE attune.pack 
ADD COLUMN installation_status TEXT DEFAULT 'not_installed',
ADD COLUMN installed_at TIMESTAMPTZ,
ADD COLUMN installation_log TEXT;

-- Create index
CREATE INDEX idx_pack_installation_status 
ON attune.pack(installation_status);
```

**Estimated Time:** 1 hour

---

#### Task P4.1.6: Background Worker for Pack Installation
**New File:** `crates/worker/src/pack_installer.rs`

```rust
pub struct PackInstaller {
    pool: PgPool,
    venv_manager: VenvManager,
    pack_storage: PackStorage,
}

impl PackInstaller {
    pub async fn install_pack(&self, pack_id: i64) -> Result<()> {
        // Load pack from DB
        // Parse runtime_deps
        // Create venv
        // Install dependencies
        // Update installation status
        // Log installation output
    }
    
    pub async fn uninstall_pack(&self, pack_id: i64) -> Result<()> {
        // Remove venv
        // Remove pack files
        // Update status
    }
}
```

**Estimated Time:** 6 hours

---

### P4 Subtotal: 7-10 days (20 hours estimated)

---

## PHASE 3: LANGUAGE ECOSYSTEM SUPPORT

**Priority:** P2 (MEDIUM)  
**Estimated Time:** 5-7 days  
**Depends On:** P4 (needs pack storage structure)

### P3.1: Implement Pack Installation Service

#### Task P3.1.1: Pack Metadata Schema
**Update:** `crates/common/src/models.rs`

```rust
/// Pack dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackDependencies {
    pub python: Option<PythonDependencies>,
    pub nodejs: Option<NodeJsDependencies>,
    pub system: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonDependencies {
    pub version: Option<String>,  // ">=3.9,<4.0"
    pub packages: Vec<String>,     // ["requests>=2.28.0", "boto3"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeJsDependencies {
    pub version: Option<String>,
    pub packages: Vec<String>,
}
```

**Estimated Time:** 2 hours

---

#### Task P3.1.2: Node.js Runtime with npm Support
**New File:** `crates/worker/src/runtime/nodejs.rs`

```rust
pub struct NodeJsRuntime {
    node_path: PathBuf,
    npm_path: PathBuf,
    work_dir: PathBuf,
}

impl NodeJsRuntime {
    async fn install_npm_packages(
        &self,
        pack_ref: &str,
        packages: &[String],
    ) -> RuntimeResult<()> {
        // Create package.json
        // Run npm install
    }
    
    async fn execute_nodejs_code(
        &self,
        script: String,
        context: &ExecutionContext,
    ) -> RuntimeResult<ExecutionResult> {
        // Similar to Python runtime
        // Pass secrets via stdin
    }
}
```

**Estimated Time:** 8 hours

---

#### Task P3.1.3: Runtime Detection Enhancement
**File:** `crates/worker/src/runtime/mod.rs`

```rust
/// Runtime registry with smarter detection
impl RuntimeRegistry {
    pub fn get_runtime_for_action(
        &self,
        action: &Action,
        runtime_info: &Runtime,
    ) -> RuntimeResult<&dyn Runtime> {
        // Use action.runtime field to select runtime
        // Fall back to file extension detection
        // Validate runtime is installed and ready
    }
}
```

**Estimated Time:** 3 hours

---

#### Task P3.1.4: Pack Upload and Extraction
**New File:** `crates/api/src/routes/pack_upload.rs`

```rust
/// POST /api/v1/packs/upload
/// Accepts .zip or .tar.gz with pack files
async fn upload_pack(
    multipart: Multipart,
) -> Result<Json<ApiResponse<Pack>>> {
    // Extract archive
    // Parse pack.yaml
    // Store pack metadata in DB
    // Store pack files in pack storage
    // Trigger installation
}
```

**Estimated Time:** 6 hours

---

### P3 Subtotal: 5-7 days (19 hours estimated)

---

## PHASE 4: LOG SIZE LIMITS

**Priority:** P1 (HIGH)  
**Estimated Time:** 3-4 days  
**Depends On:** None (can run in parallel)

### P6.1: Implement Streaming Log Collection

#### Task P6.1.1: Add Configuration for Log Limits
**File:** `crates/common/src/config.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    // ... existing fields ...
    
    #[serde(default)]
    pub log_limits: LogLimits,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogLimits {
    #[serde(default = "default_max_stdout")]
    pub max_stdout_size: usize,  // 10MB
    
    #[serde(default = "default_max_stderr")]
    pub max_stderr_size: usize,  // 10MB
    
    #[serde(default = "default_truncate")]
    pub truncate_on_exceed: bool,  // true
}

fn default_max_stdout() -> usize { 10 * 1024 * 1024 }
fn default_max_stderr() -> usize { 10 * 1024 * 1024 }
fn default_truncate() -> bool { true }
```

**Estimated Time:** 1 hour

---

#### Task P6.1.2: Implement Streaming Log Writer
**New File:** `crates/worker/src/log_writer.rs`

```rust
pub struct BoundedLogWriter {
    file: File,
    current_size: usize,
    max_size: usize,
    truncated: bool,
}

impl BoundedLogWriter {
    pub async fn write_line(&mut self, line: &str) -> Result<()> {
        if self.truncated {
            return Ok(());
        }
        
        let line_size = line.len() + 1;  // +1 for newline
        
        if self.current_size + line_size > self.max_size {
            // Write truncation notice
            let notice = format!(
                "\n[TRUNCATED: Log exceeded {} MB limit]\n",
                self.max_size / 1024 / 1024
            );
            self.file.write_all(notice.as_bytes()).await?;
            self.truncated = true;
            return Ok(());
        }
        
        self.file.write_all(line.as_bytes()).await?;
        self.file.write_all(b"\n").await?;
        self.current_size += line_size;
        
        Ok(())
    }
}
```

**Estimated Time:** 2 hours

---

#### Task P6.1.3: Update Python Runtime with Streaming
**File:** `crates/worker/src/runtime/python.rs`

```rust
async fn execute_with_streaming(
    &self,
    mut cmd: Command,
    execution_id: i64,
    log_limits: &LogLimits,
) -> RuntimeResult<ExecutionResult> {
    let mut child = cmd.spawn()?;
    
    // Spawn tasks to stream stdout and stderr
    let stdout_handle = if let Some(stdout) = child.stdout.take() {
        let exec_id = execution_id;
        let max_size = log_limits.max_stdout_size;
        tokio::spawn(async move {
            stream_to_file(stdout, exec_id, "stdout", max_size).await
        })
    } else {
        // Handle None case
    };
    
    let stderr_handle = if let Some(stderr) = child.stderr.take() {
        // Similar for stderr
    };
    
    // Wait for process to complete
    let status = child.wait().await?;
    
    // Wait for log streaming to complete
    let stdout_result = stdout_handle.await??;
    let stderr_result = stderr_handle.await??;
    
    // Return ExecutionResult
}

async fn stream_to_file(
    stream: ChildStdout,
    execution_id: i64,
    stream_name: &str,
    max_size: usize,
) -> Result<StreamResult> {
    let log_path = format!("/var/lib/attune/artifacts/execution_{}/{}.log",
                           execution_id, stream_name);
    let file = File::create(&log_path).await?;
    let mut writer = BoundedLogWriter::new(file, max_size);
    
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    
    while let Some(line) = lines.next_line().await? {
        writer.write_line(&line).await?;
    }
    
    Ok(StreamResult {
        total_size: writer.current_size,
        truncated: writer.truncated,
    })
}
```

**Estimated Time:** 6 hours

---

#### Task P6.1.4: Update Artifacts Manager
**File:** `crates/worker/src/artifacts.rs`

```rust
// Add method to read logs with pagination
pub async fn read_log_page(
    &self,
    execution_id: i64,
    stream: &str,  // "stdout" or "stderr"
    offset: usize,
    limit: usize,
) -> Result<LogPage> {
    let log_path = self.get_execution_dir(execution_id)
        .join(format!("{}.log", stream));
    
    // Read file with offset/limit
    // Return LogPage with content and metadata
}

pub struct LogPage {
    pub content: String,
    pub offset: usize,
    pub total_size: usize,
    pub truncated: bool,
}
```

**Estimated Time:** 2 hours

---

#### Task P6.1.5: Add Log Retrieval API Endpoint
**File:** `crates/api/src/routes/executions.rs`

```rust
/// GET /api/v1/executions/:id/logs/:stream?offset=0&limit=1000
async fn get_execution_logs(
    State(state): State<Arc<AppState>>,
    Path((exec_id, stream)): Path<(i64, String)>,
    Query(params): Query<LogQueryParams>,
) -> Result<Json<ApiResponse<LogPage>>> {
    // Read logs from artifact storage
    // Return paginated results
}
```

**Estimated Time:** 3 hours

---

### P6 Subtotal: 3-4 days (14 hours estimated)

---

## Testing Strategy

### Unit Tests
- [ ] Secret injection methods (stdin, temp file)
- [ ] Venv creation and management
- [ ] Dependency installation
- [ ] Log size limiting
- [ ] Log streaming

### Integration Tests
- [ ] End-to-end action execution with secrets
- [ ] Pack installation workflow
- [ ] Multiple packs with different Python versions
- [ ] Large log output handling
- [ ] Concurrent executions

### Security Tests
- [ ] Secrets not visible in process table
- [ ] Secrets not in /proc/pid/environ
- [ ] Secrets not in command line args
- [ ] Actions can successfully access secrets
- [ ] Venv isolation between packs

### Performance Tests
- [ ] Log streaming with high throughput
- [ ] Many concurrent executions
- [ ] Large pack installations
- [ ] Memory usage with large logs

---

## Rollout Plan

### Phase 1: Development (Weeks 1-3)
- Implement all features in feature branches
- Unit testing for each component
- Code review for security-critical changes

### Phase 2: Integration Testing (Week 4)
- Merge all feature branches
- Run full integration test suite
- Security audit of secret handling
- Performance testing

### Phase 3: Beta Testing (Week 5)
- Deploy to staging environment
- Internal testing with real packs
- Gather feedback
- Fix critical issues

### Phase 4: Production Release (Week 6)
- Final security review
- Documentation complete
- Deploy to production
- Monitor for issues

---

## Success Criteria

### Must Have (v1.0)
- ✅ Secrets not visible in process table
- ✅ Per-pack Python virtual environments
- ✅ Pack installation with dependency management
- ✅ Log size limits enforced
- ✅ All security tests passing
- ✅ Documentation complete

### Nice to Have (v1.1)
- Multiple Python version support
- Node.js runtime fully implemented
- Log streaming API
- Container-based runtimes
- Pack marketplace

---

## Risk Mitigation

### Risk: Breaking Existing Packs
**Mitigation:** 
- Maintain backward compatibility mode
- Provide migration guide
- Gradual rollout with opt-in

### Risk: Performance Degradation
**Mitigation:**
- Performance benchmarks before/after
- Optimize hot paths
- Add caching where appropriate

### Risk: Security Vulnerabilities
**Mitigation:**
- External security audit
- Penetration testing
- Bug bounty program

### Risk: Complex Dependency Resolution
**Mitigation:**
- Start with simple requirements.txt
- Document dependency conflicts
- Provide troubleshooting guide

---

## Resources Needed

### Development
- 1-2 senior Rust engineers (3-5 weeks)
- Security consultant (1 week review)
- Technical writer (documentation)

### Infrastructure
- Staging environment for testing
- CI/CD pipeline updates
- Log storage (S3 or similar)

### Tools
- Security scanning tools
- Performance profiling tools
- Load testing framework

---

## Next Steps

1. **Review and Approve Plan** - Team meeting to review
2. **Create GitHub Issues** - One per major task
3. **Assign Owners** - Who owns each phase?
4. **Set Milestones** - Weekly checkpoints
5. **Begin Phase 1** - Security fixes first!

---

**Status:** Ready for Review  
**Last Updated:** 2024-01-02  
**Next Review:** After team approval