-- Migration: Identity and Authentication
-- Description: Creates identity, permission, and policy tables
-- Version: 20250101000002

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

-- Comments
COMMENT ON TABLE identity IS 'Identities represent users or service accounts';
COMMENT ON COLUMN identity.login IS 'Unique login identifier';
COMMENT ON COLUMN identity.display_name IS 'Human-readable name';
COMMENT ON COLUMN identity.password_hash IS 'Argon2 hashed password for authentication (NULL for service accounts or external auth)';
COMMENT ON COLUMN identity.attributes IS 'Custom attributes (email, groups, etc.)';

-- ============================================================================
-- ADD FOREIGN KEY CONSTRAINTS TO EXISTING TABLES
-- ============================================================================

-- Add foreign key constraint for pack.installed_by now that identity table exists
ALTER TABLE pack
    ADD CONSTRAINT fk_pack_installed_by
    FOREIGN KEY (installed_by)
    REFERENCES identity(id)
    ON DELETE SET NULL;

-- ============================================================================

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

-- Comments
COMMENT ON TABLE permission_set IS 'Permission sets group permissions together (like roles)';
COMMENT ON COLUMN permission_set.ref IS 'Unique permission set reference (format: pack.name)';
COMMENT ON COLUMN permission_set.label IS 'Human-readable name';
COMMENT ON COLUMN permission_set.grants IS 'Array of permission grants';

-- ============================================================================

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

-- Comments
COMMENT ON TABLE permission_assignment IS 'Links identities to permission sets (many-to-many)';
COMMENT ON COLUMN permission_assignment.identity IS 'Identity being granted permissions';
COMMENT ON COLUMN permission_assignment.permset IS 'Permission set being assigned';

-- ============================================================================

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

-- Comments
COMMENT ON TABLE policy IS 'Policies define execution controls (rate limiting, concurrency)';
COMMENT ON COLUMN policy.ref IS 'Unique policy reference (format: pack.name)';
COMMENT ON COLUMN policy.action IS 'Action this policy applies to';
COMMENT ON COLUMN policy.parameters IS 'Parameter names used for policy grouping';
COMMENT ON COLUMN policy.method IS 'How to handle policy violations (cancel/enqueue)';
COMMENT ON COLUMN policy.threshold IS 'Numeric limit (e.g., max concurrent executions)';

-- ============================================================================
