-- Migration: Action
-- Description: Creates action table (with is_adhoc from start)
-- Version: 20250101000005

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
    parameter_delivery TEXT NOT NULL DEFAULT 'stdin' CHECK (parameter_delivery IN ('stdin', 'file')),
    parameter_format TEXT NOT NULL DEFAULT 'json' CHECK (parameter_format IN ('dotenv', 'json', 'yaml')),
    output_format TEXT NOT NULL DEFAULT 'text' CHECK (output_format IN ('text', 'json', 'yaml', 'jsonl')),
    is_adhoc BOOLEAN NOT NULL DEFAULT FALSE,
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
CREATE INDEX idx_action_parameter_delivery ON action(parameter_delivery);
CREATE INDEX idx_action_parameter_format ON action(parameter_format);
CREATE INDEX idx_action_output_format ON action(output_format);
CREATE INDEX idx_action_is_adhoc ON action(is_adhoc) WHERE is_adhoc = true;
CREATE INDEX idx_action_created ON action(created DESC);

-- Trigger
CREATE TRIGGER update_action_updated
    BEFORE UPDATE ON action
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE action IS 'Actions are executable tasks that can be triggered';
COMMENT ON COLUMN action.ref IS 'Unique action reference (format: pack.name)';
COMMENT ON COLUMN action.pack IS 'Pack this action belongs to';
COMMENT ON COLUMN action.label IS 'Human-readable action name';
COMMENT ON COLUMN action.entrypoint IS 'Script or command to execute';
COMMENT ON COLUMN action.runtime IS 'Runtime environment for execution';
COMMENT ON COLUMN action.param_schema IS 'JSON schema for action parameters';
COMMENT ON COLUMN action.out_schema IS 'JSON schema for action output';
COMMENT ON COLUMN action.parameter_delivery IS 'How parameters are delivered: stdin (standard input - secure), file (temporary file - secure for large payloads). Environment variables are set separately via execution.env_vars.';
COMMENT ON COLUMN action.parameter_format IS 'Parameter serialization format: json (JSON object - default), dotenv (KEY=''VALUE''), yaml (YAML format)';
COMMENT ON COLUMN action.output_format IS 'Output parsing format: text (no parsing - raw stdout), json (parse stdout as JSON), yaml (parse stdout as YAML), jsonl (parse each line as JSON, collect into array)';
COMMENT ON COLUMN action.is_adhoc IS 'True if action was manually created (ad-hoc), false if installed from pack';

-- ============================================================================

-- Add foreign key constraint for policy table
ALTER TABLE policy
    ADD CONSTRAINT policy_action_fkey
    FOREIGN KEY (action) REFERENCES action(id) ON DELETE CASCADE;

-- Note: Foreign key constraints for key table (key_owner_action_fkey, key_owner_sensor_fkey)
-- will be added in migration 20250101000009_keys_artifacts.sql after the key table is created
