-- Migration: Execution System
-- Description: Creates tables for actions, rules, executions, and inquiries
-- Version: 20250101000004


-- ============================================================================
-- ACTION TABLE
-- ============================================================================

CREATE TABLE action (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT NOT NULL,
    label TEXT NOT NULL,
    description TEXT NOT NULL,
    entrypoint TEXT NOT NULL,
    runtime BIGINT REFERENCES runtime(id),
    param_schema JSONB,
    out_schema JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT action_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT action_ref_format CHECK (ref ~ '^[^.]+\.[^.]+$')
);

-- Indexes
CREATE INDEX idx_action_ref ON action(ref);
CREATE INDEX idx_action_pack ON action(pack);
CREATE INDEX idx_action_runtime ON action(runtime);
CREATE INDEX idx_action_created ON action(created DESC);
CREATE INDEX idx_action_pack_runtime ON action(pack, runtime);
CREATE INDEX idx_action_pack_created ON action(pack, created DESC);

-- Trigger
CREATE TRIGGER update_action_updated
    BEFORE UPDATE ON action
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON action TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE action_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE action IS 'Actions are executable tasks/operations';
COMMENT ON COLUMN action.ref IS 'Unique action reference (format: pack.name)';
COMMENT ON COLUMN action.label IS 'Human-readable action name';
COMMENT ON COLUMN action.entrypoint IS 'Code entry point for the action';
COMMENT ON COLUMN action.runtime IS 'Execution environment for the action';
COMMENT ON COLUMN action.param_schema IS 'JSON schema for action input parameters';
COMMENT ON COLUMN action.out_schema IS 'JSON schema for action output/results';

-- Add foreign key constraints that reference action table
ALTER TABLE policy
    ADD CONSTRAINT policy_action_fkey
    FOREIGN KEY (action) REFERENCES action(id) ON DELETE CASCADE;

ALTER TABLE key
    ADD CONSTRAINT key_owner_action_fkey
    FOREIGN KEY (owner_action) REFERENCES action(id) ON DELETE CASCADE;

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
    action BIGINT NOT NULL REFERENCES action(id),
    action_ref TEXT NOT NULL,
    trigger BIGINT NOT NULL REFERENCES trigger(id),
    trigger_ref TEXT NOT NULL,
    conditions JSONB NOT NULL DEFAULT '[]'::jsonb,
    action_params JSONB DEFAULT '{}'::jsonb,
    trigger_params JSONB DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL,
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

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON rule TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE rule_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE rule IS 'Rules connect triggers to actions with conditional logic';
COMMENT ON COLUMN rule.ref IS 'Unique rule reference (format: pack.name)';
COMMENT ON COLUMN rule.label IS 'Human-readable rule name';
COMMENT ON COLUMN rule.action IS 'Action to execute when rule conditions are met';
COMMENT ON COLUMN rule.trigger IS 'Trigger that activates this rule';
COMMENT ON COLUMN rule.conditions IS 'JSON array of condition expressions';
COMMENT ON COLUMN rule.action_params IS 'JSON object of parameters to pass to the action when rule is triggered';
COMMENT ON COLUMN rule.trigger_params IS 'JSON object of parameters for trigger configuration and event filtering';
COMMENT ON COLUMN rule.enabled IS 'Whether this rule is active';

-- Add foreign key constraint to enforcement table
ALTER TABLE enforcement
    ADD CONSTRAINT enforcement_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;

-- ============================================================================
-- EXECUTION TABLE
-- ============================================================================

CREATE TABLE execution (
    id BIGSERIAL PRIMARY KEY,
    action BIGINT REFERENCES action(id),
    action_ref TEXT NOT NULL,
    config JSONB,
    parent BIGINT REFERENCES execution(id),
    enforcement BIGINT REFERENCES enforcement(id),
    executor BIGINT REFERENCES identity(id) ON DELETE SET NULL,
    status execution_status_enum NOT NULL DEFAULT 'requested',
    result JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_execution_action ON execution(action);
CREATE INDEX idx_execution_action_ref ON execution(action_ref);
CREATE INDEX idx_execution_parent ON execution(parent);
CREATE INDEX idx_execution_enforcement ON execution(enforcement);
CREATE INDEX idx_execution_executor ON execution(executor);
CREATE INDEX idx_execution_status ON execution(status);
CREATE INDEX idx_execution_created ON execution(created DESC);
CREATE INDEX idx_execution_updated ON execution(updated DESC);
CREATE INDEX idx_execution_status_created ON execution(status, created DESC);
CREATE INDEX idx_execution_status_updated ON execution(status, updated DESC);
CREATE INDEX idx_execution_action_status ON execution(action, status);
CREATE INDEX idx_execution_executor_created ON execution(executor, created DESC);
CREATE INDEX idx_execution_parent_created ON execution(parent, created DESC);
CREATE INDEX idx_execution_result_gin ON execution USING GIN (result);

-- Trigger
CREATE TRIGGER update_execution_updated
    BEFORE UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON execution TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE execution_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE execution IS 'Executions represent action runs, supports nested workflows';
COMMENT ON COLUMN execution.action IS 'Action being executed (may be null if action deleted)';
COMMENT ON COLUMN execution.action_ref IS 'Action reference (preserved even if action deleted)';
COMMENT ON COLUMN execution.config IS 'Snapshot of action configuration at execution time';
COMMENT ON COLUMN execution.parent IS 'Parent execution ID for workflow hierarchies';
COMMENT ON COLUMN execution.enforcement IS 'Enforcement that triggered this execution (if rule-driven)';
COMMENT ON COLUMN execution.executor IS 'Identity that initiated the execution';
COMMENT ON COLUMN execution.status IS 'Current execution lifecycle status';
COMMENT ON COLUMN execution.result IS 'Execution output/results';

-- ============================================================================
-- INQUIRY TABLE
-- ============================================================================

CREATE TABLE inquiry (
    id BIGSERIAL PRIMARY KEY,
    execution BIGINT NOT NULL REFERENCES execution(id) ON DELETE CASCADE,
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

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON inquiry TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE inquiry_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE inquiry IS 'Inquiries enable human-in-the-loop workflows with async user interactions';
COMMENT ON COLUMN inquiry.execution IS 'Execution that is waiting on this inquiry';
COMMENT ON COLUMN inquiry.prompt IS 'Question or prompt text for the user';
COMMENT ON COLUMN inquiry.response_schema IS 'JSON schema defining expected response format';
COMMENT ON COLUMN inquiry.assigned_to IS 'Identity who should respond to this inquiry';
COMMENT ON COLUMN inquiry.status IS 'Current inquiry lifecycle status';
COMMENT ON COLUMN inquiry.response IS 'User response data';
COMMENT ON COLUMN inquiry.timeout_at IS 'When this inquiry expires';
COMMENT ON COLUMN inquiry.responded_at IS 'When the response was received';

-- ============================================================================
-- WORKFLOW DEFINITION TABLE
-- ============================================================================

CREATE TABLE workflow_definition (
    id BIGSERIAL PRIMARY KEY,
    ref VARCHAR(255) NOT NULL UNIQUE,
    pack BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref VARCHAR(255) NOT NULL,
    label VARCHAR(255) NOT NULL,
    description TEXT,
    version VARCHAR(50) NOT NULL,
    param_schema JSONB,
    out_schema JSONB,
    definition JSONB NOT NULL,
    tags TEXT[] DEFAULT '{}',
    enabled BOOLEAN DEFAULT true NOT NULL,
    created TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Indexes
CREATE INDEX idx_workflow_def_pack ON workflow_definition(pack);
CREATE INDEX idx_workflow_def_enabled ON workflow_definition(enabled);
CREATE INDEX idx_workflow_def_ref ON workflow_definition(ref);
CREATE INDEX idx_workflow_def_tags ON workflow_definition USING gin(tags);

-- Trigger
CREATE TRIGGER update_workflow_definition_updated
    BEFORE UPDATE ON workflow_definition
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_definition TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE workflow_definition_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE workflow_definition IS 'Stores workflow definitions (YAML parsed to JSON)';
COMMENT ON COLUMN workflow_definition.ref IS 'Unique workflow reference (e.g., pack_name.workflow_name)';
COMMENT ON COLUMN workflow_definition.definition IS 'Complete workflow specification including tasks, variables, and transitions';
COMMENT ON COLUMN workflow_definition.param_schema IS 'JSON schema for workflow input parameters';
COMMENT ON COLUMN workflow_definition.out_schema IS 'JSON schema for workflow output';

-- ============================================================================
-- WORKFLOW EXECUTION TABLE
-- ============================================================================

CREATE TABLE workflow_execution (
    id BIGSERIAL PRIMARY KEY,
    execution BIGINT NOT NULL REFERENCES execution(id) ON DELETE CASCADE,
    workflow_def BIGINT NOT NULL REFERENCES workflow_definition(id),
    current_tasks TEXT[] DEFAULT '{}',
    completed_tasks TEXT[] DEFAULT '{}',
    failed_tasks TEXT[] DEFAULT '{}',
    skipped_tasks TEXT[] DEFAULT '{}',
    variables JSONB DEFAULT '{}',
    task_graph JSONB NOT NULL,
    status execution_status_enum NOT NULL DEFAULT 'requested',
    error_message TEXT,
    paused BOOLEAN DEFAULT false NOT NULL,
    pause_reason TEXT,
    created TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Indexes
CREATE INDEX idx_workflow_exec_execution ON workflow_execution(execution);
CREATE INDEX idx_workflow_exec_workflow_def ON workflow_execution(workflow_def);
CREATE INDEX idx_workflow_exec_status ON workflow_execution(status);
CREATE INDEX idx_workflow_exec_paused ON workflow_execution(paused) WHERE paused = true;

-- Trigger
CREATE TRIGGER update_workflow_execution_updated
    BEFORE UPDATE ON workflow_execution
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_execution TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE workflow_execution_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE workflow_execution IS 'Runtime state tracking for workflow executions';
COMMENT ON COLUMN workflow_execution.variables IS 'Workflow-scoped variables, updated via publish directives';
COMMENT ON COLUMN workflow_execution.task_graph IS 'Execution graph with dependencies and transitions';
COMMENT ON COLUMN workflow_execution.current_tasks IS 'Array of task names currently executing';
COMMENT ON COLUMN workflow_execution.paused IS 'True if workflow execution is paused (can be resumed)';

-- ============================================================================
-- WORKFLOW TASK EXECUTION TABLE
-- ============================================================================

CREATE TABLE workflow_task_execution (
    id BIGSERIAL PRIMARY KEY,
    workflow_execution BIGINT NOT NULL REFERENCES workflow_execution(id) ON DELETE CASCADE,
    execution BIGINT NOT NULL REFERENCES execution(id) ON DELETE CASCADE,
    task_name VARCHAR(255) NOT NULL,
    task_index INTEGER,
    task_batch INTEGER,
    status execution_status_enum NOT NULL DEFAULT 'requested',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    result JSONB,
    error JSONB,
    retry_count INTEGER DEFAULT 0 NOT NULL,
    max_retries INTEGER DEFAULT 0 NOT NULL,
    next_retry_at TIMESTAMPTZ,
    timeout_seconds INTEGER,
    timed_out BOOLEAN DEFAULT false NOT NULL,
    created TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Indexes
CREATE INDEX idx_wf_task_exec_workflow ON workflow_task_execution(workflow_execution);
CREATE INDEX idx_wf_task_exec_execution ON workflow_task_execution(execution);
CREATE INDEX idx_wf_task_exec_status ON workflow_task_execution(status);
CREATE INDEX idx_wf_task_exec_task_name ON workflow_task_execution(task_name);
CREATE INDEX idx_wf_task_exec_retry ON workflow_task_execution(retry_count) WHERE retry_count > 0;
CREATE INDEX idx_wf_task_exec_timeout ON workflow_task_execution(timed_out) WHERE timed_out = true;

-- Trigger
CREATE TRIGGER update_workflow_task_execution_updated
    BEFORE UPDATE ON workflow_task_execution
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_task_execution TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE workflow_task_execution_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE workflow_task_execution IS 'Individual task executions within workflows';
COMMENT ON COLUMN workflow_task_execution.task_index IS 'Index for with-items iteration tasks (0-based)';
COMMENT ON COLUMN workflow_task_execution.task_batch IS 'Batch number for batched with-items processing';
COMMENT ON COLUMN workflow_task_execution.duration_ms IS 'Task execution duration in milliseconds';

-- ============================================================================
-- MODIFY ACTION TABLE - Add Workflow Support
-- ============================================================================

ALTER TABLE action
    ADD COLUMN is_workflow BOOLEAN DEFAULT false NOT NULL,
    ADD COLUMN workflow_def BIGINT REFERENCES workflow_definition(id) ON DELETE CASCADE;

CREATE INDEX idx_action_is_workflow ON action(is_workflow) WHERE is_workflow = true;
CREATE INDEX idx_action_workflow_def ON action(workflow_def);

COMMENT ON COLUMN action.is_workflow IS 'True if this action is a workflow (composable action graph)';
COMMENT ON COLUMN action.workflow_def IS 'Reference to workflow definition if is_workflow=true';

-- ============================================================================
-- WORKFLOW VIEWS
-- ============================================================================

CREATE VIEW workflow_execution_summary AS
SELECT
    we.id,
    we.execution,
    wd.ref as workflow_ref,
    wd.label as workflow_label,
    wd.version as workflow_version,
    we.status,
    we.paused,
    array_length(we.current_tasks, 1) as current_task_count,
    array_length(we.completed_tasks, 1) as completed_task_count,
    array_length(we.failed_tasks, 1) as failed_task_count,
    array_length(we.skipped_tasks, 1) as skipped_task_count,
    we.error_message,
    we.created,
    we.updated
FROM workflow_execution we
JOIN workflow_definition wd ON we.workflow_def = wd.id;

COMMENT ON VIEW workflow_execution_summary IS 'Summary view of workflow executions with task counts';

CREATE VIEW workflow_task_detail AS
SELECT
    wte.id,
    wte.workflow_execution,
    we.execution as workflow_execution_id,
    wd.ref as workflow_ref,
    wte.task_name,
    wte.task_index,
    wte.task_batch,
    wte.status,
    wte.retry_count,
    wte.max_retries,
    wte.timed_out,
    wte.duration_ms,
    wte.started_at,
    wte.completed_at,
    wte.created,
    wte.updated
FROM workflow_task_execution wte
JOIN workflow_execution we ON wte.workflow_execution = we.id
JOIN workflow_definition wd ON we.workflow_def = wd.id;

COMMENT ON VIEW workflow_task_detail IS 'Detailed view of task executions with workflow context';

CREATE VIEW workflow_action_link AS
SELECT
    wd.id as workflow_def_id,
    wd.ref as workflow_ref,
    wd.label,
    wd.version,
    wd.enabled,
    a.id as action_id,
    a.ref as action_ref,
    a.pack as pack_id,
    a.pack_ref
FROM workflow_definition wd
LEFT JOIN action a ON a.workflow_def = wd.id AND a.is_workflow = true;

COMMENT ON VIEW workflow_action_link IS 'Links workflow definitions to their corresponding action records';

-- Permissions for views
GRANT SELECT ON workflow_execution_summary TO svc_attune;
GRANT SELECT ON workflow_task_detail TO svc_attune;
GRANT SELECT ON workflow_action_link TO svc_attune;
