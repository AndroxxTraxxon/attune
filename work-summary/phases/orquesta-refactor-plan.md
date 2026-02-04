# Orquesta-Style Workflow Refactoring Plan

## Goal
Refactor the workflow execution engine from a dependency-based DAG model to a transition-based graph traversal model inspired by StackStorm's Orquesta engine. This will simplify the code and naturally support workflow cycles.

## Current Problems
1. **Over-engineered**: Computing dependencies, levels, and topological sort that we never actually use
2. **Not using transitions**: We parse `next` transitions but execute based on dependencies instead
3. **Artificial DAG restriction**: Prevents legitimate use cases like monitoring loops
4. **Polling-based**: Continuously polls for "ready tasks" instead of reacting to completions

## Orquesta Model Benefits
1. **Simpler**: Pure graph traversal following transitions
2. **Event-driven**: Task completions trigger next task scheduling
3. **Naturally supports cycles**: Workflows terminate when transitions stop scheduling tasks
4. **Intuitive**: Follow the `next` arrows in the workflow definition

## Implementation Plan

### Phase 1: Documentation Updates
**Files to modify:**
- `docs/workflow-execution-engine.md`
- `work-summary/TODO.md`

**Changes:**
- [ ] Remove references to DAG and topological sort
- [ ] Document transition-based execution model
- [ ] Add examples of cyclic workflows (monitoring loops)
- [ ] Document join semantics clearly
- [ ] Document workflow termination conditions

### Phase 2: Refactor Graph Module (`crates/executor/src/workflow/graph.rs`)

**Remove:**
- [x] `CircularDependency` error variant (cycles are now valid)
- [x] `NoEntryPoint` error variant (can have workflows with all tasks having inbound edges if manually started)
- [x] `level` field from `TaskNode`
- [x] `execution_order` field from `TaskGraph`
- [x] `compute_levels()` method (not needed)
- [x] Topological sort logic in `From<GraphBuilder> for TaskGraph`

**Keep/Modify:**
- [x] `entry_points` - still useful as default starting tasks
- [x] Renamed `dependencies` to `inbound_edges` - needed for entry point detection and join tracking
- [x] Renamed `dependents` to `outbound_edges` - needed for identifying edges
- [x] `next_tasks()` - **KEY METHOD** - evaluates transitions
- [x] Simplified `compute_dependencies()` to `compute_inbound_edges()` - only tracks inbound edges
- [x] Updated `TaskNode.dependencies` to `TaskNode.inbound_tasks`

**Add:**
- [x] `get_inbound_tasks(&self, task_name: &str) -> Vec<String>` - returns all tasks that can transition to this task
- [x] Documentation explaining that cycles are supported

### Phase 3: Enhance Transition Evaluation

**Files to modify:**
- `crates/executor/src/workflow/graph.rs`

**Changes:**
- [x] `next_tasks()` already returns task names based on success/failure
- [ ] Add support for evaluating `when` conditions (deferred - needs context)
- [ ] Consider returning a struct with task name + transition info instead of just String (deferred)

### Phase 4: Add Join Tracking (`crates/executor/src/workflow/coordinator.rs`)

**Add to WorkflowExecutionState:**
- [x] `scheduled_tasks: HashSet<String>` - tasks scheduled but not yet executing
- [x] `join_state: HashMap<String, HashSet<String>>` - track which predecessors completed for each join task
- [x] Renamed `current_tasks` to `executing_tasks` for clarity

**Add methods:**
- [x] Join checking logic implemented in `on_task_completion()` method
  - Checks if join conditions are met
  - Returns true immediately if no join specified
  - Returns true if join count reached

### Phase 5: Refactor Workflow Coordinator

**Files to modify:**
- `crates/executor/src/workflow/coordinator.rs`

**Major refactor of `WorkflowExecutionHandle::execute()`:**

```rust
// NEW EXECUTION MODEL:
// 1. Schedule entry point tasks
// 2. Wait for task completions
// 3. On completion, evaluate transitions and schedule next tasks
// 4. Terminate when nothing executing and nothing scheduled
```

**Changes:**
- [x] Replaced polling ready_tasks with checking scheduled_tasks
- [x] Start execution by scheduling all entry point tasks
- [x] Removed `graph.ready_tasks()` call
- [x] Added `spawn_task_execution()` method that:
  - Spawns task execution from main loop
- [x] Modified `execute_task_async()` to:
  - Move task from scheduled to executing when starting
  - On completion, evaluate `graph.next_tasks()` 
  - Call `on_task_completion()` to schedule next tasks
  - Handle join state updates
- [x] Updated termination condition:
  - `scheduled_tasks.is_empty() && executing_tasks.is_empty()`

**Specific implementation steps:**

1. [x] Added `spawn_task_execution()` method
2. [x] Added `on_task_completion()` method that evaluates transitions
3. [x] Refactored `execute()` to start with entry points
4. [x] Changed main loop to spawn scheduled tasks and check for completion
5. [x] Updated `execute_task_async()` to call `on_task_completion()` at the end
6. [x] Implemented join barrier logic in `on_task_completion()`

### Phase 6: Update Tests

**Files to modify:**
- `crates/executor/src/workflow/graph.rs` (tests module)
- `crates/executor/src/workflow/coordinator.rs` (tests module)
- Add new test files if needed

**Test cases to add:**
- [x] Simple cycle (task transitions to itself) - test_cycle_support
- [ ] Complex cycle (task A -> B -> C -> A)
- [ ] Cycle with termination condition (monitoring loop that exits)
- [ ] Join with 2 parallel tasks
- [ ] Join with N tasks (where join = 2 of 3)
- [ ] Multiple entry points
- [x] Workflow with no entry points (all tasks have inbound edges) - test_cycle_support covers this
- [x] Task that transitions to multiple next tasks - test_parallel_entry_points covers this

**Test cases to update:**
- [x] Updated existing tests to work with new model
- [x] Removed dependency on circular dependency errors

### Phase 7: Add Cycle Protection

**Safety mechanisms to add:**
- [ ] Workflow execution timeout (max total execution time)
- [ ] Task iteration limit (max times a single task can execute in one workflow)
- [ ] Add to config: `max_workflow_duration_seconds`
- [ ] Add to config: `max_task_iterations_per_workflow`
- [ ] Track iteration count per task in WorkflowExecutionState

### Phase 8: Update Workflow YAML Examples

**Files to create/update:**
- Add example workflows demonstrating cycles
- `docs/examples/monitoring-loop.yaml`
- `docs/examples/retry-with-cycle.yaml`
- `docs/examples/conditional-loop.yaml`

### Phase 9: Final Documentation

**Update:**
- [ ] `README.md` - mention cycle support
- [ ] `docs/workflow-execution-engine.md` - complete rewrite of execution model section
- [ ] `docs/testing-status.md` - add new test requirements
- [ ] `CHANGELOG.md` - document the breaking change

## Testing Strategy

1. **Unit Tests**: Test graph building, transition evaluation, join logic
2. **Integration Tests**: Test full workflow execution with cycles
3. **Manual Testing**: Run example workflows with monitoring loops
4. **Performance Testing**: Ensure cycle detection doesn't cause performance issues

## Migration Notes

**Breaking Changes:**
- Workflows that relied on implicit execution order from levels may behave differently
- Cycles that were previously errors are now valid
- Entry point detection behavior may change slightly

**Backwards Compatibility:**
- All valid DAG workflows should continue to work
- The transition model is more explicit and should be more predictable

## Estimated Effort

- Phase 1 (Docs): 1 hour (DEFERRED)
- Phase 2 (Graph refactor): 2-3 hours ✅ COMPLETE
- Phase 3 (Transition enhancement): 1 hour (PARTIAL - basic implementation done)
- Phase 4 (Join tracking): 1-2 hours ✅ COMPLETE
- Phase 5 (Coordinator refactor): 3-4 hours ✅ COMPLETE
- Phase 6 (Tests): 2-3 hours (PARTIAL - basic tests updated, more needed)
- Phase 7 (Cycle protection): 1-2 hours (DEFERRED - not critical for now)
- Phase 8 (Examples): 1 hour (TODO)
- Phase 9 (Final docs): 1 hour (TODO)

**Total: 13-19 hours**
**Completed so far: ~6-8 hours**

## Success Criteria

1. [x] All existing tests pass ✅
2. [x] New cycle tests pass ✅
3. [ ] Example monitoring loop workflow executes successfully
4. [ ] Documentation is complete and accurate
5. [ ] No performance regression (not tested yet)
6. [x] Code is simpler than before (fewer lines, less complexity) ✅

## Core Implementation Complete ✅

The fundamental refactoring from DAG to transition-based graph traversal is complete:
- Removed all cycle detection code
- Refactored graph building to use inbound/outbound edges
- Implemented transition-based task scheduling
- Added join barrier support
- Updated tests to validate cycle support

Remaining work is primarily documentation and additional examples.

## Implementation Order

Execute phases in order 1-9, completing all tasks in each phase before moving to the next.
Commit after each phase for easy rollback if needed.

---

## Notes from Orquesta Documentation

**Key insights:**
- Tasks are nodes, transitions are edges
- Entry points are tasks with no inbound edges
- Workflow terminates when no tasks running AND no tasks scheduled
- Join creates a barrier - single instance waits for multiple inbound transitions
- Without join, task is invoked multiple times (once per inbound transition)
- Fail-fast: task failure with no transition terminates workflow
- Transitions evaluated in order, first matching transition wins