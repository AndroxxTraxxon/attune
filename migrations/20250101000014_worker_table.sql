-- Migration: Worker Table
-- Description: Creates worker table for tracking worker registration and heartbeat
-- Version: 20250101000014

-- ============================================================================
-- WORKER TABLE
-- ============================================================================

CREATE TABLE worker (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    worker_type worker_type_enum NOT NULL,
    worker_role worker_role_enum NOT NULL,
    runtime BIGINT REFERENCES runtime(id) ON DELETE SET NULL,
    host TEXT,
    port INTEGER,
    status worker_status_enum NOT NULL DEFAULT 'active',
    capabilities JSONB,
    meta JSONB,
    last_heartbeat TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_worker_name ON worker(name);
CREATE INDEX idx_worker_type ON worker(worker_type);
CREATE INDEX idx_worker_role ON worker(worker_role);
CREATE INDEX idx_worker_runtime ON worker(runtime);
CREATE INDEX idx_worker_status ON worker(status);
CREATE INDEX idx_worker_last_heartbeat ON worker(last_heartbeat DESC) WHERE last_heartbeat IS NOT NULL;
CREATE INDEX idx_worker_created ON worker(created DESC);
CREATE INDEX idx_worker_status_role ON worker(status, worker_role);
CREATE INDEX idx_worker_capabilities_gin ON worker USING GIN (capabilities);
CREATE INDEX idx_worker_meta_gin ON worker USING GIN (meta);

-- Trigger
CREATE TRIGGER update_worker_updated
    BEFORE UPDATE ON worker
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE worker IS 'Worker registration and tracking table for action and sensor workers';
COMMENT ON COLUMN worker.name IS 'Unique worker identifier (typically hostname-based)';
COMMENT ON COLUMN worker.worker_type IS 'Worker deployment type (local or remote)';
COMMENT ON COLUMN worker.worker_role IS 'Worker role (action or sensor)';
COMMENT ON COLUMN worker.runtime IS 'Runtime environment this worker supports (optional)';
COMMENT ON COLUMN worker.host IS 'Worker host address';
COMMENT ON COLUMN worker.port IS 'Worker port number';
COMMENT ON COLUMN worker.status IS 'Worker operational status';
COMMENT ON COLUMN worker.capabilities IS 'Worker capabilities (e.g., max_concurrent_executions, supported runtimes)';
COMMENT ON COLUMN worker.meta IS 'Additional worker metadata';
COMMENT ON COLUMN worker.last_heartbeat IS 'Timestamp of last heartbeat from worker';

-- ============================================================================
