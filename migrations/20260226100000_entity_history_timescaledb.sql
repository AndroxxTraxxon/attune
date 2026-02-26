-- Migration: TimescaleDB Entity History Tracking
-- Description: Creates append-only history hypertables for execution, worker, enforcement,
--              and event tables. Uses JSONB diff format to track field-level changes via
--              PostgreSQL triggers. See docs/plans/timescaledb-entity-history.md for full design.
-- Version: 20260226100000

-- ============================================================================
-- EXTENSION
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;

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
        changed := changed || 'status';
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    IF OLD.result IS DISTINCT FROM NEW.result THEN
        changed := changed || 'result';
        old_vals := old_vals || jsonb_build_object('result', OLD.result);
        new_vals := new_vals || jsonb_build_object('result', NEW.result);
    END IF;

    IF OLD.executor IS DISTINCT FROM NEW.executor THEN
        changed := changed || 'executor';
        old_vals := old_vals || jsonb_build_object('executor', OLD.executor);
        new_vals := new_vals || jsonb_build_object('executor', NEW.executor);
    END IF;

    IF OLD.workflow_task IS DISTINCT FROM NEW.workflow_task THEN
        changed := changed || 'workflow_task';
        old_vals := old_vals || jsonb_build_object('workflow_task', OLD.workflow_task);
        new_vals := new_vals || jsonb_build_object('workflow_task', NEW.workflow_task);
    END IF;

    IF OLD.env_vars IS DISTINCT FROM NEW.env_vars THEN
        changed := changed || 'env_vars';
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
        changed := changed || 'name';
        old_vals := old_vals || jsonb_build_object('name', OLD.name);
        new_vals := new_vals || jsonb_build_object('name', NEW.name);
    END IF;

    IF OLD.status IS DISTINCT FROM NEW.status THEN
        changed := changed || 'status';
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    IF OLD.capabilities IS DISTINCT FROM NEW.capabilities THEN
        changed := changed || 'capabilities';
        old_vals := old_vals || jsonb_build_object('capabilities', OLD.capabilities);
        new_vals := new_vals || jsonb_build_object('capabilities', NEW.capabilities);
    END IF;

    IF OLD.meta IS DISTINCT FROM NEW.meta THEN
        changed := changed || 'meta';
        old_vals := old_vals || jsonb_build_object('meta', OLD.meta);
        new_vals := new_vals || jsonb_build_object('meta', NEW.meta);
    END IF;

    IF OLD.host IS DISTINCT FROM NEW.host THEN
        changed := changed || 'host';
        old_vals := old_vals || jsonb_build_object('host', OLD.host);
        new_vals := new_vals || jsonb_build_object('host', NEW.host);
    END IF;

    IF OLD.port IS DISTINCT FROM NEW.port THEN
        changed := changed || 'port';
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
        changed := changed || 'status';
        old_vals := old_vals || jsonb_build_object('status', OLD.status);
        new_vals := new_vals || jsonb_build_object('status', NEW.status);
    END IF;

    IF OLD.payload IS DISTINCT FROM NEW.payload THEN
        changed := changed || 'payload';
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
        changed := changed || 'config';
        old_vals := old_vals || jsonb_build_object('config', OLD.config);
        new_vals := new_vals || jsonb_build_object('config', NEW.config);
    END IF;

    IF OLD.payload IS DISTINCT FROM NEW.payload THEN
        changed := changed || 'payload';
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
