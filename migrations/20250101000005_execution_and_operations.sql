-- Migration: Execution and Operations
-- Description: Creates execution, inquiry, rule, worker, and notification tables.
--              Includes retry tracking, worker health views, and helper functions.
--              Consolidates former migrations: 000006 (execution_system), 000008
--              (worker_notification), 000014 (worker_table), and 20260209 (phase3).
--
--              NOTE: The execution table is converted to a TimescaleDB hypertable in
--              migration 000009. Hypertables cannot be the target of FK constraints,
--              so columns referencing execution (inquiry.execution, workflow_execution.execution)
--              are plain BIGINT with no FK. Similarly, columns ON the execution table that
--              would self-reference or reference other hypertables (parent, enforcement,
--              original_execution) are plain BIGINT. The action and executor FKs are also
--              omitted since they would need to be dropped during hypertable conversion.
-- Version: 20250101000005

-- ============================================================================
-- EXECUTION TABLE
-- ============================================================================

CREATE TABLE execution (
    id BIGSERIAL PRIMARY KEY,
    action BIGINT,          -- references action(id); no FK because execution becomes a hypertable
    action_ref TEXT NOT NULL,
    config JSONB,
    env_vars JSONB,
    parent BIGINT,          -- self-reference; no FK because execution becomes a hypertable
    enforcement BIGINT,     -- references enforcement(id); no FK (both are hypertables)
    executor BIGINT,        -- references identity(id); no FK because execution becomes a hypertable
    worker BIGINT,          -- references worker(id); no FK because execution becomes a hypertable
    status execution_status_enum NOT NULL DEFAULT 'requested',
    result JSONB,
    started_at TIMESTAMPTZ,         -- set when execution transitions to 'running'
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_workflow BOOLEAN DEFAULT false NOT NULL,
    workflow_def BIGINT,    -- references workflow_definition(id); no FK because execution becomes a hypertable
    workflow_task JSONB,

    -- Retry tracking (baked in from phase 3)
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER,
    retry_reason TEXT,
    original_execution BIGINT, -- self-reference; no FK because execution becomes a hypertable

    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_execution_action ON execution(action);
CREATE INDEX idx_execution_action_ref ON execution(action_ref);
CREATE INDEX idx_execution_parent ON execution(parent);
CREATE INDEX idx_execution_enforcement ON execution(enforcement);
CREATE INDEX idx_execution_executor ON execution(executor);
CREATE INDEX idx_execution_worker ON execution(worker);
CREATE INDEX idx_execution_status ON execution(status);
CREATE INDEX idx_execution_created ON execution(created DESC);
CREATE INDEX idx_execution_updated ON execution(updated DESC);
CREATE INDEX idx_execution_status_created ON execution(status, created DESC);
CREATE INDEX idx_execution_status_updated ON execution(status, updated DESC);
CREATE INDEX idx_execution_action_status ON execution(action, status);
CREATE INDEX idx_execution_executor_created ON execution(executor, created DESC);
CREATE INDEX idx_execution_worker_created ON execution(worker, created DESC);
CREATE INDEX idx_execution_parent_created ON execution(parent, created DESC);
CREATE INDEX idx_execution_result_gin ON execution USING GIN (result);
CREATE INDEX idx_execution_env_vars_gin ON execution USING GIN (env_vars);
CREATE INDEX idx_execution_original_execution ON execution(original_execution) WHERE original_execution IS NOT NULL;
CREATE INDEX idx_execution_status_retry ON execution(status, retry_count) WHERE status = 'failed' AND retry_count < COALESCE(max_retries, 0);

-- Trigger
CREATE TRIGGER update_execution_updated
    BEFORE UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE execution IS 'Executions represent action runs, supports nested workflows';
COMMENT ON COLUMN execution.action IS 'Action being executed (may be null if action deleted)';
COMMENT ON COLUMN execution.action_ref IS 'Action reference (preserved even if action deleted)';
COMMENT ON COLUMN execution.config IS 'Snapshot of action configuration at execution time';
COMMENT ON COLUMN execution.env_vars IS 'Environment variables for this execution as key-value pairs (string -> string). These are set in the execution environment and are separate from action parameters. Used for execution context, configuration, and non-sensitive metadata.';
COMMENT ON COLUMN execution.parent IS 'Parent execution ID for workflow hierarchies (no FK — execution is a hypertable)';
COMMENT ON COLUMN execution.enforcement IS 'Enforcement that triggered this execution (no FK — both are hypertables)';
COMMENT ON COLUMN execution.executor IS 'Identity that initiated the execution (no FK — execution is a hypertable)';
COMMENT ON COLUMN execution.worker IS 'Assigned worker handling this execution (no FK — execution is a hypertable)';
COMMENT ON COLUMN execution.status IS 'Current execution lifecycle status';
COMMENT ON COLUMN execution.result IS 'Execution output/results';
COMMENT ON COLUMN execution.retry_count IS 'Current retry attempt number (0 = first attempt, 1 = first retry, etc.)';
COMMENT ON COLUMN execution.max_retries IS 'Maximum retries for this execution. Copied from action.max_retries at creation time.';
COMMENT ON COLUMN execution.retry_reason IS 'Reason for retry (e.g., "worker_unavailable", "transient_error", "manual_retry")';
COMMENT ON COLUMN execution.original_execution IS 'ID of the original execution if this is a retry. Forms a retry chain.';

-- ============================================================================

-- ============================================================================
-- INQUIRY TABLE
-- ============================================================================

CREATE TABLE inquiry (
    id BIGSERIAL PRIMARY KEY,
    execution BIGINT NOT NULL, -- references execution(id); no FK because execution is a hypertable
    prompt TEXT NOT NULL,
    response_schema JSONB,
    assigned_to BIGINT REFERENCES identity(id) ON DELETE SET NULL,
    status inquiry_status_enum NOT NULL DEFAULT 'pending',
    response JSONB,
    timeout_at TIMESTAMPTZ,
    responded_at TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_inquiry_execution ON inquiry(execution);
CREATE INDEX idx_inquiry_assigned_to ON inquiry(assigned_to);
CREATE INDEX idx_inquiry_status ON inquiry(status);
CREATE INDEX idx_inquiry_timeout_at ON inquiry(timeout_at) WHERE timeout_at IS NOT NULL;
CREATE INDEX idx_inquiry_created ON inquiry(created DESC);
CREATE INDEX idx_inquiry_status_created ON inquiry(status, created DESC);
CREATE INDEX idx_inquiry_assigned_status ON inquiry(assigned_to, status);
CREATE INDEX idx_inquiry_execution_status ON inquiry(execution, status);
CREATE INDEX idx_inquiry_response_gin ON inquiry USING GIN (response);

-- Trigger
CREATE TRIGGER update_inquiry_updated
    BEFORE UPDATE ON inquiry
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE inquiry IS 'Inquiries enable human-in-the-loop workflows with async user interactions';
COMMENT ON COLUMN inquiry.execution IS 'Execution that is waiting on this inquiry (no FK — execution is a hypertable)';
COMMENT ON COLUMN inquiry.prompt IS 'Question or prompt text for the user';
COMMENT ON COLUMN inquiry.response_schema IS 'JSON schema defining expected response format';
COMMENT ON COLUMN inquiry.assigned_to IS 'Identity who should respond to this inquiry';
COMMENT ON COLUMN inquiry.status IS 'Current inquiry lifecycle status';
COMMENT ON COLUMN inquiry.response IS 'User response data';
COMMENT ON COLUMN inquiry.timeout_at IS 'When this inquiry expires';
COMMENT ON COLUMN inquiry.responded_at IS 'When the response was received';

-- ============================================================================

-- ============================================================================
-- RULE TABLE
-- ============================================================================

CREATE TABLE rule (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT NOT NULL,
    label TEXT NOT NULL,
    description TEXT NOT NULL,
    action BIGINT REFERENCES action(id) ON DELETE SET NULL,
    action_ref TEXT NOT NULL,
    trigger BIGINT REFERENCES trigger(id) ON DELETE SET NULL,
    trigger_ref TEXT NOT NULL,
    conditions JSONB NOT NULL DEFAULT '[]'::jsonb,
    action_params JSONB DEFAULT '{}'::jsonb,
    trigger_params JSONB DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL,
    is_adhoc BOOLEAN NOT NULL DEFAULT FALSE,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT rule_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT rule_ref_format CHECK (ref ~ '^[^.]+\.[^.]+$')
);

-- Indexes
CREATE INDEX idx_rule_ref ON rule(ref);
CREATE INDEX idx_rule_pack ON rule(pack);
CREATE INDEX idx_rule_action ON rule(action);
CREATE INDEX idx_rule_trigger ON rule(trigger);
CREATE INDEX idx_rule_enabled ON rule(enabled) WHERE enabled = TRUE;
CREATE INDEX idx_rule_is_adhoc ON rule(is_adhoc) WHERE is_adhoc = true;
CREATE INDEX idx_rule_created ON rule(created DESC);
CREATE INDEX idx_rule_trigger_enabled ON rule(trigger, enabled);
CREATE INDEX idx_rule_action_enabled ON rule(action, enabled);
CREATE INDEX idx_rule_pack_enabled ON rule(pack, enabled);
CREATE INDEX idx_rule_action_params_gin ON rule USING GIN (action_params);
CREATE INDEX idx_rule_trigger_params_gin ON rule USING GIN (trigger_params);

-- Trigger
CREATE TRIGGER update_rule_updated
    BEFORE UPDATE ON rule
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE rule IS 'Rules link triggers to actions with conditions';
COMMENT ON COLUMN rule.ref IS 'Unique rule reference (format: pack.name)';
COMMENT ON COLUMN rule.label IS 'Human-readable rule name';
COMMENT ON COLUMN rule.action IS 'Action to execute when rule triggers (null if action deleted)';
COMMENT ON COLUMN rule.trigger IS 'Trigger that activates this rule (null if trigger deleted)';
COMMENT ON COLUMN rule.conditions IS 'Condition expressions to evaluate before executing action';
COMMENT ON COLUMN rule.action_params IS 'Parameter overrides for the action';
COMMENT ON COLUMN rule.trigger_params IS 'Parameter overrides for the trigger';
COMMENT ON COLUMN rule.enabled IS 'Whether this rule is active';
COMMENT ON COLUMN rule.is_adhoc IS 'True if rule was manually created (ad-hoc), false if installed from pack';

-- ============================================================================

-- Add foreign key constraints now that rule table exists
ALTER TABLE enforcement
    ADD CONSTRAINT enforcement_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;

ALTER TABLE event
    ADD CONSTRAINT event_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;

-- ============================================================================
-- WORKER TABLE
-- ============================================================================

CREATE TABLE worker (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    worker_type worker_type_enum NOT NULL,
    worker_role worker_role_enum NOT NULL,
    runtime BIGINT REFERENCES runtime(id) ON DELETE SET NULL,
    host TEXT,
    port INTEGER,
    status worker_status_enum NOT NULL DEFAULT 'active',
    capabilities JSONB,
    meta JSONB,
    last_heartbeat TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_worker_name ON worker(name);
CREATE INDEX idx_worker_type ON worker(worker_type);
CREATE INDEX idx_worker_role ON worker(worker_role);
CREATE INDEX idx_worker_runtime ON worker(runtime);
CREATE INDEX idx_worker_status ON worker(status);
CREATE INDEX idx_worker_last_heartbeat ON worker(last_heartbeat DESC) WHERE last_heartbeat IS NOT NULL;
CREATE INDEX idx_worker_created ON worker(created DESC);
CREATE INDEX idx_worker_status_role ON worker(status, worker_role);
CREATE INDEX idx_worker_capabilities_gin ON worker USING GIN (capabilities);
CREATE INDEX idx_worker_meta_gin ON worker USING GIN (meta);
CREATE INDEX idx_worker_capabilities_health_status ON worker USING GIN ((capabilities -> 'health' -> 'status'));

-- Trigger
CREATE TRIGGER update_worker_updated
    BEFORE UPDATE ON worker
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE worker IS 'Worker registration and tracking table for action and sensor workers';
COMMENT ON COLUMN worker.name IS 'Unique worker identifier (typically hostname-based)';
COMMENT ON COLUMN worker.worker_type IS 'Worker deployment type (local or remote)';
COMMENT ON COLUMN worker.worker_role IS 'Worker role (action or sensor)';
COMMENT ON COLUMN worker.runtime IS 'Runtime environment this worker supports (optional)';
COMMENT ON COLUMN worker.host IS 'Worker host address';
COMMENT ON COLUMN worker.port IS 'Worker port number';
COMMENT ON COLUMN worker.status IS 'Worker operational status';
COMMENT ON COLUMN worker.capabilities IS 'Worker capabilities (e.g., max_concurrent_executions, supported runtimes)';
COMMENT ON COLUMN worker.meta IS 'Additional worker metadata';
COMMENT ON COLUMN worker.last_heartbeat IS 'Timestamp of last heartbeat from worker';

-- ============================================================================
-- NOTIFICATION TABLE
-- ============================================================================

CREATE TABLE notification (
    id BIGSERIAL PRIMARY KEY,
    channel TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity TEXT NOT NULL,
    activity TEXT NOT NULL,
    state notification_status_enum NOT NULL DEFAULT 'created',
    content JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_notification_channel ON notification(channel);
CREATE INDEX idx_notification_entity_type ON notification(entity_type);
CREATE INDEX idx_notification_entity ON notification(entity);
CREATE INDEX idx_notification_state ON notification(state);
CREATE INDEX idx_notification_created ON notification(created DESC);
CREATE INDEX idx_notification_channel_state ON notification(channel, state);
CREATE INDEX idx_notification_entity_type_entity ON notification(entity_type, entity);
CREATE INDEX idx_notification_state_created ON notification(state, created DESC);
CREATE INDEX idx_notification_content_gin ON notification USING GIN (content);

-- Trigger
CREATE TRIGGER update_notification_updated
    BEFORE UPDATE ON notification
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Function for pg_notify on notification insert
CREATE OR REPLACE FUNCTION notify_on_insert()
RETURNS TRIGGER AS $$
DECLARE
    payload TEXT;
BEGIN
    -- Build JSON payload with id, entity, and activity
    payload := json_build_object(
        'id', NEW.id,
        'entity_type', NEW.entity_type,
        'entity', NEW.entity,
        'activity', NEW.activity
    )::text;

    -- Send notification to the specified channel
    PERFORM pg_notify(NEW.channel, payload);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to send pg_notify on notification insert
CREATE TRIGGER notify_on_notification_insert
    AFTER INSERT ON notification
    FOR EACH ROW
    EXECUTE FUNCTION notify_on_insert();

-- Comments
COMMENT ON TABLE notification IS 'System notifications about entity changes for real-time updates';
COMMENT ON COLUMN notification.channel IS 'Notification channel (typically table name)';
COMMENT ON COLUMN notification.entity_type IS 'Type of entity (table name)';
COMMENT ON COLUMN notification.entity IS 'Entity identifier (typically ID or ref)';
COMMENT ON COLUMN notification.activity IS 'Activity type (e.g., "created", "updated", "completed")';
COMMENT ON COLUMN notification.state IS 'Processing state of notification';
COMMENT ON COLUMN notification.content IS 'Optional notification payload data';

-- ============================================================================
-- WORKER HEALTH VIEWS AND FUNCTIONS
-- ============================================================================

-- View for healthy workers (convenience for queries)
CREATE OR REPLACE VIEW healthy_workers AS
SELECT
    w.id,
    w.name,
    w.worker_type,
    w.worker_role,
    w.runtime,
    w.status,
    w.capabilities,
    w.last_heartbeat,
    (w.capabilities -> 'health' ->> 'status')::TEXT as health_status,
    (w.capabilities -> 'health' ->> 'queue_depth')::INTEGER as queue_depth,
    (w.capabilities -> 'health' ->> 'consecutive_failures')::INTEGER as consecutive_failures
FROM worker w
WHERE
    w.status = 'active'
    AND w.last_heartbeat > NOW() - INTERVAL '30 seconds'
    AND (
        -- Healthy if no health info (backward compatible)
        w.capabilities -> 'health' IS NULL
        OR
        -- Or explicitly marked healthy
        w.capabilities -> 'health' ->> 'status' IN ('healthy', 'degraded')
    );

COMMENT ON VIEW healthy_workers IS 'Workers that are active, have fresh heartbeat, and are healthy or degraded (not unhealthy)';

-- Function to get worker queue depth estimate
CREATE OR REPLACE FUNCTION get_worker_queue_depth(worker_id_param BIGINT)
RETURNS INTEGER AS $$
BEGIN
    RETURN (
        SELECT (capabilities -> 'health' ->> 'queue_depth')::INTEGER
        FROM worker
        WHERE id = worker_id_param
    );
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_worker_queue_depth IS 'Extract current queue depth from worker health metadata';

-- Function to check if execution is retriable
CREATE OR REPLACE FUNCTION is_execution_retriable(execution_id_param BIGINT)
RETURNS BOOLEAN AS $$
DECLARE
    exec_record RECORD;
BEGIN
    SELECT
        e.retry_count,
        e.max_retries,
        e.status
    INTO exec_record
    FROM execution e
    WHERE e.id = execution_id_param;

    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;

    -- Can retry if:
    -- 1. Status is failed
    -- 2. max_retries is set and > 0
    -- 3. retry_count < max_retries
    RETURN (
        exec_record.status = 'failed'
        AND exec_record.max_retries IS NOT NULL
        AND exec_record.max_retries > 0
        AND exec_record.retry_count < exec_record.max_retries
    );
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION is_execution_retriable IS 'Check if a failed execution can be automatically retried based on retry limits';
