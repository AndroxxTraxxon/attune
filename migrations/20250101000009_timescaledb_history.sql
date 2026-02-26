-- Migration: TimescaleDB Entity History and Analytics
-- Description: Creates append-only history hypertables for execution, worker, enforcement,
--              and event tables. Uses JSONB diff format to track field-level changes via
--              PostgreSQL triggers. Includes continuous aggregates for dashboard analytics.
--              Consolidates former migrations: 20260226100000 (entity_history_timescaledb),
--              20260226200000 (continuous_aggregates), and 20260226300000 (fix + result digest).
--              See docs/plans/timescaledb-entity-history.md for full design.
-- Version: 20250101000009

-- ============================================================================
-- EXTENSION
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Returns a small {digest, size, type} object instead of the full JSONB value.
-- Used in history triggers for columns that can be arbitrarily large (e.g. result).
-- The full value is always available on the live row.
CREATE OR REPLACE FUNCTION _jsonb_digest_summary(val JSONB)
RETURNS JSONB AS $$
BEGIN
    IF val IS NULL THEN
        RETURN NULL;
    END IF;
    RETURN jsonb_build_object(
        'digest', 'md5:' || md5(val::text),
        'size',   octet_length(val::text),
        'type',   jsonb_typeof(val)
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION _jsonb_digest_summary(JSONB) IS
    'Returns a compact {digest, size, type} summary of a JSONB value for use in history tables. '
    'The digest is md5 of the text representation — sufficient for change-detection, not for security.';

-- ============================================================================
-- HISTORY TABLES
-- ============================================================================

-- ----------------------------------------------------------------------------
-- execution_history
-- ----------------------------------------------------------------------------

CREATE TABLE execution_history (
    time             TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    operation        TEXT           NOT NULL,
    entity_id        BIGINT         NOT NULL,
    entity_ref       TEXT,
    changed_fields   TEXT[]         NOT NULL DEFAULT '{}',
    old_values       JSONB,
    new_values       JSONB
);

SELECT create_hypertable('execution_history', 'time',
    chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_execution_history_entity
    ON execution_history (entity_id, time DESC);

CREATE INDEX idx_execution_history_entity_ref
    ON execution_history (entity_ref, time DESC);

CREATE INDEX idx_execution_history_status_changes
    ON execution_history (time DESC)
    WHERE 'status' = ANY(changed_fields);

CREATE INDEX idx_execution_history_changed_fields
    ON execution_history USING GIN (changed_fields);

COMMENT ON TABLE execution_history IS 'Append-only history of field-level changes to the execution table (TimescaleDB hypertable)';
COMMENT ON COLUMN execution_history.time IS 'When the change occurred (hypertable partitioning dimension)';
COMMENT ON COLUMN execution_history.operation IS 'INSERT, UPDATE, or DELETE';
COMMENT ON COLUMN execution_history.entity_id IS 'execution.id of the changed row';
COMMENT ON COLUMN execution_history.entity_ref IS 'Denormalized action_ref for JOIN-free queries';
COMMENT ON COLUMN execution_history.changed_fields IS 'Array of field names that changed (empty for INSERT/DELETE)';
COMMENT ON COLUMN execution_history.old_values IS 'Previous values of changed fields (NULL for INSERT)';
COMMENT ON COLUMN execution_history.new_values IS 'New values of changed fields (NULL for DELETE)';

-- ----------------------------------------------------------------------------
-- worker_history
-- ----------------------------------------------------------------------------

CREATE TABLE worker_history (
    time             TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    operation        TEXT           NOT NULL,
    entity_id        BIGINT         NOT NULL,
    entity_ref       TEXT,
    changed_fields   TEXT[]         NOT NULL DEFAULT '{}',
    old_values       JSONB,
    new_values       JSONB
);

SELECT create_hypertable('worker_history', 'time',
    chunk_time_interval => INTERVAL '7 days');

CREATE INDEX idx_worker_history_entity
    ON worker_history (entity_id, time DESC);

CREATE INDEX idx_worker_history_entity_ref
    ON worker_history (entity_ref, time DESC);

CREATE INDEX idx_worker_history_status_changes
    ON worker_history (time DESC)
    WHERE 'status' = ANY(changed_fields);

CREATE INDEX idx_worker_history_changed_fields
    ON worker_history USING GIN (changed_fields);

COMMENT ON TABLE worker_history IS 'Append-only history of field-level changes to the worker table (TimescaleDB hypertable)';
COMMENT ON COLUMN worker_history.entity_ref IS 'Denormalized worker name for JOIN-free queries';

-- ----------------------------------------------------------------------------
-- enforcement_history
-- ----------------------------------------------------------------------------

CREATE TABLE enforcement_history (
    time             TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    operation        TEXT           NOT NULL,
    entity_id        BIGINT         NOT NULL,
    entity_ref       TEXT,
    changed_fields   TEXT[]         NOT NULL DEFAULT '{}',
    old_values       JSONB,
    new_values       JSONB
);

SELECT create_hypertable('enforcement_history', 'time',
    chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_enforcement_history_entity
    ON enforcement_history (entity_id, time DESC);

CREATE INDEX idx_enforcement_history_entity_ref
    ON enforcement_history (entity_ref, time DESC);

CREATE INDEX idx_enforcement_history_status_changes
    ON enforcement_history (time DESC)
    WHERE 'status' = ANY(changed_fields);

CREATE INDEX idx_enforcement_history_changed_fields
    ON enforcement_history USING GIN (changed_fields);

COMMENT ON TABLE enforcement_history IS 'Append-only history of field-level changes to the enforcement table (TimescaleDB hypertable)';
COMMENT ON COLUMN enforcement_history.entity_ref IS 'Denormalized rule_ref for JOIN-free queries';

-- ----------------------------------------------------------------------------
-- event_history
-- ----------------------------------------------------------------------------

CREATE TABLE event_history (
    time             TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    operation        TEXT           NOT NULL,
    entity_id        BIGINT         NOT NULL,
    entity_ref       TEXT,
    changed_fields   TEXT[]         NOT NULL DEFAULT '{}',
    old_values       JSONB,
    new_values       JSONB
);

SELECT create_hypertable('event_history', 'time',
    chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_event_history_entity
    ON event_history (entity_id, time DESC);

CREATE INDEX idx_event_history_entity_ref
    ON event_history (entity_ref, time DESC);

CREATE INDEX idx_event_history_changed_fields
    ON event_history USING GIN (changed_fields);

COMMENT ON TABLE event_history IS 'Append-only history of field-level changes to the event table (TimescaleDB hypertable)';
COMMENT ON COLUMN event_history.entity_ref IS 'Denormalized trigger_ref for JOIN-free queries';

-- ============================================================================
-- TRIGGER FUNCTIONS
-- ============================================================================

-- ----------------------------------------------------------------------------
-- execution history trigger
-- Tracked fields: status, result, executor, workflow_task, env_vars
-- Note: result uses _jsonb_digest_summary() to avoid storing large payloads
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION record_execution_history()
RETURNS TRIGGER AS $$
DECLARE
    changed TEXT[] := '{}';
    old_vals JSONB := '{}';
    new_vals JSONB := '{}';
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO execution_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'INSERT', NEW.id, NEW.action_ref, '{}', NULL,
                jsonb_build_object(
                    'status', NEW.status,
                    'action_ref', NEW.action_ref,
                    'executor', NEW.executor,
                    'parent', NEW.parent,
                    'enforcement', NEW.enforcement
                ));
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO execution_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'DELETE', OLD.id, OLD.action_ref, '{}', NULL, NULL);
        RETURN OLD;
    END IF;

    -- UPDATE: detect which fields changed

    IF OLD.status IS DISTINCT FROM NEW.status THEN
        changed := array_append(changed, 'status');
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    -- Result: store a compact digest instead of the full JSONB to avoid bloat.
    -- The live execution row always has the complete result.
    IF OLD.result IS DISTINCT FROM NEW.result THEN
        changed := array_append(changed, 'result');
        old_vals := old_vals || jsonb_build_object('result', _jsonb_digest_summary(OLD.result));
        new_vals := new_vals || jsonb_build_object('result', _jsonb_digest_summary(NEW.result));
    END IF;

    IF OLD.executor IS DISTINCT FROM NEW.executor THEN
        changed := array_append(changed, 'executor');
        old_vals := old_vals || jsonb_build_object('executor', OLD.executor);
        new_vals := new_vals || jsonb_build_object('executor', NEW.executor);
    END IF;

    IF OLD.workflow_task IS DISTINCT FROM NEW.workflow_task THEN
        changed := array_append(changed, 'workflow_task');
        old_vals := old_vals || jsonb_build_object('workflow_task', OLD.workflow_task);
        new_vals := new_vals || jsonb_build_object('workflow_task', NEW.workflow_task);
    END IF;

    IF OLD.env_vars IS DISTINCT FROM NEW.env_vars THEN
        changed := array_append(changed, 'env_vars');
        old_vals := old_vals || jsonb_build_object('env_vars', OLD.env_vars);
        new_vals := new_vals || jsonb_build_object('env_vars', NEW.env_vars);
    END IF;

    -- Only record if something actually changed
    IF array_length(changed, 1) > 0 THEN
        INSERT INTO execution_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'UPDATE', NEW.id, NEW.action_ref, changed, old_vals, new_vals);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_execution_history() IS 'Records field-level changes to execution table in execution_history hypertable';

-- ----------------------------------------------------------------------------
-- worker history trigger
-- Tracked fields: name, status, capabilities, meta, host, port
-- Excludes: last_heartbeat when it is the only field that changed
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION record_worker_history()
RETURNS TRIGGER AS $$
DECLARE
    changed TEXT[] := '{}';
    old_vals JSONB := '{}';
    new_vals JSONB := '{}';
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO worker_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'INSERT', NEW.id, NEW.name, '{}', NULL,
                jsonb_build_object(
                    'name', NEW.name,
                    'worker_type', NEW.worker_type,
                    'worker_role', NEW.worker_role,
                    'status', NEW.status,
                    'host', NEW.host,
                    'port', NEW.port
                ));
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO worker_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'DELETE', OLD.id, OLD.name, '{}', NULL, NULL);
        RETURN OLD;
    END IF;

    -- UPDATE: detect which fields changed
    IF OLD.name IS DISTINCT FROM NEW.name THEN
        changed := array_append(changed, 'name');
        old_vals := old_vals || jsonb_build_object('name', OLD.name);
        new_vals := new_vals || jsonb_build_object('name', NEW.name);
    END IF;

    IF OLD.status IS DISTINCT FROM NEW.status THEN
        changed := array_append(changed, 'status');
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    IF OLD.capabilities IS DISTINCT FROM NEW.capabilities THEN
        changed := array_append(changed, 'capabilities');
        old_vals := old_vals || jsonb_build_object('capabilities', OLD.capabilities);
        new_vals := new_vals || jsonb_build_object('capabilities', NEW.capabilities);
    END IF;

    IF OLD.meta IS DISTINCT FROM NEW.meta THEN
        changed := array_append(changed, 'meta');
        old_vals := old_vals || jsonb_build_object('meta', OLD.meta);
        new_vals := new_vals || jsonb_build_object('meta', NEW.meta);
    END IF;

    IF OLD.host IS DISTINCT FROM NEW.host THEN
        changed := array_append(changed, 'host');
        old_vals := old_vals || jsonb_build_object('host', OLD.host);
        new_vals := new_vals || jsonb_build_object('host', NEW.host);
    END IF;

    IF OLD.port IS DISTINCT FROM NEW.port THEN
        changed := array_append(changed, 'port');
        old_vals := old_vals || jsonb_build_object('port', OLD.port);
        new_vals := new_vals || jsonb_build_object('port', NEW.port);
    END IF;

    -- Only record if something besides last_heartbeat changed.
    -- Pure heartbeat-only updates are excluded to avoid high-volume noise.
    IF array_length(changed, 1) > 0 THEN
        INSERT INTO worker_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'UPDATE', NEW.id, NEW.name, changed, old_vals, new_vals);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_worker_history() IS 'Records field-level changes to worker table in worker_history hypertable. Excludes heartbeat-only updates.';

-- ----------------------------------------------------------------------------
-- enforcement history trigger
-- Tracked fields: status, payload
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION record_enforcement_history()
RETURNS TRIGGER AS $$
DECLARE
    changed TEXT[] := '{}';
    old_vals JSONB := '{}';
    new_vals JSONB := '{}';
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO enforcement_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'INSERT', NEW.id, NEW.rule_ref, '{}', NULL,
                jsonb_build_object(
                    'rule_ref', NEW.rule_ref,
                    'trigger_ref', NEW.trigger_ref,
                    'status', NEW.status,
                    'condition', NEW.condition,
                    'event', NEW.event
                ));
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO enforcement_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'DELETE', OLD.id, OLD.rule_ref, '{}', NULL, NULL);
        RETURN OLD;
    END IF;

    -- UPDATE: detect which fields changed
    IF OLD.status IS DISTINCT FROM NEW.status THEN
        changed := array_append(changed, 'status');
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    IF OLD.payload IS DISTINCT FROM NEW.payload THEN
        changed := array_append(changed, 'payload');
        old_vals := old_vals || jsonb_build_object('payload', OLD.payload);
        new_vals := new_vals || jsonb_build_object('payload', NEW.payload);
    END IF;

    -- Only record if something actually changed
    IF array_length(changed, 1) > 0 THEN
        INSERT INTO enforcement_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'UPDATE', NEW.id, NEW.rule_ref, changed, old_vals, new_vals);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_enforcement_history() IS 'Records field-level changes to enforcement table in enforcement_history hypertable';

-- ----------------------------------------------------------------------------
-- event history trigger
-- Tracked fields: config, payload
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION record_event_history()
RETURNS TRIGGER AS $$
DECLARE
    changed TEXT[] := '{}';
    old_vals JSONB := '{}';
    new_vals JSONB := '{}';
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO event_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'INSERT', NEW.id, NEW.trigger_ref, '{}', NULL,
                jsonb_build_object(
                    'trigger_ref', NEW.trigger_ref,
                    'source', NEW.source,
                    'source_ref', NEW.source_ref,
                    'rule', NEW.rule,
                    'rule_ref', NEW.rule_ref
                ));
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO event_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'DELETE', OLD.id, OLD.trigger_ref, '{}', NULL, NULL);
        RETURN OLD;
    END IF;

    -- UPDATE: detect which fields changed
    IF OLD.config IS DISTINCT FROM NEW.config THEN
        changed := array_append(changed, 'config');
        old_vals := old_vals || jsonb_build_object('config', OLD.config);
        new_vals := new_vals || jsonb_build_object('config', NEW.config);
    END IF;

    IF OLD.payload IS DISTINCT FROM NEW.payload THEN
        changed := array_append(changed, 'payload');
        old_vals := old_vals || jsonb_build_object('payload', OLD.payload);
        new_vals := new_vals || jsonb_build_object('payload', NEW.payload);
    END IF;

    -- Only record if something actually changed
    IF array_length(changed, 1) > 0 THEN
        INSERT INTO event_history (time, operation, entity_id, entity_ref, changed_fields, old_values, new_values)
        VALUES (NOW(), 'UPDATE', NEW.id, NEW.trigger_ref, changed, old_vals, new_vals);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_event_history() IS 'Records field-level changes to event table in event_history hypertable';

-- ============================================================================
-- ATTACH TRIGGERS TO OPERATIONAL TABLES
-- ============================================================================

CREATE TRIGGER execution_history_trigger
    AFTER INSERT OR UPDATE OR DELETE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION record_execution_history();

CREATE TRIGGER worker_history_trigger
    AFTER INSERT OR UPDATE OR DELETE ON worker
    FOR EACH ROW
    EXECUTE FUNCTION record_worker_history();

CREATE TRIGGER enforcement_history_trigger
    AFTER INSERT OR UPDATE OR DELETE ON enforcement
    FOR EACH ROW
    EXECUTE FUNCTION record_enforcement_history();

CREATE TRIGGER event_history_trigger
    AFTER INSERT OR UPDATE OR DELETE ON event
    FOR EACH ROW
    EXECUTE FUNCTION record_event_history();

-- ============================================================================
-- COMPRESSION POLICIES
-- ============================================================================

ALTER TABLE execution_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'time DESC'
);
SELECT add_compression_policy('execution_history', INTERVAL '7 days');

ALTER TABLE worker_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'time DESC'
);
SELECT add_compression_policy('worker_history', INTERVAL '7 days');

ALTER TABLE enforcement_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'time DESC'
);
SELECT add_compression_policy('enforcement_history', INTERVAL '7 days');

ALTER TABLE event_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'time DESC'
);
SELECT add_compression_policy('event_history', INTERVAL '7 days');

-- ============================================================================
-- RETENTION POLICIES
-- ============================================================================

SELECT add_retention_policy('execution_history', INTERVAL '90 days');
SELECT add_retention_policy('enforcement_history', INTERVAL '90 days');
SELECT add_retention_policy('event_history', INTERVAL '30 days');
SELECT add_retention_policy('worker_history', INTERVAL '180 days');

-- ============================================================================
-- CONTINUOUS AGGREGATES
-- ============================================================================

-- Drop existing continuous aggregates if they exist, so this migration can be
-- re-run safely after a partial failure. (TimescaleDB continuous aggregates
-- must be dropped with CASCADE to remove their associated policies.)
DROP MATERIALIZED VIEW IF EXISTS execution_status_hourly CASCADE;
DROP MATERIALIZED VIEW IF EXISTS execution_throughput_hourly CASCADE;
DROP MATERIALIZED VIEW IF EXISTS event_volume_hourly CASCADE;
DROP MATERIALIZED VIEW IF EXISTS worker_status_hourly CASCADE;
DROP MATERIALIZED VIEW IF EXISTS enforcement_volume_hourly CASCADE;

-- ----------------------------------------------------------------------------
-- execution_status_hourly
-- Tracks execution status transitions per hour, grouped by action_ref and new status.
-- Powers: execution throughput chart, failure rate widget, status breakdown over time.
-- ----------------------------------------------------------------------------

CREATE MATERIALIZED VIEW execution_status_hourly
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

SELECT add_continuous_aggregate_policy('execution_status_hourly',
    start_offset    => INTERVAL '7 days',
    end_offset      => INTERVAL '1 hour',
    schedule_interval => INTERVAL '30 minutes'
);

-- ----------------------------------------------------------------------------
-- execution_throughput_hourly
-- Tracks total execution creation volume per hour, regardless of status.
-- Powers: execution throughput sparkline on the dashboard.
-- ----------------------------------------------------------------------------

CREATE MATERIALIZED VIEW execution_throughput_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_ref AS action_ref,
    COUNT(*) AS execution_count
FROM execution_history
WHERE operation = 'INSERT'
GROUP BY bucket, entity_ref
WITH NO DATA;

SELECT add_continuous_aggregate_policy('execution_throughput_hourly',
    start_offset    => INTERVAL '7 days',
    end_offset      => INTERVAL '1 hour',
    schedule_interval => INTERVAL '30 minutes'
);

-- ----------------------------------------------------------------------------
-- event_volume_hourly
-- Tracks event creation volume per hour by trigger ref.
-- Powers: event throughput monitoring widget.
-- ----------------------------------------------------------------------------

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

SELECT add_continuous_aggregate_policy('event_volume_hourly',
    start_offset    => INTERVAL '7 days',
    end_offset      => INTERVAL '1 hour',
    schedule_interval => INTERVAL '30 minutes'
);

-- ----------------------------------------------------------------------------
-- worker_status_hourly
-- Tracks worker status changes per hour (online/offline/draining transitions).
-- Powers: worker health trends widget.
-- ----------------------------------------------------------------------------

CREATE MATERIALIZED VIEW worker_status_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_ref AS worker_name,
    new_values->>'status' AS new_status,
    COUNT(*) AS transition_count
FROM worker_history
WHERE 'status' = ANY(changed_fields)
GROUP BY bucket, entity_ref, new_values->>'status'
WITH NO DATA;

SELECT add_continuous_aggregate_policy('worker_status_hourly',
    start_offset    => INTERVAL '30 days',
    end_offset      => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);

-- ----------------------------------------------------------------------------
-- enforcement_volume_hourly
-- Tracks enforcement creation volume per hour by rule ref.
-- Powers: rule activation rate monitoring.
-- ----------------------------------------------------------------------------

CREATE MATERIALIZED VIEW enforcement_volume_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_ref AS rule_ref,
    COUNT(*) AS enforcement_count
FROM enforcement_history
WHERE operation = 'INSERT'
GROUP BY bucket, entity_ref
WITH NO DATA;

SELECT add_continuous_aggregate_policy('enforcement_volume_hourly',
    start_offset    => INTERVAL '7 days',
    end_offset      => INTERVAL '1 hour',
    schedule_interval => INTERVAL '30 minutes'
);

-- ============================================================================
-- INITIAL REFRESH NOTE
-- ============================================================================
-- NOTE: refresh_continuous_aggregate() cannot run inside a transaction block,
-- and the migration runner wraps each file in BEGIN/COMMIT. The continuous
-- aggregate policies configured above will automatically backfill data within
-- their first scheduled interval (30 min – 1 hour). On a fresh database there
-- is no history data to backfill anyway.
--
-- If you need an immediate manual refresh after migration, run outside a
-- transaction:
--   CALL refresh_continuous_aggregate('execution_status_hourly', NULL, NOW());
--   CALL refresh_continuous_aggregate('execution_throughput_hourly', NULL, NOW());
--   CALL refresh_continuous_aggregate('event_volume_hourly', NULL, NOW());
--   CALL refresh_continuous_aggregate('worker_status_hourly', NULL, NOW());
--   CALL refresh_continuous_aggregate('enforcement_volume_hourly', NULL, NOW());
