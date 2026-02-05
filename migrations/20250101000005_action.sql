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
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT action_ref_lowercase CHECK (ref = LOWER(ref)),
    CONSTRAINT action_ref_format CHECK (ref ~ '^[^.]+\.[^.]+$')
);

-- ============================================================================

-- Add foreign key constraint for policy table
ALTER TABLE policy
    ADD CONSTRAINT policy_action_fkey
    FOREIGN KEY (action) REFERENCES action(id) ON DELETE CASCADE;

-- Note: Foreign key constraints for key table (key_owner_action_fkey, key_owner_sensor_fkey)
-- will be added in migration 20250101000009_keys_artifacts.sql after the key table is created
