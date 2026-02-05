-- Migration: Restore webhook functions
-- Description: Recreate webhook functions that were accidentally dropped in 20260129000001
-- Date: 2026-02-04

-- Drop existing functions to avoid signature conflicts
DROP FUNCTION IF EXISTS enable_trigger_webhook(BIGINT, JSONB);
DROP FUNCTION IF EXISTS enable_trigger_webhook(BIGINT);
DROP FUNCTION IF EXISTS disable_trigger_webhook(BIGINT);
DROP FUNCTION IF EXISTS regenerate_trigger_webhook_key(BIGINT);

-- Function to enable webhooks for a trigger
CREATE OR REPLACE FUNCTION enable_trigger_webhook(
    p_trigger_id BIGINT,
    p_config JSONB DEFAULT '{}'::jsonb
)
RETURNS TABLE(
    webhook_enabled BOOLEAN,
    webhook_key VARCHAR(255),
    webhook_url TEXT
) AS $$
DECLARE
    v_webhook_key VARCHAR(255);
    v_api_base_url TEXT := 'http://localhost:8080'; -- Default, should be configured
BEGIN
    -- Check if trigger exists
    IF NOT EXISTS (SELECT 1 FROM trigger WHERE id = p_trigger_id) THEN
        RAISE EXCEPTION 'Trigger with id % does not exist', p_trigger_id;
    END IF;

    -- Generate webhook key if one doesn't exist
    SELECT t.webhook_key INTO v_webhook_key
    FROM trigger t
    WHERE t.id = p_trigger_id;

    IF v_webhook_key IS NULL THEN
        v_webhook_key := generate_webhook_key();
    END IF;

    -- Update trigger to enable webhooks
    UPDATE trigger
    SET
        webhook_enabled = TRUE,
        webhook_key = v_webhook_key,
        webhook_config = p_config,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Return webhook details
    RETURN QUERY SELECT
        TRUE,
        v_webhook_key,
        v_api_base_url || '/api/v1/webhooks/' || v_webhook_key;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION enable_trigger_webhook(BIGINT, JSONB) IS
    'Enables webhooks for a trigger with optional configuration. Generates a new webhook key if one does not exist. Returns webhook details.';

-- Function to disable webhooks for a trigger
CREATE OR REPLACE FUNCTION disable_trigger_webhook(
    p_trigger_id BIGINT
)
RETURNS BOOLEAN AS $$
BEGIN
    -- Check if trigger exists
    IF NOT EXISTS (SELECT 1 FROM trigger WHERE id = p_trigger_id) THEN
        RAISE EXCEPTION 'Trigger with id % does not exist', p_trigger_id;
    END IF;

    -- Update trigger to disable webhooks
    -- Set webhook_key to NULL when disabling to remove it from API responses
    UPDATE trigger
    SET
        webhook_enabled = FALSE,
        webhook_key = NULL,
        updated = NOW()
    WHERE id = p_trigger_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION disable_trigger_webhook(BIGINT) IS
    'Disables webhooks for a trigger. Webhook key is removed when disabled.';

-- Function to regenerate webhook key for a trigger
CREATE OR REPLACE FUNCTION regenerate_trigger_webhook_key(
    p_trigger_id BIGINT
)
RETURNS TABLE(
    webhook_key VARCHAR(255),
    previous_key_revoked BOOLEAN
) AS $$
DECLARE
    v_new_key VARCHAR(255);
    v_old_key VARCHAR(255);
    v_webhook_enabled BOOLEAN;
BEGIN
    -- Check if trigger exists
    IF NOT EXISTS (SELECT 1 FROM trigger WHERE id = p_trigger_id) THEN
        RAISE EXCEPTION 'Trigger with id % does not exist', p_trigger_id;
    END IF;

    -- Get current webhook state
    SELECT t.webhook_key, t.webhook_enabled INTO v_old_key, v_webhook_enabled
    FROM trigger t
    WHERE t.id = p_trigger_id;

    -- Check if webhooks are enabled
    IF NOT v_webhook_enabled THEN
        RAISE EXCEPTION 'Webhooks are not enabled for trigger %', p_trigger_id;
    END IF;

    -- Generate new key
    v_new_key := generate_webhook_key();

    -- Update trigger with new key
    UPDATE trigger
    SET
        webhook_key = v_new_key,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Return new key and whether old key was present
    RETURN QUERY SELECT
        v_new_key,
        (v_old_key IS NOT NULL);
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION regenerate_trigger_webhook_key(BIGINT) IS
    'Regenerates webhook key for a trigger. Returns new key and whether a previous key was revoked.';

-- Verify all functions exist
DO $$
BEGIN
    -- Check enable_trigger_webhook exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_proc p
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname = current_schema()
        AND p.proname = 'enable_trigger_webhook'
    ) THEN
        RAISE EXCEPTION 'enable_trigger_webhook function not found after migration';
    END IF;

    -- Check disable_trigger_webhook exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_proc p
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname = current_schema()
        AND p.proname = 'disable_trigger_webhook'
    ) THEN
        RAISE EXCEPTION 'disable_trigger_webhook function not found after migration';
    END IF;

    -- Check regenerate_trigger_webhook_key exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_proc p
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname = current_schema()
        AND p.proname = 'regenerate_trigger_webhook_key'
    ) THEN
        RAISE EXCEPTION 'regenerate_trigger_webhook_key function not found after migration';
    END IF;

    RAISE NOTICE 'All webhook functions successfully restored';
END $$;
