-- Migration: Add Worker Role
-- Description: Adds worker_role field to distinguish between action workers and sensor workers
-- Version: 20260131000001

-- ============================================================================
-- WORKER ROLE ENUM
-- ============================================================================

DO $$ BEGIN
    CREATE TYPE worker_role_enum AS ENUM ('action', 'sensor', 'hybrid');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE worker_role_enum IS 'Worker role type: action (executes actions), sensor (monitors triggers), or hybrid (both)';

-- ============================================================================
-- ADD WORKER ROLE COLUMN
-- ============================================================================

ALTER TABLE worker
    ADD COLUMN IF NOT EXISTS worker_role worker_role_enum NOT NULL DEFAULT 'action';

-- Create index for efficient role-based queries
CREATE INDEX IF NOT EXISTS idx_worker_role ON worker(worker_role);
CREATE INDEX IF NOT EXISTS idx_worker_role_status ON worker(worker_role, status);

-- Comments
COMMENT ON COLUMN worker.worker_role IS 'Worker role: action (executes actions), sensor (monitors for triggers), or hybrid (both capabilities)';

-- Update existing workers to be action workers (backward compatibility)
UPDATE worker SET worker_role = 'action' WHERE worker_role IS NULL;
