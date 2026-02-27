# Event & Enforcement Tables → TimescaleDB Hypertable Migration

**Date:** 2026-02  
**Scope:** Database migrations, Rust models/repositories/API, Web UI

## Summary

Converted the `event` and `enforcement` tables from regular PostgreSQL tables to TimescaleDB hypertables, and removed the now-unnecessary `event_history` and `enforcement_history` tables.

- **Events** are immutable after insert (never updated), so a separate change-tracking history table added no value.
- **Enforcements** are updated exactly once (~1 second after creation, to set status from `created` to `processed` or `disabled`), well before the 7-day compression window. A history table tracking one deterministic status change per row was unnecessary overhead.

Both tables now benefit from automatic time-based partitioning, compression, and retention directly.

## Motivation

The `event_history` and `enforcement_history` hypertables were created alongside `execution_history` and `worker_history` to track field-level changes. However:

- **Events** are never modified after creation — no code path in the API, executor, worker, or sensor ever updates an event row. The history trigger was recording INSERT operations only, duplicating data already in the `event` table.
- **Enforcements** undergo a single, predictable status transition (created → processed/disabled) within ~1 second. The history table recorded one INSERT and one UPDATE per enforcement — the INSERT was redundant, and the UPDATE only changed `status`. The new `resolved_at` column captures this lifecycle directly on the enforcement row itself.

## Changes

### Database Migrations

**`000004_trigger_sensor_event_rule.sql`**:
- Removed `updated` column from the `event` table
- Removed `update_event_updated` trigger
- Replaced `updated` column with `resolved_at TIMESTAMPTZ` (nullable) on the `enforcement` table
- Removed `update_enforcement_updated` trigger
- Updated column comments for enforcement (status lifecycle, resolved_at semantics)

**`000008_notify_triggers.sql`**:
- Updated enforcement NOTIFY trigger payloads: `updated` → `resolved_at`

**`000009_timescaledb_history.sql`**:
- Removed `event_history` table, all its indexes, trigger function, trigger, compression and retention policies
- Removed `enforcement_history` table, all its indexes, trigger function, trigger, compression and retention policies
- Added hypertable conversion for `event` table:
  - Dropped FK constraint from `enforcement.event` → `event(id)`
  - Changed PK from `(id)` to `(id, created)`
  - Converted to hypertable with 1-day chunk interval
  - Compression segmented by `trigger_ref`, retention 90 days
- Added hypertable conversion for `enforcement` table:
  - Dropped FK constraint from `execution.enforcement` → `enforcement(id)`
  - Changed PK from `(id)` to `(id, created)`
  - Converted to hypertable with 1-day chunk interval
  - Compression segmented by `rule_ref`, retention 90 days
- Updated `event_volume_hourly` continuous aggregate to query `event` table directly
- Updated `enforcement_volume_hourly` continuous aggregate to query `enforcement` table directly

### Rust Code — Events

**`crates/common/src/models.rs`**:
- Removed `updated` field from `Event` struct
- Removed `Event` variant from `HistoryEntityType` enum

**`crates/common/src/repositories/event.rs`**:
- Removed `UpdateEventInput` struct and `Update` trait implementation for `EventRepository`
- Updated all SELECT queries to remove `updated` column

**`crates/api/src/dto/event.rs`**:
- Removed `updated` field from `EventResponse`

**`crates/common/tests/event_repository_tests.rs`**:
- Removed all update tests
- Renamed timestamp test to `test_event_created_timestamp_auto_set`
- Updated `test_delete_event_enforcement_retains_event_id` (FK dropped, so enforcement.event is now a dangling reference after event deletion)

### Rust Code — Enforcements

**`crates/common/src/models.rs`**:
- Replaced `updated: DateTime<Utc>` with `resolved_at: Option<DateTime<Utc>>` on `Enforcement` struct
- Removed `Enforcement` variant from `HistoryEntityType` enum
- Updated `FromStr`, `Display`, and `table_name()` implementations (only `Execution` and `Worker` remain)

**`crates/common/src/repositories/event.rs`**:
- Added `resolved_at: Option<DateTime<Utc>>` to `UpdateEnforcementInput`
- Updated all SELECT queries to use `resolved_at` instead of `updated`
- Update query no longer appends `, updated = NOW()` — `resolved_at` is set explicitly by the caller

**`crates/api/src/dto/event.rs`**:
- Replaced `updated` with `resolved_at: Option<DateTime<Utc>>` on `EnforcementResponse`

**`crates/executor/src/enforcement_processor.rs`**:
- Both status update paths (Processed and Disabled) now set `resolved_at: Some(chrono::Utc::now())`
- Updated test mock enforcement struct

**`crates/common/tests/enforcement_repository_tests.rs`**:
- Updated all tests to use `resolved_at` instead of `updated`
- Renamed `test_create_enforcement_with_invalid_event_fails` → `test_create_enforcement_with_nonexistent_event_succeeds` (FK dropped)
- Renamed `test_enforcement_timestamps_auto_managed` → `test_enforcement_resolved_at_lifecycle`
- All `UpdateEnforcementInput` usages now include `resolved_at` field

### Rust Code — History Infrastructure

**`crates/api/src/routes/history.rs`**:
- Removed `get_event_history` and `get_enforcement_history` endpoints
- Removed `/events/{id}/history` and `/enforcements/{id}/history` routes
- Updated doc comments to list only `execution` and `worker`

**`crates/api/src/dto/history.rs`**:
- Updated entity type comment

**`crates/common/src/repositories/entity_history.rs`**:
- Updated tests to remove `Event` and `Enforcement` variant assertions
- Both now correctly fail to parse as `HistoryEntityType`

### Web UI

**`web/src/pages/events/EventDetailPage.tsx`**:
- Removed `EntityHistoryPanel` component

**`web/src/pages/enforcements/EnforcementDetailPage.tsx`**:
- Removed `EntityHistoryPanel` component
- Added `resolved_at` display in Overview card ("Resolved At" field, shows "Pending" when null)
- Added `resolved_at` display in Metadata sidebar

**`web/src/hooks/useHistory.ts`**:
- Removed `"event"` and `"enforcement"` from `HistoryEntityType` union and `pluralMap`
- Removed `useEventHistory` and `useEnforcementHistory` convenience hooks

**`web/src/hooks/useEnforcementStream.ts`**:
- Removed history query invalidation (no more enforcement_history table)

### Documentation

- Updated `AGENTS.md`: table counts (22→20), history entity list, FK policy, enforcement lifecycle (resolved_at), pitfall #17
- Updated `docs/plans/timescaledb-entity-history.md`: removed event_history and enforcement_history from all tables, added notes about both hypertables

## Key Design Decisions

1. **Composite PK `(id, created)` on both tables**: Required by TimescaleDB — the partitioning column must be part of the PK. The `id` column retains its `BIGSERIAL` for unique identification; `created` is added for partitioning.

2. **Dropped FKs targeting hypertables**: TimescaleDB hypertables cannot be the target of foreign key constraints. Affected: `enforcement.event → event(id)` and `execution.enforcement → enforcement(id)`. Both columns remain as plain BIGINT for application-level joins. Since the original FKs were `ON DELETE SET NULL` (soft references), this is a minor change — the columns may now become dangling references if the referenced row is deleted.

3. **`resolved_at` instead of `updated`**: The `updated` column was a generic auto-managed timestamp. The new `resolved_at` column is semantically meaningful — it records specifically when the enforcement was resolved (status transitioned away from `created`). It is `NULL` while the enforcement is pending, making it easy to query for unresolved enforcements. The executor sets it explicitly alongside the status change.

4. **Compression segmentation**: Event table segments by `trigger_ref`, enforcement table segments by `rule_ref` — matching the most common query patterns for each table.

5. **90-day retention for both**: Aligned with execution history retention since events and enforcements are primary operational records in the event-driven pipeline.