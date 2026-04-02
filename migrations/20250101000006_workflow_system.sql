-- Migration: Workflow System
-- Description: Creates workflow_definition and workflow_execution tables
--              (workflow_task_execution consolidated into execution.workflow_task JSONB)
--
--              NOTE: The execution table is converted to a TimescaleDB hypertable in
--              migration 000009. Hypertables cannot be the target of FK constraints,
--              so workflow_execution.execution is a plain BIGINT with no FK.
--              execution.workflow_def also has no FK (added as plain BIGINT in 000005)
--              since execution is a hypertable and FKs from hypertables are only
--              supported for simple cases — we omit it for consistency.
-- Version: 20250101000006

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
    created TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Indexes
CREATE INDEX idx_workflow_def_pack ON workflow_definition(pack);
CREATE INDEX idx_workflow_def_ref ON workflow_definition(ref);
CREATE INDEX idx_workflow_def_tags ON workflow_definition USING gin(tags);

-- Trigger
CREATE TRIGGER update_workflow_definition_updated
    BEFORE UPDATE ON workflow_definition
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

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
    execution BIGINT NOT NULL, -- references execution(id); no FK because execution is a hypertable
    workflow_def BIGINT NOT NULL REFERENCES workflow_definition(id) ON DELETE CASCADE,
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
    updated TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT uq_workflow_execution_execution UNIQUE (execution)
);

-- Indexes
CREATE INDEX idx_workflow_exec_workflow_def ON workflow_execution(workflow_def);
CREATE INDEX idx_workflow_exec_status ON workflow_execution(status);
CREATE INDEX idx_workflow_exec_paused ON workflow_execution(paused) WHERE paused = true;

-- Trigger
CREATE TRIGGER update_workflow_execution_updated
    BEFORE UPDATE ON workflow_execution
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE workflow_execution IS 'Runtime state tracking for workflow executions. execution column has no FK — execution is a hypertable.';
COMMENT ON COLUMN workflow_execution.variables IS 'Workflow-scoped variables, updated via publish directives';
COMMENT ON COLUMN workflow_execution.task_graph IS 'Execution graph with dependencies and transitions';
COMMENT ON COLUMN workflow_execution.current_tasks IS 'Array of task names currently executing';
COMMENT ON COLUMN workflow_execution.paused IS 'True if workflow execution is paused (can be resumed)';

-- ============================================================================
-- MODIFY ACTION TABLE - Add Workflow Support
-- ============================================================================

ALTER TABLE action
    ADD COLUMN workflow_def BIGINT REFERENCES workflow_definition(id) ON DELETE CASCADE;

CREATE INDEX idx_action_workflow_def ON action(workflow_def);

COMMENT ON COLUMN action.workflow_def IS 'Reference to workflow definition (non-null means this action is a workflow)';

-- NOTE: execution.workflow_def has no FK constraint because execution is a
-- TimescaleDB hypertable (converted in migration 000009). The column was
-- created as a plain BIGINT in migration 000005.

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

CREATE VIEW workflow_action_link AS
SELECT
    wd.id as workflow_def_id,
    wd.ref as workflow_ref,
    wd.label,
    wd.version,
    a.id as action_id,
    a.ref as action_ref,
    a.pack as pack_id,
    a.pack_ref
FROM workflow_definition wd
LEFT JOIN action a ON a.workflow_def = wd.id;

COMMENT ON VIEW workflow_action_link IS 'Links workflow definitions to their corresponding action records';
