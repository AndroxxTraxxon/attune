-- Migration: Core Tables
-- Description: Creates core tables for packs, runtimes, workers, identity, permissions, policies, and keys
-- Version: 20250101000002


-- ============================================================================
-- PACK TABLE
-- ============================================================================

CREATE TABLE pack (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    description TEXT,
    version TEXT NOT NULL,
    conf_schema JSONB NOT NULL DEFAULT '{}'::jsonb,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    runtime_deps TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    is_standard BOOLEAN NOT NULL DEFAULT FALSE,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT pack_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT pack_ref_format CHECK (ref ~ '^[a-z][a-z0-9_-]+$'),
    CONSTRAINT pack_version_semver CHECK (
        version ~ '^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
    )
);

-- Indexes
CREATE INDEX idx_pack_ref ON pack(ref);
CREATE INDEX idx_pack_created ON pack(created DESC);
CREATE INDEX idx_pack_is_standard ON pack(is_standard) WHERE is_standard = TRUE;
CREATE INDEX idx_pack_is_standard_created ON pack(is_standard, created DESC);
CREATE INDEX idx_pack_version_created ON pack(version, created DESC);
CREATE INDEX idx_pack_config_gin ON pack USING GIN (config);
CREATE INDEX idx_pack_meta_gin ON pack USING GIN (meta);
CREATE INDEX idx_pack_tags_gin ON pack USING GIN (tags);
CREATE INDEX idx_pack_runtime_deps_gin ON pack USING GIN (runtime_deps);

-- Trigger
CREATE TRIGGER update_pack_updated
    BEFORE UPDATE ON pack
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON pack TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE pack_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE pack IS 'Packs bundle related automation components';
COMMENT ON COLUMN pack.ref IS 'Unique pack reference identifier (e.g., "slack", "github")';
COMMENT ON COLUMN pack.label IS 'Human-readable pack name';
COMMENT ON COLUMN pack.version IS 'Semantic version of the pack';
COMMENT ON COLUMN pack.conf_schema IS 'JSON schema for pack configuration';
COMMENT ON COLUMN pack.config IS 'Pack configuration values';
COMMENT ON COLUMN pack.meta IS 'Pack metadata';
COMMENT ON COLUMN pack.runtime_deps IS 'Array of required runtime references';
COMMENT ON COLUMN pack.is_standard IS 'Whether this is a core/built-in pack';

-- ============================================================================
-- RUNTIME TABLE
-- ============================================================================

CREATE TABLE runtime (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT,
    description TEXT,
    runtime_type runtime_type_enum NOT NULL,
    name TEXT NOT NULL,
    distributions JSONB NOT NULL,
    installation JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT runtime_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT runtime_ref_format CHECK (ref ~ '^[^.]+\.(action|sensor)\.[^.]+$')
);

-- Indexes
CREATE INDEX idx_runtime_ref ON runtime(ref);
CREATE INDEX idx_runtime_pack ON runtime(pack);
CREATE INDEX idx_runtime_type ON runtime(runtime_type);
CREATE INDEX idx_runtime_created ON runtime(created DESC);
CREATE INDEX idx_runtime_pack_type ON runtime(pack, runtime_type);
CREATE INDEX idx_runtime_type_created ON runtime(runtime_type, created DESC);

-- Trigger
CREATE TRIGGER update_runtime_updated
    BEFORE UPDATE ON runtime
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON runtime TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE runtime_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE runtime IS 'Runtime environments for executing actions and sensors';
COMMENT ON COLUMN runtime.ref IS 'Unique runtime reference (format: pack.type.name)';
COMMENT ON COLUMN runtime.runtime_type IS 'Type of runtime (action or sensor)';
COMMENT ON COLUMN runtime.name IS 'Runtime name (e.g., "python3.11", "nodejs20")';
COMMENT ON COLUMN runtime.distributions IS 'Available distributions for this runtime';
COMMENT ON COLUMN runtime.installation IS 'Installation requirements and instructions';

-- ============================================================================
-- WORKER TABLE
-- ============================================================================

CREATE TABLE worker (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    worker_type worker_type_enum NOT NULL,
    runtime BIGINT REFERENCES runtime(id),
    host TEXT,
    port INTEGER,
    status worker_status_enum DEFAULT 'inactive',
    capabilities JSONB,
    meta JSONB,
    last_heartbeat TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT worker_port_range CHECK (port IS NULL OR (port > 0 AND port <= 65535))
);

-- Indexes
CREATE INDEX idx_worker_name ON worker(name);
CREATE INDEX idx_worker_type ON worker(worker_type);
CREATE INDEX idx_worker_runtime ON worker(runtime);
CREATE INDEX idx_worker_status ON worker(status);
CREATE INDEX idx_worker_last_heartbeat ON worker(last_heartbeat DESC);
CREATE INDEX idx_worker_status_runtime ON worker(status, runtime);
CREATE INDEX idx_worker_type_status ON worker(worker_type, status);

-- Trigger
CREATE TRIGGER update_worker_updated
    BEFORE UPDATE ON worker
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON worker TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE worker_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE worker IS 'Worker processes that execute actions';
COMMENT ON COLUMN worker.name IS 'Worker identifier';
COMMENT ON COLUMN worker.worker_type IS 'Deployment type (local, remote, container)';
COMMENT ON COLUMN worker.runtime IS 'Associated runtime environment';
COMMENT ON COLUMN worker.status IS 'Current operational status';
COMMENT ON COLUMN worker.capabilities IS 'Worker capabilities and features';
COMMENT ON COLUMN worker.last_heartbeat IS 'Last health check timestamp';

-- ============================================================================
-- IDENTITY TABLE
-- ============================================================================

CREATE TABLE identity (
    id BIGSERIAL PRIMARY KEY,
    login TEXT NOT NULL UNIQUE,
    display_name TEXT,
    password_hash TEXT,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_identity_login ON identity(login);
CREATE INDEX idx_identity_created ON identity(created DESC);
CREATE INDEX idx_identity_password_hash ON identity(password_hash) WHERE password_hash IS NOT NULL;
CREATE INDEX idx_identity_attributes_gin ON identity USING GIN (attributes);

-- Trigger
CREATE TRIGGER update_identity_updated
    BEFORE UPDATE ON identity
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON identity TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE identity_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE identity IS 'Identities represent users or service accounts';
COMMENT ON COLUMN identity.login IS 'Unique login identifier';
COMMENT ON COLUMN identity.display_name IS 'Human-readable name';
COMMENT ON COLUMN identity.password_hash IS 'Argon2 hashed password for authentication (NULL for service accounts or external auth)';
COMMENT ON COLUMN identity.attributes IS 'Custom attributes (email, groups, etc.)';

-- ============================================================================
-- PERMISSION_SET TABLE
-- ============================================================================

CREATE TABLE permission_set (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT,
    label TEXT,
    description TEXT,
    grants JSONB NOT NULL DEFAULT '[]'::jsonb,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT permission_set_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT permission_set_ref_format CHECK (ref ~ '^[^.]+\.[^.]+$')
);

-- Indexes
CREATE INDEX idx_permission_set_ref ON permission_set(ref);
CREATE INDEX idx_permission_set_pack ON permission_set(pack);
CREATE INDEX idx_permission_set_created ON permission_set(created DESC);

-- Trigger
CREATE TRIGGER update_permission_set_updated
    BEFORE UPDATE ON permission_set
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON permission_set TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE permission_set_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE permission_set IS 'Permission sets group permissions together (like roles)';
COMMENT ON COLUMN permission_set.ref IS 'Unique permission set reference (format: pack.name)';
COMMENT ON COLUMN permission_set.label IS 'Human-readable name';
COMMENT ON COLUMN permission_set.grants IS 'Array of permission grants';

-- ============================================================================
-- PERMISSION_ASSIGNMENT TABLE
-- ============================================================================

CREATE TABLE permission_assignment (
    id BIGSERIAL PRIMARY KEY,
    identity BIGINT NOT NULL REFERENCES identity(id) ON DELETE CASCADE,
    permset BIGINT NOT NULL REFERENCES permission_set(id) ON DELETE CASCADE,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Unique constraint to prevent duplicate assignments
    CONSTRAINT unique_identity_permset UNIQUE (identity, permset)
);

-- Indexes
CREATE INDEX idx_permission_assignment_identity ON permission_assignment(identity);
CREATE INDEX idx_permission_assignment_permset ON permission_assignment(permset);
CREATE INDEX idx_permission_assignment_created ON permission_assignment(created DESC);
CREATE INDEX idx_permission_assignment_identity_created ON permission_assignment(identity, created DESC);
CREATE INDEX idx_permission_assignment_permset_created ON permission_assignment(permset, created DESC);

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON permission_assignment TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE permission_assignment_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE permission_assignment IS 'Links identities to permission sets (many-to-many)';
COMMENT ON COLUMN permission_assignment.identity IS 'Identity being granted permissions';
COMMENT ON COLUMN permission_assignment.permset IS 'Permission set being assigned';

-- ============================================================================
-- POLICY TABLE
-- ============================================================================

CREATE TABLE policy (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT,
    action BIGINT, -- Forward reference to action table, will add constraint in next migration
    action_ref TEXT,
    parameters TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    method policy_method_enum NOT NULL,
    threshold INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT policy_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT policy_ref_format CHECK (ref ~ '^[^.]+\.[^.]+$'),
    CONSTRAINT policy_threshold_positive CHECK (threshold > 0)
);

-- Indexes
CREATE INDEX idx_policy_ref ON policy(ref);
CREATE INDEX idx_policy_pack ON policy(pack);
CREATE INDEX idx_policy_action ON policy(action);
CREATE INDEX idx_policy_created ON policy(created DESC);
CREATE INDEX idx_policy_action_created ON policy(action, created DESC);
CREATE INDEX idx_policy_pack_created ON policy(pack, created DESC);
CREATE INDEX idx_policy_parameters_gin ON policy USING GIN (parameters);
CREATE INDEX idx_policy_tags_gin ON policy USING GIN (tags);

-- Trigger
CREATE TRIGGER update_policy_updated
    BEFORE UPDATE ON policy
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON policy TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE policy_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE policy IS 'Policies define execution controls (rate limiting, concurrency)';
COMMENT ON COLUMN policy.ref IS 'Unique policy reference (format: pack.name)';
COMMENT ON COLUMN policy.action IS 'Action this policy applies to';
COMMENT ON COLUMN policy.parameters IS 'Parameter names used for policy grouping';
COMMENT ON COLUMN policy.method IS 'How to handle policy violations (cancel/enqueue)';
COMMENT ON COLUMN policy.threshold IS 'Numeric limit (e.g., max concurrent executions)';

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

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON key TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE key_id_seq TO svc_attune;

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
