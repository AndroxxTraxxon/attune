-- Migration: Execution System
-- Description: Creates execution (with workflow columns), inquiry, and rule tables
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
COMMENT ON COLUMN rule.action IS 'Action to execute when rule triggers';
COMMENT ON COLUMN rule.trigger IS 'Trigger that activates this rule';
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
