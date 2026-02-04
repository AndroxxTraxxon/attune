-- Migration: Fix webhook function overload issue
-- Description: Drop the old enable_trigger_webhook(bigint) signature to resolve
--              "function is not unique" error when the newer version with config
--              parameter is present.
-- Date: 2026-01-29

-- Drop the old function signature from 20260120000001_add_webhook_support.sql
-- The newer version with JSONB config parameter should be the only one
DROP FUNCTION IF EXISTS enable_trigger_webhook(BIGINT);

-- The new signature with config parameter is already defined in
-- 20260127000001_consolidate_webhook_config.sql:
-- attune.enable_trigger_webhook(p_trigger_id BIGINT, p_config JSONB DEFAULT '{}'::jsonb)

-- Similarly, check and clean up any other webhook function overloads

-- Drop old disable_trigger_webhook if it has conflicts
DROP FUNCTION IF EXISTS disable_trigger_webhook(BIGINT);

-- Drop old regenerate_webhook_key if it has conflicts
DROP FUNCTION IF EXISTS regenerate_trigger_webhook_key(BIGINT);

-- Note: The current versions of these functions should be:
-- - attune.enable_trigger_webhook(BIGINT, JSONB DEFAULT '{}'::jsonb)
-- - attune.disable_trigger_webhook(BIGINT)
-- - attune.regenerate_trigger_webhook_key(BIGINT)

-- Verify functions exist after cleanup
DO $$
BEGIN
    -- Check that enable_trigger_webhook exists with correct signature
    -- Use current_schema() to work with both production (attune) and test schemas
    IF NOT EXISTS (
        SELECT 1 FROM pg_proc p
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname = current_schema()
        AND p.proname = 'enable_trigger_webhook'
        AND pg_get_function_arguments(p.oid) LIKE '%jsonb%'
    ) THEN
        RAISE EXCEPTION 'enable_trigger_webhook function with JSONB config not found after migration';
    END IF;
END $$;
