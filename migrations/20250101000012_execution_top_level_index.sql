-- Migration: Execution top-level listing index
-- Description: Adds a partial index for the common top-level execution list path
--              (`WHERE parent IS NULL ORDER BY created DESC`) so both the count
--              and page query can scan a narrower structure.
-- Version: 20250101000012

CREATE INDEX IF NOT EXISTS idx_execution_top_level_created
    ON execution (created DESC)
    WHERE parent IS NULL;
