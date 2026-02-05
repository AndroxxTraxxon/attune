-- Migration: Keys and Artifacts
-- Description: Creates key table for secrets management and artifact table for execution outputs
-- Version: 20250101000009

-- ============================================================================
-- KEY TABLE
-- ============================================================================

CREATE TABLE key (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    owner_type owner_type_enum NOT NULL,
    owner TEXT,
    owner_identity BIGINT REFERENCES identity(id),
    owner_pack BIGINT REFERENCES pack(id),
    owner_pack_ref TEXT,
    owner_action BIGINT, -- Forward reference to action table
    owner_action_ref TEXT,
    owner_sensor BIGINT, -- Forward reference to sensor table
    owner_sensor_ref TEXT,
    name TEXT NOT NULL,
    encrypted BOOLEAN NOT NULL,
    encryption_key_hash TEXT,
    value TEXT NOT NULL,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT key_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT key_ref_format CHECK (ref ~ '^([^.]+\.)?[^.]+$')
);

-- Unique index on owner_type, owner, name
CREATE UNIQUE INDEX idx_key_unique ON key(owner_type, owner, name);

-- Indexes
CREATE INDEX idx_key_ref ON key(ref);
CREATE INDEX idx_key_owner_type ON key(owner_type);
CREATE INDEX idx_key_owner_identity ON key(owner_identity);
CREATE INDEX idx_key_owner_pack ON key(owner_pack);
CREATE INDEX idx_key_owner_action ON key(owner_action);
CREATE INDEX idx_key_owner_sensor ON key(owner_sensor);
CREATE INDEX idx_key_created ON key(created DESC);
CREATE INDEX idx_key_owner_type_owner ON key(owner_type, owner);
CREATE INDEX idx_key_owner_identity_name ON key(owner_identity, name);
CREATE INDEX idx_key_owner_pack_name ON key(owner_pack, name);

-- Function to validate and set owner fields
CREATE OR REPLACE FUNCTION validate_key_owner()
RETURNS TRIGGER AS $$
DECLARE
    owner_count INTEGER := 0;
BEGIN
    -- Count how many owner fields are set
    IF NEW.owner_identity IS NOT NULL THEN owner_count := owner_count + 1; END IF;
    IF NEW.owner_pack IS NOT NULL THEN owner_count := owner_count + 1; END IF;
    IF NEW.owner_action IS NOT NULL THEN owner_count := owner_count + 1; END IF;
    IF NEW.owner_sensor IS NOT NULL THEN owner_count := owner_count + 1; END IF;

    -- System owner should have no owner fields set
    IF NEW.owner_type = 'system' THEN
        IF owner_count > 0 THEN
            RAISE EXCEPTION 'System owner cannot have specific owner fields set';
        END IF;
        NEW.owner := 'system';
    -- All other types must have exactly one owner field set
    ELSIF owner_count != 1 THEN
        RAISE EXCEPTION 'Exactly one owner field must be set for owner_type %', NEW.owner_type;
    -- Validate owner_type matches the populated field and set owner
    ELSIF NEW.owner_type = 'identity' THEN
        IF NEW.owner_identity IS NULL THEN
            RAISE EXCEPTION 'owner_identity must be set for owner_type identity';
        END IF;
        NEW.owner := NEW.owner_identity::TEXT;
    ELSIF NEW.owner_type = 'pack' THEN
        IF NEW.owner_pack IS NULL THEN
            RAISE EXCEPTION 'owner_pack must be set for owner_type pack';
        END IF;
        NEW.owner := NEW.owner_pack::TEXT;
    ELSIF NEW.owner_type = 'action' THEN
        IF NEW.owner_action IS NULL THEN
            RAISE EXCEPTION 'owner_action must be set for owner_type action';
        END IF;
        NEW.owner := NEW.owner_action::TEXT;
    ELSIF NEW.owner_type = 'sensor' THEN
        IF NEW.owner_sensor IS NULL THEN
            RAISE EXCEPTION 'owner_sensor must be set for owner_type sensor';
        END IF;
        NEW.owner := NEW.owner_sensor::TEXT;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to validate owner fields
CREATE TRIGGER validate_key_owner_trigger
    BEFORE INSERT OR UPDATE ON key
    FOR EACH ROW
    EXECUTE FUNCTION validate_key_owner();

-- Trigger for updated timestamp
CREATE TRIGGER update_key_updated
    BEFORE UPDATE ON key
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE key IS 'Keys store configuration values and secrets with ownership scoping';
COMMENT ON COLUMN key.ref IS 'Unique key reference (format: [owner.]name)';
COMMENT ON COLUMN key.owner_type IS 'Type of owner (system, identity, pack, action, sensor)';
COMMENT ON COLUMN key.owner IS 'Owner identifier (auto-populated by trigger)';
COMMENT ON COLUMN key.owner_identity IS 'Identity owner (if owner_type=identity)';
COMMENT ON COLUMN key.owner_pack IS 'Pack owner (if owner_type=pack)';
COMMENT ON COLUMN key.owner_pack_ref IS 'Pack reference for owner_pack';
COMMENT ON COLUMN key.owner_action IS 'Action owner (if owner_type=action)';
COMMENT ON COLUMN key.owner_sensor IS 'Sensor owner (if owner_type=sensor)';
COMMENT ON COLUMN key.name IS 'Key name within owner scope';
COMMENT ON COLUMN key.encrypted IS 'Whether the value is encrypted';
COMMENT ON COLUMN key.encryption_key_hash IS 'Hash of encryption key used';
COMMENT ON COLUMN key.value IS 'The actual value (encrypted if encrypted=true)';


-- Add foreign key constraints for action and sensor references
ALTER TABLE key
    ADD CONSTRAINT key_owner_action_fkey
    FOREIGN KEY (owner_action) REFERENCES action(id) ON DELETE CASCADE;

ALTER TABLE key
    ADD CONSTRAINT key_owner_sensor_fkey
    FOREIGN KEY (owner_sensor) REFERENCES sensor(id) ON DELETE CASCADE;

-- ============================================================================
-- ARTIFACT TABLE
-- ============================================================================

CREATE TABLE artifact (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL,
    scope owner_type_enum NOT NULL DEFAULT 'system',
    owner TEXT NOT NULL DEFAULT '',
    type artifact_type_enum NOT NULL,
    retention_policy artifact_retention_enum NOT NULL DEFAULT 'versions',
    retention_limit INTEGER NOT NULL DEFAULT 1,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_artifact_ref ON artifact(ref);
CREATE INDEX idx_artifact_scope ON artifact(scope);
CREATE INDEX idx_artifact_owner ON artifact(owner);
CREATE INDEX idx_artifact_type ON artifact(type);
CREATE INDEX idx_artifact_created ON artifact(created DESC);
CREATE INDEX idx_artifact_scope_owner ON artifact(scope, owner);
CREATE INDEX idx_artifact_type_created ON artifact(type, created DESC);

-- Trigger
CREATE TRIGGER update_artifact_updated
    BEFORE UPDATE ON artifact
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE artifact IS 'Artifacts track files, logs, and outputs from executions';
COMMENT ON COLUMN artifact.ref IS 'Artifact reference/path';
COMMENT ON COLUMN artifact.scope IS 'Owner type (system, identity, pack, action, sensor)';
COMMENT ON COLUMN artifact.owner IS 'Owner identifier';
COMMENT ON COLUMN artifact.type IS 'Artifact type (file, url, progress, etc.)';
COMMENT ON COLUMN artifact.retention_policy IS 'How to retain artifacts (versions, days, hours, minutes)';
COMMENT ON COLUMN artifact.retention_limit IS 'Numeric limit for retention policy';

-- ============================================================================
-- QUEUE_STATS TABLE
-- ============================================================================

CREATE TABLE queue_stats (
    action_id BIGINT PRIMARY KEY REFERENCES action(id) ON DELETE CASCADE,
    queue_length INTEGER NOT NULL DEFAULT 0,
    active_count INTEGER NOT NULL DEFAULT 0,
    max_concurrent INTEGER NOT NULL DEFAULT 1,
    oldest_enqueued_at TIMESTAMPTZ,
    total_enqueued BIGINT NOT NULL DEFAULT 0,
    total_completed BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_queue_stats_last_updated ON queue_stats(last_updated);

-- Comments
COMMENT ON TABLE queue_stats IS 'Real-time queue statistics for action execution ordering';
COMMENT ON COLUMN queue_stats.action_id IS 'Foreign key to action table';
COMMENT ON COLUMN queue_stats.queue_length IS 'Number of executions waiting in queue';
COMMENT ON COLUMN queue_stats.active_count IS 'Number of currently running executions';
COMMENT ON COLUMN queue_stats.max_concurrent IS 'Maximum concurrent executions allowed';
COMMENT ON COLUMN queue_stats.oldest_enqueued_at IS 'Timestamp of oldest queued execution (NULL if queue empty)';
COMMENT ON COLUMN queue_stats.total_enqueued IS 'Total executions enqueued since queue creation';
COMMENT ON COLUMN queue_stats.total_completed IS 'Total executions completed since queue creation';
COMMENT ON COLUMN queue_stats.last_updated IS 'Timestamp of last statistics update';
