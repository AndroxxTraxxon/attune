-- Phase 3: Retry Tracking and Action Timeout Configuration
-- This migration adds support for:
-- 1. Retry tracking on executions (attempt count, max attempts, retry reason)
-- 2. Action-level timeout configuration
-- 3. Worker health metrics

-- Add retry tracking fields to execution table
ALTER TABLE execution
ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0,
ADD COLUMN max_retries INTEGER,
ADD COLUMN retry_reason TEXT,
ADD COLUMN original_execution BIGINT REFERENCES execution(id) ON DELETE SET NULL;

-- Add index for finding retry chains
CREATE INDEX idx_execution_original_execution ON execution(original_execution) WHERE original_execution IS NOT NULL;

-- Add timeout configuration to action table
ALTER TABLE action
ADD COLUMN timeout_seconds INTEGER,
ADD COLUMN max_retries INTEGER DEFAULT 0;

-- Add comment explaining timeout behavior
COMMENT ON COLUMN action.timeout_seconds IS 'Worker queue TTL override in seconds. If NULL, uses global worker_queue_ttl_ms config. Allows per-action timeout tuning.';
COMMENT ON COLUMN action.max_retries IS 'Maximum number of automatic retry attempts for failed executions. 0 = no retries (default).';
COMMENT ON COLUMN execution.retry_count IS 'Current retry attempt number (0 = first attempt, 1 = first retry, etc.)';
COMMENT ON COLUMN execution.max_retries IS 'Maximum retries for this execution. Copied from action.max_retries at creation time.';
COMMENT ON COLUMN execution.retry_reason IS 'Reason for retry (e.g., "worker_unavailable", "transient_error", "manual_retry")';
COMMENT ON COLUMN execution.original_execution IS 'ID of the original execution if this is a retry. Forms a retry chain.';

-- Add worker health tracking fields
-- These are stored in the capabilities JSONB field as a "health" object:
-- {
--   "runtimes": [...],
--   "health": {
--     "status": "healthy|degraded|unhealthy",
--     "last_check": "2026-02-09T12:00:00Z",
--     "consecutive_failures": 0,
--     "total_executions": 100,
--     "failed_executions": 2,
--     "average_execution_time_ms": 1500,
--     "queue_depth": 5
--   }
-- }

-- Add index for health-based queries (using JSONB path operators)
CREATE INDEX idx_worker_capabilities_health_status ON worker
USING GIN ((capabilities -> 'health' -> 'status'));

-- Add view for healthy workers (convenience for queries)
CREATE OR REPLACE VIEW healthy_workers AS
SELECT
    w.id,
    w.name,
    w.worker_type,
    w.worker_role,
    w.runtime,
    w.status,
    w.capabilities,
    w.last_heartbeat,
    (w.capabilities -> 'health' ->> 'status')::TEXT as health_status,
    (w.capabilities -> 'health' ->> 'queue_depth')::INTEGER as queue_depth,
    (w.capabilities -> 'health' ->> 'consecutive_failures')::INTEGER as consecutive_failures
FROM worker w
WHERE
    w.status = 'active'
    AND w.last_heartbeat > NOW() - INTERVAL '30 seconds'
    AND (
        -- Healthy if no health info (backward compatible)
        w.capabilities -> 'health' IS NULL
        OR
        -- Or explicitly marked healthy
        w.capabilities -> 'health' ->> 'status' IN ('healthy', 'degraded')
    );

COMMENT ON VIEW healthy_workers IS 'Workers that are active, have fresh heartbeat, and are healthy or degraded (not unhealthy)';

-- Add function to get worker queue depth estimate
CREATE OR REPLACE FUNCTION get_worker_queue_depth(worker_id_param BIGINT)
RETURNS INTEGER AS $$
BEGIN
    -- Extract queue depth from capabilities.health.queue_depth
    -- Returns NULL if not available
    RETURN (
        SELECT (capabilities -> 'health' ->> 'queue_depth')::INTEGER
        FROM worker
        WHERE id = worker_id_param
    );
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_worker_queue_depth IS 'Extract current queue depth from worker health metadata';

-- Add function to check if execution is retriable
CREATE OR REPLACE FUNCTION is_execution_retriable(execution_id_param BIGINT)
RETURNS BOOLEAN AS $$
DECLARE
    exec_record RECORD;
BEGIN
    SELECT
        e.retry_count,
        e.max_retries,
        e.status
    INTO exec_record
    FROM execution e
    WHERE e.id = execution_id_param;

    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;

    -- Can retry if:
    -- 1. Status is failed
    -- 2. max_retries is set and > 0
    -- 3. retry_count < max_retries
    RETURN (
        exec_record.status = 'failed'
        AND exec_record.max_retries IS NOT NULL
        AND exec_record.max_retries > 0
        AND exec_record.retry_count < exec_record.max_retries
    );
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION is_execution_retriable IS 'Check if a failed execution can be automatically retried based on retry limits';

-- Add indexes for retry queries
CREATE INDEX idx_execution_status_retry ON execution(status, retry_count) WHERE status = 'failed' AND retry_count < COALESCE(max_retries, 0);
