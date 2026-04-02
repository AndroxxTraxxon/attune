# Executor HA Horizontal Scaling Plan

## Overview

This plan describes the changes required to make the Attune executor service safe to run with multiple replicas. The current implementation already uses RabbitMQ competing consumers for most work distribution, but several correctness-critical parts of the executor still rely on process-local memory or non-atomic database updates. Those assumptions make the service unsafe under horizontal scaling, message replay, or partial failure.

The goal of this plan is to make executor behavior correct under:

- Multiple executor replicas running concurrently
- Message redelivery from RabbitMQ
- Replica crash/restart during scheduling or workflow advancement
- Background recovery loops running on more than one replica

## Problem Summary

The current executor has five HA blockers:

1. **Concurrency/FIFO control is process-local.**
   `ExecutionQueueManager` stores active slots and waiting queues in memory. That means one replica can admit work while another replica receives the completion notification, causing slot release and queue advancement to fail.

2. **Execution scheduling has no atomic claim step.**
   The scheduler reads an execution in `requested`, does policy checks and worker selection, then updates it later. Two replicas can both observe the same row as schedulable and both dispatch it.

3. **Workflow orchestration is not serialized.**
   Workflow start and workflow advancement perform read-check-create-update sequences without a distributed lock or optimistic version check, so duplicate successor tasks can be created.

4. **Event/enforcement/inquiry handlers are not idempotent.**
   Duplicate message delivery can create duplicate enforcements, duplicate executions, duplicate workflow starts, and duplicate inquiries.

5. **Timeout and DLQ handlers use non-conditional updates.**
   Recovery loops can overwrite newer worker-owned state if the execution changes between the initial read and the final update.

## Goals

- Make execution scheduling single-winner under multiple executor replicas
- Make policy-based concurrency control and FIFO queueing shared across replicas
- Make workflow start and workflow advancement idempotent and serialized
- Make duplicate message delivery safe for executor-owned entities
- Make recovery loops safe to run on every replica

## Non-Goals

- Re-architecting RabbitMQ usage across the whole platform
- Replacing PostgreSQL with a dedicated distributed lock service
- Solving general worker autoscaling or Kubernetes deployment concerns in this phase
- Reworking unrelated executor features like retry policy design unless needed for HA correctness

## Design Principles

### Database is the source of truth

All coordination state that affects correctness must live in PostgreSQL, not in executor memory. In-memory caches and metrics are fine as optimization or observability layers, but correctness cannot depend on them.

### State transitions must be compare-and-swap

Any executor action that changes ownership or lifecycle state must use an atomic update that verifies the prior state in the same statement or transaction.

### Handlers must be idempotent on domain keys

RabbitMQ gives at-least-once delivery semantics. The executor must therefore tolerate duplicate delivery even when the same message is processed more than once or by different replicas.

### Workflow orchestration must be serialized per workflow

A workflow execution should have exactly one active mutator at a time when evaluating transitions and dispatching successor tasks.

## Proposed Implementation Phases

## Current Status

As of the current implementation state:

- Phase 1 is substantially implemented.
- Phases 2, 3, 4, and 5 are implemented.

Completed so far:

- Atomic `requested -> scheduling` claim support was added in `ExecutionRepository`.
- Scheduler state transitions for regular action dispatch were converted to conditional/CAS-style updates.
- Redelivered `execution.requested` messages for stale `scheduling` rows are now retried/reclaimed instead of being silently acknowledged away.
- Shared concurrency/FIFO coordination now uses durable PostgreSQL admission tables for action/group slot ownership and queued execution ordering.
- `ExecutionQueueManager` now acts as a thin API-compatible facade over the DB-backed admission path when constructed with a pool.
- Slot release, queued removal, and rollback/restore flows now operate against shared DB state rather than process-local memory.
- `queue_stats` remains derived telemetry, but it is now refreshed transactionally from the shared admission state.
- Workflow start is now idempotent at the parent workflow state level via `workflow_execution(execution)` uniqueness plus repository create-or-get behavior.
- Workflow advancement now runs under a per-workflow PostgreSQL advisory lock, row-locks `workflow_execution` with `SELECT ... FOR UPDATE`, and performs serialized mutation inside an explicit SQL transaction.
- Durable workflow child dispatch dedupe is now enforced with the `workflow_task_dispatch` coordination table and repository create-or-get helpers.
- `execution` and `enforcement` were switched from Timescale hypertables back to normal PostgreSQL tables to remove HA/idempotency friction around foreign keys and unique constraints. `event` remains a hypertable, and history tables remain Timescale-backed.
- Direct uniqueness/idempotency invariants were added for `enforcement(rule, event)`, top-level `execution(enforcement)`, and `inquiry(execution)`.
- Event, enforcement, and inquiry handlers were updated to use create-or-get flows and conditional status transitions so duplicate delivery becomes safe.
- Timeout and DLQ recovery loops now use conditional state transitions and only emit side effects when the guarded update succeeds.

Partially complete / still open:

- HA-focused integration and failure-injection coverage still needs to be expanded around the new invariants and recovery behavior.
- The new migrations and DB-backed FIFO tests still need end-to-end validation against a real Postgres/Timescale environment.

## Phase 1: Atomic Execution Claiming

### Objective

Ensure only one executor replica can claim a `requested` execution for scheduling.

### Changes

**`crates/common/src/repositories/execution.rs`**

- Add a repository method for atomic status transition, for example:
  - `claim_for_scheduling(id, expected_status, executor_id) -> Option<Execution>`
  - or a more general compare-and-swap helper
- Implement the claim as a single `UPDATE ... WHERE id = $1 AND status = 'requested' RETURNING ...`
- Optionally persist the claiming replica identity in `execution.executor` for debugging and traceability

**`crates/executor/src/scheduler.rs`**

- Claim the execution before policy enforcement, worker selection, workflow start, or any side effects
- Use `scheduling` as the claimed intermediate state
- If the claim returns no row, treat the execution as already claimed/handled and acknowledge the message
- Convert all later scheduler writes to conditional transitions from claimed state

### Success Criteria

- Two schedulers racing on the same execution cannot both dispatch it
- Redelivered `execution.requested` messages become harmless no-ops after the first successful claim

### Status

Implemented for regular action scheduling, with additional stale-claim recovery for redelivered `execution.requested` messages. The remaining gap for this area is broader integration with the still-pending shared admission/queueing work in Phase 2.

## Phase 2: Shared Concurrency Control and FIFO Queueing

### Objective

Replace the in-memory `ExecutionQueueManager` as the source of truth for concurrency slots and waiting order.

### Changes

**New schema**

Add database-backed coordination tables, likely along these lines:

- `execution_admission_slot`
  - active slot ownership for an action/group key
- `execution_admission_queue`
  - ordered waiting executions for an action/group key

Alternative naming is fine, but the design needs to support:

- Action-level concurrency limits
- Parameter-group concurrency keys
- FIFO ordering within each action/group
- Deterministic advancement when a slot is released

**`crates/executor/src/policy_enforcer.rs`**

- Replace `ExecutionQueueManager` slot acquisition with DB-backed admission logic
- Keep existing policy semantics:
  - `enqueue`
  - `cancel`
  - parameter-based concurrency grouping

**`crates/executor/src/completion_listener.rs`**

- Release the shared slot transactionally on completion
- Select and wake the next queued execution in the same transaction
- Republish only after the DB state is committed

**`crates/executor/src/queue_manager.rs`**

- Either remove it entirely or reduce it to a thin adapter over DB-backed coordination
- Do not keep active slot ownership in process-local `DashMap`

**`crates/common/src/repositories/queue_stats.rs`**

- Keep `queue_stats` as derived telemetry only
- Do not rely on it for correctness

### Success Criteria

- Completion processed by a different executor replica still releases the correct slot
- FIFO ordering holds across multiple executor replicas
- Restarting an executor does not lose queue ownership state

### Status

Implemented.

Completed:

- Shared admission state now lives in PostgreSQL via durable action/group queue rows and execution entry rows.
- Action-level concurrency limits and parameter-group concurrency keys are enforced against that shared admission state.
- FIFO ordering is determined by durable queued-entry order rather than process-local memory.
- Completion-time slot release promotes the next queued execution inside the same DB transaction.
- Rollback helpers can restore released slots or removed queued entries if republish/cleanup fails after the DB mutation.
- `ExecutionQueueManager` remains as a facade for the existing scheduler/policy code paths, but it no longer acts as the correctness source of truth when running with a DB pool.

## Phase 3: Workflow Start Idempotency and Serialized Advancement

### Objective

Ensure workflow orchestration is safe under concurrent replicas and duplicate completion messages.

### Changes

**Migration**

Add a uniqueness constraint to guarantee one workflow state row per parent execution:

```sql
ALTER TABLE workflow_execution
ADD CONSTRAINT uq_workflow_execution_execution UNIQUE (execution);
```

**`crates/executor/src/scheduler.rs`**

- Change workflow start to be idempotent:
  - either `INSERT ... ON CONFLICT ...`
  - or claim parent execution first and only create workflow state once
- When advancing a workflow:
  - wrap read/decide/write logic in a transaction
  - lock the `workflow_execution` row with `SELECT ... FOR UPDATE`
  - or use advisory locks keyed by workflow execution id

**Successor dispatch dedupe**

Add a durable uniqueness guarantee for child task dispatch, for example:

- one unique key for regular workflow tasks:
  - `(workflow_execution_id, task_name, task_index IS NULL)`
- one unique key for `with_items` children:
  - `(workflow_execution_id, task_name, task_index)`

This may be implemented with explicit columns or a dedupe table if indexing the current JSONB layout is awkward.

**Repository support**

- Add workflow repository helpers that support transactional locking and conditional updates
- Avoid blind overwrite of `completed_tasks`, `failed_tasks`, and `variables` outside a serialized transaction

### Success Criteria

- Starting the same workflow twice cannot create two `workflow_execution` rows
- Duplicate `execution.completed` delivery for a workflow child cannot create duplicate successor executions
- Two executor replicas cannot concurrently mutate the same workflow state

### Status

Implemented.

Completed:

- `workflow_execution(execution)` uniqueness is part of the workflow schema and workflow start uses create-or-get semantics.
- Workflow parent executions are claimed before orchestration starts.
- Workflow advancement now runs under a per-workflow PostgreSQL advisory lock held on the same DB connection that performs the serialized advancement work.
- The serialized workflow path is wrapped in an explicit SQL transaction.
- `workflow_execution` is row-locked with `SELECT ... FOR UPDATE` before mutation.
- Successor/child dispatch dedupe is enforced with the durable `workflow_task_dispatch` table keyed by `(workflow_execution, task_name, COALESCE(task_index, -1))`.
- Child `ExecutionRequested` messages are staged and published only after the workflow transaction commits.

## Phase 4: Idempotent Event, Enforcement, and Inquiry Handling

### Objective

Make duplicate delivery safe for all earlier and later executor-owned side effects.

### Changes

**Enforcement dedupe**

Add a uniqueness rule so one event/rule pair produces at most one enforcement when `event` is present.

Example:

```sql
CREATE UNIQUE INDEX uq_enforcement_rule_event
ON enforcement(rule, event)
WHERE event IS NOT NULL;
```

**`crates/executor/src/event_processor.rs`**

- Use upsert-or-ignore semantics for enforcement creation
- Treat uniqueness conflicts as idempotent success

**`crates/executor/src/enforcement_processor.rs`**

- Check current enforcement status before creating an execution
- Add a durable relation that prevents an enforcement from creating more than one top-level execution
- Options:
  - unique partial index on `execution(enforcement)` for top-level executions
  - or a separate coordination record

**Inquiry dedupe**

- Prevent duplicate inquiry creation per execution result/completion path
- Add a unique invariant such as one active inquiry per execution, if that matches product semantics
- Update completion handling to tolerate duplicate `execution.completed`

### Success Criteria

- Duplicate `event.created` does not create duplicate enforcements
- Duplicate `enforcement.created` does not create duplicate executions
- Duplicate completion handling does not create duplicate inquiries

### Status

Implemented.

Completed:

- `enforcement(rule, event)` uniqueness is enforced directly with a partial unique index when both keys are present.
- Top-level execution creation is deduped with a unique invariant on `execution(enforcement)` where `parent IS NULL`.
- Inquiry creation is deduped with a unique invariant on `inquiry(execution)`.
- `event_processor` now uses create-or-get enforcement handling and only republishes when the persisted enforcement still needs processing.
- `enforcement_processor` now skips duplicate non-`created` enforcements, creates or reuses the top-level execution, and conditionally resolves enforcement state.
- `inquiry_handler` now uses create-or-get inquiry handling and only emits `InquiryCreated` when the inquiry was actually created.

## Phase 5: Safe Recovery Loops

### Objective

Make timeout and DLQ processing safe under races and multiple replicas.

### Changes

**`crates/executor/src/timeout_monitor.rs`**

- Replace unconditional updates with conditional state transitions:
  - `UPDATE execution SET ... WHERE id = $1 AND status = 'scheduled' ... RETURNING ...`
- Only publish completion side effects when a row was actually updated
- Consider including `updated < cutoff` in the same update statement

**`crates/executor/src/dead_letter_handler.rs`**

- Change failure transition to conditional update based on current state
- Do not overwrite executions that have already moved to `running` or terminal state
- Only emit side effects when the row transition succeeded

**`crates/executor/src/service.rs`**

- It is acceptable for these loops to run on every replica once updates are conditional
- Optional future optimization: leader election for janitor loops to reduce duplicate scans and log noise

### Success Criteria

- Timeout monitor cannot fail an execution that has already moved to `running`
- DLQ handler cannot overwrite newer state
- Running multiple timeout monitors produces no conflicting state transitions

### Status

Implemented.

Completed:

- Timeout failure now uses a conditional transition that only succeeds when the execution is still `scheduled` and still older than the timeout cutoff.
- Timeout-driven completion side effects are only published when that guarded update succeeds.
- DLQ handling now treats messages as stale unless the execution is still exactly `scheduled`.
- DLQ failure transitions now use conditional status updates and no longer overwrite newer `running` or terminal state.

## Testing Plan

Add focused HA tests after the repository and scheduler primitives are in place.

### Repository tests

- Compare-and-swap execution claim succeeds exactly once
- Conditional timeout/DLQ transition updates exactly one row or zero rows as expected
- Workflow uniqueness constraint prevents duplicate workflow state rows

### Executor integration tests

- Two scheduler instances processing the same `execution.requested` message only dispatch once
- Completion consumed by a different executor replica still advances the shared queue
- Duplicate workflow child completion does not create duplicate successor tasks
- Duplicate `event.created` and `enforcement.created` messages do not create duplicate downstream records

### Failure-injection tests

- Executor crashes after claiming but before publish
- Executor crashes after slot release but before republish
- Duplicate `execution.completed` delivery after successful workflow advancement

## Recommended Execution Order for Next Session

1. Add more HA-focused integration tests for duplicate delivery, cross-replica completion, and recovery rollback paths
2. Add failure-injection tests for crash/replay scenarios around `scheduling` reclaim, workflow advancement, and post-commit publish paths
3. Validate the new migrations and DB-backed FIFO behavior end-to-end against a real Postgres/Timescale environment
4. Consider a small follow-up cleanup pass to reduce or remove the in-memory fallback code in `ExecutionQueueManager` once the DB path is fully baked

## Expected Outcome

After this plan is implemented, the executor should be able to scale horizontally without relying on singleton behavior. Multiple executor replicas should be able to process work concurrently while preserving:

- exactly-once scheduling semantics at the execution state level
- shared concurrency limits and FIFO behavior
- correct workflow orchestration
- safe replay handling
- safe recovery behavior during failures and redelivery

At the current state, the core executor HA phases are implemented. The remaining work is confidence-building: failure-injection coverage, multi-replica integration testing, and end-to-end migration validation in a live database environment.
