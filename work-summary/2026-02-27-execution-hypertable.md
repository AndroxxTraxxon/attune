# Execution Table → TimescaleDB Hypertable Conversion

**Date**: 2026-02-27
**Scope**: Database migration, Rust code fixes, AGENTS.md updates

## Summary

Converted the `execution` table from a regular PostgreSQL table to a TimescaleDB hypertable partitioned on `created` (1-day chunks), consistent with the existing `event` and `enforcement` hypertable conversions. This enables automatic time-based partitioning, compression, and retention for execution data.

## Key Design Decisions

- **`updated` column preserved**: Unlike `event` (immutable) and `enforcement` (single update), executions are updated ~4 times during their lifecycle. The `updated` column and its BEFORE UPDATE trigger are kept because the timeout monitor and UI depend on them.
- **`execution_history` preserved**: The execution_history hypertable tracks field-level diffs which remain valuable for a mutable table. Its continuous aggregates (`execution_status_hourly`, `execution_throughput_hourly`) are unchanged.
- **7-day compression window is safe**: Executions complete within at most ~1 day, so all updates finish well before compression kicks in.
- **New `execution_volume_hourly` continuous aggregate**: Queries the execution hypertable directly (like `event_volume_hourly` queries event), providing belt-and-suspenders volume monitoring alongside the history-based aggregates.

## Changes

### New Migration: `migrations/20250101000010_execution_hypertable.sql`
- Drops all FK constraints referencing `execution` (inquiry, workflow_execution, self-references, action, executor, workflow_def)
- Changes PK from `(id)` to `(id, created)` (TimescaleDB requirement)
- Converts to hypertable with `create_hypertable('execution', 'created', chunk_time_interval => '1 day')`
- Adds compression policy (segmented by `action_ref`, after 7 days)
- Adds 90-day retention policy
- Adds `execution_volume_hourly` continuous aggregate with 30-minute refresh policy

### Rust Code Fixes
- **`crates/executor/src/timeout_monitor.rs`**: Replaced `SELECT * FROM execution` with explicit column list. `SELECT *` on hypertables is fragile — the execution table has DB-only columns such as `workflow_def` that are not present in the Rust `Execution` model.
- **`crates/api/tests/sse_execution_stream_tests.rs`**: Fixed references to non-existent `start_time` and `end_time` columns (replaced with `updated = NOW()`).
- **`crates/common/src/repositories/analytics.rs`**: Added `ExecutionVolumeBucket` struct and `execution_volume_hourly` / `execution_volume_hourly_by_action` repository methods for the new continuous aggregate.

### AGENTS.md Updates
- Added **Execution Table (TimescaleDB Hypertable)** documentation
- Updated FK ON DELETE Policy to reflect execution as hypertable
- Updated Nullable FK Fields to list all dropped FK constraints
- Updated table count (still 20) and migration count (9 → 10)
- Updated continuous aggregate count (5 → 6)
- Updated development status to include execution hypertable
- Added pitfall #19: never use `SELECT *` on hypertable-backed models
- Added pitfall #20: execution/event/enforcement cannot be FK targets

## FK Constraints Dropped

| Source Column | Target | Disposition |
|---|---|---|
| `inquiry.execution` | `execution(id)` | Column kept as plain BIGINT |
| `workflow_execution.execution` | `execution(id)` | Column kept as plain BIGINT |
| `execution.parent` | `execution(id)` | Self-ref, column kept |
| `execution.original_execution` | `execution(id)` | Self-ref, column kept |
| `execution.workflow_def` | `workflow_definition(id)` | Column kept |
| `execution.action` | `action(id)` | Column kept |
| `execution.executor` | `identity(id)` | Column kept |
| `execution.enforcement` | `enforcement(id)` | Already dropped in migration 000009 |

## Verification

- `cargo check --all-targets --workspace`: Zero warnings
- `cargo test --workspace --lib`: All 90 unit tests pass
- Integration test failures are pre-existing (missing `attune_test` database), unrelated to these changes
