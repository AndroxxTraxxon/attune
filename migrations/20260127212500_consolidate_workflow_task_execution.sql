-- Migration: Consolidate workflow_task_execution into execution table
-- Description: Adds workflow_task JSONB column to execution table and migrates data from workflow_task_execution
-- Version: 20260127212500


-- ============================================================================
-- STEP 1: Add workflow_task column to execution table
-- ============================================================================

ALTER TABLE execution ADD COLUMN workflow_task JSONB;

COMMENT ON COLUMN execution.workflow_task IS 'Workflow task metadata (only populated for workflow task executions)';

-- ============================================================================
-- STEP 2: Migrate existing workflow_task_execution data to execution.workflow_task
-- ============================================================================

-- Update execution records with workflow task metadata
UPDATE execution e
SET workflow_task = jsonb_build_object(
    'workflow_execution', wte.workflow_execution,
    'task_name', wte.task_name,
    'task_index', wte.task_index,
    'task_batch', wte.task_batch,
    'retry_count', wte.retry_count,
    'max_retries', wte.max_retries,
    'next_retry_at', to_char(wte.next_retry_at, 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
    'timeout_seconds', wte.timeout_seconds,
    'timed_out', wte.timed_out,
    'duration_ms', wte.duration_ms,
    'started_at', to_char(wte.started_at, 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
    'completed_at', to_char(wte.completed_at, 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
)
FROM workflow_task_execution wte
WHERE e.id = wte.execution;

-- ============================================================================
-- STEP 3: Create indexes for efficient JSONB queries
-- ============================================================================

-- General GIN index for JSONB operations
CREATE INDEX idx_execution_workflow_task_gin ON execution USING GIN (workflow_task)
WHERE workflow_task IS NOT NULL;

-- Specific index for workflow_execution lookups (most common query)
CREATE INDEX idx_execution_workflow_execution ON execution ((workflow_task->>'workflow_execution'))
WHERE workflow_task IS NOT NULL;

-- Index for task name lookups
CREATE INDEX idx_execution_task_name ON execution ((workflow_task->>'task_name'))
WHERE workflow_task IS NOT NULL;

-- Index for retry queries (using text comparison to avoid IMMUTABLE issue)
CREATE INDEX idx_execution_pending_retries ON execution ((workflow_task->>'next_retry_at'))
WHERE workflow_task IS NOT NULL
  AND workflow_task->>'next_retry_at' IS NOT NULL;

-- Index for timeout queries
CREATE INDEX idx_execution_timed_out ON execution ((workflow_task->>'timed_out'))
WHERE workflow_task IS NOT NULL;

-- Index for workflow task status queries (combined with execution status)
CREATE INDEX idx_execution_workflow_status ON execution (status, (workflow_task->>'workflow_execution'))
WHERE workflow_task IS NOT NULL;

-- ============================================================================
-- STEP 4: Drop the workflow_task_execution table
-- ============================================================================

-- Drop the old table (this will cascade delete any dependent objects)
DROP TABLE IF EXISTS workflow_task_execution CASCADE;

-- ============================================================================
-- STEP 5: Update comments and documentation
-- ============================================================================

COMMENT ON INDEX idx_execution_workflow_task_gin IS 'GIN index for general JSONB queries on workflow_task';
COMMENT ON INDEX idx_execution_workflow_execution IS 'Index for finding tasks by workflow execution ID';
COMMENT ON INDEX idx_execution_task_name IS 'Index for finding tasks by name';
COMMENT ON INDEX idx_execution_pending_retries IS 'Index for finding tasks pending retry';
COMMENT ON INDEX idx_execution_timed_out IS 'Index for finding timed out tasks';
COMMENT ON INDEX idx_execution_workflow_status IS 'Index for workflow task status queries';

-- ============================================================================
-- VERIFICATION QUERIES (for manual testing)
-- ============================================================================

-- Verify migration: Count workflow task executions
-- SELECT COUNT(*) FROM execution WHERE workflow_task IS NOT NULL;

-- Verify indexes exist
-- SELECT indexname, indexdef FROM pg_indexes WHERE tablename = 'execution' AND indexname LIKE '%workflow%';

-- Test workflow task queries
-- SELECT * FROM execution WHERE workflow_task->>'workflow_execution' = '1';
-- SELECT * FROM execution WHERE workflow_task->>'task_name' = 'example_task';
-- SELECT * FROM execution WHERE (workflow_task->>'timed_out')::boolean = true;
