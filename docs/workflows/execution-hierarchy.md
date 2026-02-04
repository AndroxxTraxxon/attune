# Execution Hierarchy and Parent Relationships

## Overview

The `execution` table supports two types of parent-child relationships:

1. **General execution hierarchies** (via `parent` field)
2. **Workflow task executions** (via `workflow_task` metadata)

This document explains why both are needed, how they differ, and when to use each.

## Field Purposes

### `execution.parent` (General Hierarchy)

**Type**: `Option<Id>` - Foreign key to `execution.id`

**Purpose**: Generic execution tree traversal for ANY type of parent-child relationship.

**Used for**:
- **Workflow tasks**: Parent is the workflow's main execution record
- **Child actions**: Parent is the action that spawned them  
- **Nested workflows**: Parent is the outer workflow's execution
- **Any future parent-child patterns**

**Example SQL**:
```sql
-- Find all child executions (any type)
SELECT * FROM attune.execution WHERE parent = 100;
```

### `execution.workflow_task.workflow_execution` (Workflow-Specific)

**Type**: `Id` within `WorkflowTaskMetadata` JSONB - References `workflow_execution.id`

**Purpose**: Direct link to workflow orchestration state.

**Provides access to**:
- Task graph structure
- Workflow variables
- Current/completed/failed task lists
- Workflow-specific metadata

**Example SQL**:
```sql
-- Find all tasks in a specific workflow
SELECT * FROM attune.execution 
WHERE workflow_task->>'workflow_execution' = '50';
```

## Workflow Task Execution Structure

When a workflow executes, three types of records are created:

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Parent Execution (the workflow itself as an execution)  │
├─────────────────────────────────────────────────────────────┤
│ id: 100                                                     │
│ action_ref: "my_pack.my_workflow"                          │
│ parent: None (or outer workflow ID if nested)              │
│ workflow_task: None                                         │
│ status: running                                             │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │
                            │ references (execution field)
                            │
┌─────────────────────────────────────────────────────────────┐
│ 2. Workflow Execution Record (orchestration state)         │
├─────────────────────────────────────────────────────────────┤
│ id: 50                                                      │
│ execution: 100          ← points to parent execution       │
│ workflow_def: 10                                            │
│ task_graph: {...}                                           │
│ variables: {...}                                            │
│ current_tasks: ["send_email", "process_data"]              │
│ completed_tasks: []                                         │
│ failed_tasks: []                                            │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │
                            │ references (workflow_execution)
                            │
┌─────────────────────────────────────────────────────────────┐
│ 3. Task Execution (one per workflow task)                  │
├─────────────────────────────────────────────────────────────┤
│ id: 101                                                     │
│ action_ref: "my_pack.send_email"                           │
│ parent: 100             ← points to workflow execution     │
│ workflow_task: {                                            │
│   workflow_execution: 50  ← points to workflow_execution   │
│   task_name: "send_email",                                 │
│   task_index: null,                                         │
│   retry_count: 0,                                           │
│   max_retries: 3,                                           │
│   ...                                                       │
│ }                                                           │
│ status: running                                             │
└─────────────────────────────────────────────────────────────┘
```

## Relationship Diagram

```
┌─────────────────────┐
│  Task Execution     │
│  (id: 101)          │
│                     │
│  parent: 100        │──────┐
│                     │      │
│  workflow_task: {   │      │
│    workflow_exec: 50│──┐   │
│  }                  │  │   │
└─────────────────────┘  │   │
                         │   │
                         │   ▼
                         │  ┌─────────────────────┐
                         │  │ Parent Execution    │
                         │  │ (id: 100)           │
                         │  │ [The Workflow]      │
                         │  └─────────────────────┘
                         │           ▲
                         │           │
                         │           │ execution: 100
                         │           │
                         │  ┌─────────────────────┐
                         └─▶│ Workflow Execution  │
                            │ (id: 50)            │
                            │ [Orchestration]     │
                            └─────────────────────┘
```

**Key**: Both `parent` and `workflow_task.workflow_execution` ultimately reference the same workflow, but serve different query patterns.

## Why Both Fields Are Needed

### ✅ Reason 1: `parent` is Generic

The `parent` field is used for **all types** of execution hierarchies, not just workflows:

**Example 1: Action spawning child actions**
```rust
// Parent action execution
let parent_exec = create_execution("my_pack.parent_action").await?;

// Child action executions (NOT workflow tasks)
let child1 = CreateExecutionInput {
    action_ref: "my_pack.child_action_1".to_string(),
    parent: Some(parent_exec.id),
    workflow_task: None,  // Not a workflow task!
    ...
};
```

**Example 2: Nested workflows**
```rust
// Outer workflow execution
let outer_workflow = create_workflow("outer_workflow").await?;

// Inner workflow execution (nested)
let inner_workflow = CreateExecutionInput {
    action_ref: "inner_workflow".to_string(),
    parent: Some(outer_workflow.id),
    workflow_task: None,  // This is a workflow, not a task
    ...
};
```

### ✅ Reason 2: Workflow-Specific State is Separate

The `workflow_execution` table contains orchestration state that doesn't belong in the main `execution` record:

- **Task graph**: Directed acyclic graph of task dependencies
- **Workflow variables**: Scoped variable context
- **Task tracking**: current_tasks, completed_tasks, failed_tasks arrays
- **Workflow metadata**: pause_reason, error_message, etc.

Direct access via `workflow_task.workflow_execution` avoids JOINs.

### ✅ Reason 3: Query Efficiency

**Without direct `workflow_execution` reference**, finding workflow state requires:

```sql
-- BAD: Two JOINs required
SELECT we.* 
FROM attune.execution task
JOIN attune.execution parent ON task.parent = parent.id
JOIN attune.workflow_execution we ON we.execution = parent.id
WHERE task.id = 101;
```

**With direct reference**:
```sql
-- GOOD: Single lookup via JSONB
SELECT we.*
FROM attune.workflow_execution we
WHERE we.id = (
    SELECT (workflow_task->>'workflow_execution')::bigint 
    FROM attune.execution 
    WHERE id = 101
);
```

### ✅ Reason 4: Clear Semantics

- `parent` = "What execution spawned me?"
- `workflow_task.workflow_execution` = "What workflow orchestration state do I belong to?"

These are related but semantically different questions.

## Use Cases and Query Patterns

### Use Case 1: Generic Execution Tree Traversal

```rust
// Get ALL child executions (workflow tasks, child actions, anything)
async fn get_children(pool: &PgPool, parent_id: Id) -> Result<Vec<Execution>> {
    sqlx::query_as::<_, Execution>(
        "SELECT * FROM attune.execution WHERE parent = $1"
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

// Works for workflows, actions, any execution type
let all_children = get_children(&pool, parent_exec_id).await?;
```

### Use Case 2: Workflow Task Queries

```rust
// Get all tasks for a workflow execution
let tasks = ExecutionRepository::find_by_workflow_execution(
    &pool, 
    workflow_execution_id
).await?;

// Implementation uses direct JSONB query:
// WHERE workflow_task->>'workflow_execution' = $1
```

### Use Case 3: Workflow State Access

```rust
// From a task execution, get the workflow state
async fn get_workflow_state(
    pool: &PgPool, 
    task_exec: &Execution
) -> Result<Option<WorkflowExecution>> {
    if let Some(wt) = &task_exec.workflow_task {
        let workflow_exec = WorkflowExecutionRepository::find_by_id(
            pool, 
            wt.workflow_execution
        ).await?;
        Ok(Some(workflow_exec))
    } else {
        Ok(None)
    }
}

// Without direct link, would need to:
// 1. Get parent execution via task_exec.parent
// 2. Find workflow_execution WHERE execution = parent
```

### Use Case 4: Hierarchical Display

```rust
// Display execution tree with proper indentation
async fn display_execution_tree(pool: &PgPool, root_id: Id, indent: usize) {
    let exec = ExecutionRepository::find_by_id(pool, root_id).await.unwrap();
    println!("{:indent$}├─ {} ({})", "", exec.action_ref, exec.status, indent = indent);
    
    // Get children using generic parent relationship
    let children = sqlx::query_as::<_, Execution>(
        "SELECT * FROM attune.execution WHERE parent = $1"
    )
    .bind(root_id)
    .fetch_all(pool)
    .await
    .unwrap();
    
    for child in children {
        display_execution_tree(pool, child.id, indent + 2).await;
    }
}
```

## The Redundancy Trade-off

### For Workflow Tasks: Yes, There's Redundancy

```
task.parent 
  → parent_execution (id: 100)
    ← workflow_execution.execution

task.workflow_task.workflow_execution 
  → workflow_execution (id: 50)
    → parent_execution (id: 100)
```

Both ultimately point to the same workflow, just through different paths.

### Why This Is Acceptable

1. **Performance**: Direct link avoids JOINs (PostgreSQL JSONB is fast)
2. **Clarity**: Explicit workflow relationship vs generic parent relationship
3. **Flexibility**: `parent` can be used for non-workflow patterns
4. **Consistency**: All executions use `parent` the same way

### Alternatives Considered

#### ❌ Alternative 1: Remove `workflow_execution` from metadata

**Problem**: Forces 2-JOIN queries to access workflow state
```sql
-- Every workflow task query becomes complex
SELECT we.* 
FROM attune.execution task
JOIN attune.execution parent ON task.parent = parent.id
JOIN attune.workflow_execution we ON we.execution = parent.id
WHERE task.workflow_task IS NOT NULL;
```

#### ❌ Alternative 2: Remove `parent` for workflow tasks

**Problem**: Breaks generic execution tree queries
```sql
-- Would need complex COALESCE logic
SELECT * FROM attune.execution 
WHERE parent = $1 
   OR (workflow_task IS NOT NULL 
       AND (workflow_task->>'parent_execution')::bigint = $1);
```

#### ✅ Current Approach: Keep Both

Small redundancy in exchange for:
- Simple generic queries via `parent`
- Efficient workflow queries via `workflow_task.workflow_execution`
- Clear separation of concerns

## Validation and Best Practices

### Validation Logic (Optional)

For data integrity, you could validate consistency:

```rust
async fn validate_workflow_task_consistency(
    pool: &PgPool,
    task_exec: &Execution
) -> Result<()> {
    if let Some(wt) = &task_exec.workflow_task {
        // Get workflow_execution record
        let workflow_exec = WorkflowExecutionRepository::find_by_id(
            pool, 
            wt.workflow_execution
        ).await?;
        
        // Ensure parent matches workflow_execution.execution
        if task_exec.parent != Some(workflow_exec.execution) {
            return Err(Error::validation(format!(
                "Inconsistent parent: task.parent={:?}, workflow_exec.execution={}",
                task_exec.parent, workflow_exec.execution
            )));
        }
    }
    Ok(())
}
```

### Helper Methods (Recommended)

Add convenience methods to the `Execution` model:

```rust
impl Execution {
    /// Check if this execution is a workflow task
    pub fn is_workflow_task(&self) -> bool {
        self.workflow_task.is_some()
    }
    
    /// Get the workflow_execution record if this is a workflow task
    pub async fn get_workflow_execution(
        &self, 
        pool: &PgPool
    ) -> Result<Option<WorkflowExecution>> {
        if let Some(wt) = &self.workflow_task {
            let we = WorkflowExecutionRepository::find_by_id(pool, wt.workflow_execution).await?;
            Ok(Some(we))
        } else {
            Ok(None)
        }
    }
    
    /// Get the parent execution
    pub async fn get_parent(&self, pool: &PgPool) -> Result<Option<Execution>> {
        if let Some(parent_id) = self.parent {
            ExecutionRepository::find_by_id(pool, parent_id).await
        } else {
            Ok(None)
        }
    }
    
    /// Get all child executions (generic, works for any execution type)
    pub async fn get_children(&self, pool: &PgPool) -> Result<Vec<Execution>> {
        sqlx::query_as::<_, Execution>(
            "SELECT * FROM attune.execution WHERE parent = $1 ORDER BY created"
        )
        .bind(self.id)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
    }
}
```

## Summary

### Key Takeaways

1. **`parent`** is a generic field for ALL execution hierarchies (workflows, child actions, nested workflows)

2. **`workflow_task.workflow_execution`** is a workflow-specific optimization for direct access to orchestration state

3. **Both are needed** because:
   - `parent` must remain generic for non-workflow use cases
   - Direct workflow_execution link avoids expensive JOINs
   - Different query patterns benefit from each approach

4. **The redundancy is acceptable** because:
   - It's limited to workflow tasks only (not all executions)
   - Performance gain from avoiding JOINs
   - Clearer semantics for different use cases

### When to Use Which

| Scenario | Use `parent` | Use `workflow_task.workflow_execution` |
|----------|--------------|----------------------------------------|
| Get child executions (any type) | ✅ | ❌ |
| Build execution tree | ✅ | ❌ |
| Find all workflow tasks | ❌ | ✅ |
| Access workflow state | ❌ | ✅ |
| Non-workflow parent-child | ✅ | N/A |

### Design Principle

**Separation of concerns**:
- `parent`: Structural relationship (execution hierarchy)
- `workflow_task.workflow_execution`: Semantic relationship (workflow orchestration)

This follows the principle that a workflow task has TWO relationships:
1. As a child in the execution tree (`parent`)
2. As a task in a workflow (`workflow_task.workflow_execution`)

Both are valid, serve different purposes, and should coexist.

## References

- **Migration**: `migrations/20260127212500_consolidate_workflow_task_execution.sql`
- **Models**: `crates/common/src/models.rs` (Execution, WorkflowTaskMetadata)
- **Repositories**: `crates/common/src/repositories/execution.rs`
- **Workflow Coordinator**: `crates/executor/src/workflow/coordinator.rs`
