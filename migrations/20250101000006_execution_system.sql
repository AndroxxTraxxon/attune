-- Migration: Execution System
-- Description: Creates execution (with workflow columns) and inquiry tables
-- Version: 20250101000006

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
    is_workflow BOOLEAN DEFAULT false NOT NULL,
    workflow_def BIGINT,
    workflow_task JSONB,
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
