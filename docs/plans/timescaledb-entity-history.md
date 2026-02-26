# TimescaleDB Entity History Tracking

## Overview

This plan describes the addition of **TimescaleDB-backed history tables** to track field-level changes on key operational entities in Attune. The goal is to provide an immutable audit log and time-series analytics for status transitions and other field changes, without modifying existing operational tables or application code.

## Motivation

Currently, when a field changes on an operational table (e.g., `execution.status` moves from `requested` → `running`), the row is updated in place and only the current state is retained. The `updated` timestamp is bumped, but there is no record of:

- What the previous value was
- When each transition occurred
- How long an entity spent in each state
- Historical trends (e.g., failure rate over time, execution throughput per hour)

This data is essential for operational dashboards, debugging, SLA tracking, and capacity planning.

## Technology Choice: TimescaleDB

[TimescaleDB](https://www.timescale.com/) is a PostgreSQL extension that adds time-series capabilities:

- **Hypertables**: Automatic time-based partitioning (chunks by hour/day/week)
- **Compression**: 10-20x storage reduction on aged-out chunks
- **Retention policies**: Automatic data expiry
- **Continuous aggregates**: Auto-refreshing materialized views for dashboard rollups
- **`time_bucket()` function**: Efficient time-series grouping

It runs as an extension inside the existing PostgreSQL instance — no additional infrastructure.

## Design Decisions

### Separate history tables, not hypertable conversions

The operational tables (`execution`, `worker`, `enforcement`, `event`) will **NOT** be converted to hypertables. Reasons:

1. **UNIQUE constraints on hypertables must include the time partitioning column** — this would break `worker.name UNIQUE`, PK references, etc.
2. **Foreign keys INTO hypertables are not supported** — `execution.parent` self-references `execution(id)`, `enforcement` references `rule`, etc.
3. **UPDATE-heavy tables are a poor fit for hypertables** — hypertables are optimized for append-only INSERT workloads.

Instead, each tracked entity gets a companion `<table>_history` hypertable that receives append-only change records.

### JSONB diff format (not full row snapshots)

Each history row captures only the fields that changed, stored as JSONB:

- **Compact**: A status change is `{"status": "running"}`, not a copy of the entire row including large `result`/`config` JSONB blobs.
- **Schema-decoupled**: Adding a column to the source table requires no changes to the history table structure — only a new `IS DISTINCT FROM` check in the trigger function.
- **Answering "what changed?"**: Directly readable without diffing two full snapshots.

A `changed_fields TEXT[]` column enables efficient partial indexes and GIN-indexed queries for filtering by field name.

### PostgreSQL triggers for population

History rows are written by `AFTER INSERT OR UPDATE OR DELETE` triggers on the operational tables. This ensures:

- Every change is captured regardless of which service (API, executor, worker) made it.
- No Rust application code changes are needed for recording.
- It's impossible to miss a change path.

### Worker heartbeats excluded

`worker.last_heartbeat` is updated frequently by the heartbeat loop and is high-volume/low-value for history purposes. The trigger function explicitly excludes pure heartbeat-only updates. If heartbeat analytics are needed later, a dedicated lightweight table can be added.

## Tracked Entities

| Entity | History Table | `entity_ref` Source | Excluded Fields |
|--------|--------------|---------------------|-----------------|
| `execution` | `execution_history` | `action_ref` | *(none)* |
| `worker` | `worker_history` | `name` | `last_heartbeat` (when sole change) |
| `enforcement` | `enforcement_history` | `rule_ref` | *(none)* |
| `event` | `event_history` | `trigger_ref` | *(none)* |

## Table Schema

All four history tables share the same structure:

```sql
CREATE TABLE <entity>_history (
    time             TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    operation        TEXT           NOT NULL,  -- 'INSERT', 'UPDATE', 'DELETE'
    entity_id        BIGINT         NOT NULL,  -- PK of the source row
    entity_ref       TEXT,                     -- denormalized ref/name for JOIN-free queries
    changed_fields   TEXT[]         NOT NULL DEFAULT '{}',
    old_values       JSONB,                    -- previous values of changed fields
    new_values       JSONB                     -- new values of changed fields
);
```

Column details:

| Column | Purpose |
|--------|---------|
| `time` | Hypertable partitioning dimension; when the change occurred |
| `operation` | `INSERT`, `UPDATE`, or `DELETE` |
| `entity_id` | The source row's `id` (conceptual FK, not enforced on hypertable) |
| `entity_ref` | Denormalized human-readable identifier for efficient filtering |
| `changed_fields` | Array of field names that changed — enables partial indexes and GIN queries |
| `old_values` | JSONB of previous field values (NULL for INSERT) |
| `new_values` | JSONB of new field values (NULL for DELETE) |

## Hypertable Configuration

| History Table | Chunk Interval | Rationale |
|---------------|---------------|-----------|
| `execution_history` | 1 day | Highest expected volume |
| `enforcement_history` | 1 day | Correlated with execution volume |
| `event_history` | 1 day | Can be high volume from active sensors |
| `worker_history` | 7 days | Low volume (status changes are infrequent) |

## Indexes

Each history table gets:

1. **Entity lookup**: `(entity_id, time DESC)` — "show me history for entity X"
2. **Status change filter**: Partial index on `time DESC` where `'status' = ANY(changed_fields)` — "show me all status changes"
3. **Field filter**: GIN index on `changed_fields` — flexible field-based queries
4. **Ref-based lookup**: `(entity_ref, time DESC)` — "show me all execution history for action `core.http_request`"

## Trigger Functions

Each tracked table gets a dedicated trigger function that:

1. On `INSERT`: Records the operation with key initial field values in `new_values`.
2. On `DELETE`: Records the operation with entity identifiers.
3. On `UPDATE`: Checks each mutable field with `IS DISTINCT FROM`. If any fields changed, records the old and new values. If nothing changed, no history row is written.

### Fields tracked per entity

**execution**: `status`, `result`, `executor`, `workflow_task`, `env_vars`

**worker**: `name`, `status`, `capabilities`, `meta`, `host`, `port` (excludes `last_heartbeat` when it's the only change)

**enforcement**: `status`, `payload`

**event**: `config`, `payload`

## Compression Policies

Applied after data leaves the "hot" query window:

| History Table | Compress After | `segmentby` | `orderby` |
|---------------|---------------|-------------|-----------|
| `execution_history` | 7 days | `entity_id` | `time DESC` |
| `worker_history` | 7 days | `entity_id` | `time DESC` |
| `enforcement_history` | 7 days | `entity_id` | `time DESC` |
| `event_history` | 7 days | `entity_id` | `time DESC` |

`segmentby = entity_id` ensures that "show me history for entity X" queries are fast even on compressed chunks.

## Retention Policies

| History Table | Retain For | Rationale |
|---------------|-----------|-----------|
| `execution_history` | 90 days | Primary operational data |
| `enforcement_history` | 90 days | Tied to execution lifecycle |
| `event_history` | 30 days | High volume, less long-term value |
| `worker_history` | 180 days | Low volume, useful for capacity trends |

## Continuous Aggregates (Future)

These are not part of the initial migration but are natural follow-ons:

```sql
-- Execution status transitions per hour (for dashboards)
CREATE MATERIALIZED VIEW execution_status_transitions_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_ref AS action_ref,
    new_values->>'status' AS new_status,
    COUNT(*) AS transition_count
FROM execution_history
WHERE 'status' = ANY(changed_fields)
GROUP BY bucket, entity_ref, new_values->>'status'
WITH NO DATA;

-- Event volume per hour by trigger (for throughput monitoring)
CREATE MATERIALIZED VIEW event_volume_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_ref AS trigger_ref,
    COUNT(*) AS event_count
FROM event_history
WHERE operation = 'INSERT'
GROUP BY bucket, entity_ref
WITH NO DATA;
```

## Infrastructure Changes

### Docker Compose

Change the PostgreSQL image from `postgres:16-alpine` to `timescale/timescaledb:latest-pg16` (or a pinned version like `timescale/timescaledb:2.17.2-pg16`).

No other infrastructure changes are needed — TimescaleDB is a drop-in extension.

### Local Development

For local development (non-Docker), TimescaleDB must be installed as a PostgreSQL extension. On macOS: `brew install timescaledb`. On Linux: follow [TimescaleDB install docs](https://docs.timescale.com/self-hosted/latest/install/).

### Testing

The schema-per-test isolation pattern works with TimescaleDB. The `timescaledb` extension is database-level (created once via `CREATE EXTENSION`), and hypertables in different schemas are independent. The test schema setup requires no changes — `create_hypertable()` operates within the active `search_path`.

### SQLx Compatibility

No special SQLx support is needed. History tables are standard PostgreSQL tables from SQLx's perspective. `INSERT`, `SELECT`, `time_bucket()`, and array operators all work as regular SQL. TimescaleDB-specific DDL (`create_hypertable`, `add_compression_policy`, etc.) runs in migrations only.

## Implementation Scope

### Phase 1 (this migration)
- [ ] `CREATE EXTENSION IF NOT EXISTS timescaledb`
- [ ] Create four `<entity>_history` tables
- [ ] Convert to hypertables with `create_hypertable()`
- [ ] Create indexes (entity lookup, status change filter, GIN on changed_fields, ref lookup)
- [ ] Create trigger functions for `execution`, `worker`, `enforcement`, `event`
- [ ] Attach triggers to operational tables
- [ ] Configure compression policies
- [ ] Configure retention policies

### Phase 2 (future — API & UI)
- [ ] History repository in `crates/common/src/repositories/`
- [ ] API endpoints (e.g., `GET /api/v1/executions/:id/history`)
- [ ] Web UI history panel on entity detail pages
- [ ] Continuous aggregates for dashboards

### Phase 3 (future — analytics)
- [ ] Dashboard widgets showing execution throughput, failure rates, worker health trends
- [ ] Configurable retention periods via admin settings
- [ ] Export/archival to external storage before retention expiry

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Trigger overhead on hot paths | Triggers are lightweight (JSONB build + single INSERT into an append-optimized hypertable). Benchmark if execution throughput exceeds 1K/sec. |
| Storage growth | Compression (7-day delay) + retention policies bound storage automatically. |
| JSONB query performance | Partial indexes on `changed_fields` avoid full scans. Continuous aggregates pre-compute hot queries. |
| Schema drift (new columns not tracked) | When adding mutable columns to tracked tables, add a corresponding `IS DISTINCT FROM` check to the trigger function. Document this in the pitfalls section of AGENTS.md. |
| Test compatibility | TimescaleDB extension is database-level; schema-per-test isolation is unaffected. Verify in CI. |

## Docker Image Pinning

For reproducibility, pin the TimescaleDB image version rather than using `latest`:

```yaml
postgres:
  image: timescale/timescaledb:2.17.2-pg16
```

Update the pin periodically as new stable versions are released.