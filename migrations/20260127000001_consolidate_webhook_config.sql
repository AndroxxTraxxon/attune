-- Migration: Consolidate Webhook Configuration
-- Date: 2026-01-27
-- Description: Consolidates multiple webhook_* columns into a single webhook_config JSONB column
--              for cleaner schema and better flexibility. Keeps webhook_enabled and webhook_key
--              as separate columns for indexing and quick filtering.


-- Step 1: Add new webhook_config column
ALTER TABLE trigger
    ADD COLUMN IF NOT EXISTS webhook_config JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN trigger.webhook_config IS
    'Webhook configuration as JSON. Contains settings like secret, HMAC config, rate limits, IP whitelist, etc.';

-- Step 2: Migrate existing data to webhook_config
-- Build JSON object from existing columns
UPDATE trigger
SET webhook_config = jsonb_build_object(
    'secret', COALESCE(webhook_secret, NULL),
    'hmac', jsonb_build_object(
        'enabled', COALESCE(webhook_hmac_enabled, false),
        'secret', COALESCE(webhook_hmac_secret, NULL),
        'algorithm', COALESCE(webhook_hmac_algorithm, 'sha256')
    ),
    'rate_limit', jsonb_build_object(
        'enabled', COALESCE(webhook_rate_limit_enabled, false),
        'requests', COALESCE(webhook_rate_limit_requests, NULL),
        'window_seconds', COALESCE(webhook_rate_limit_window_seconds, NULL)
    ),
    'ip_whitelist', jsonb_build_object(
        'enabled', COALESCE(webhook_ip_whitelist_enabled, false),
        'ips', COALESCE(
            (SELECT jsonb_agg(ip) FROM unnest(webhook_ip_whitelist) AS ip),
            '[]'::jsonb
        )
    ),
    'payload_size_limit_kb', COALESCE(webhook_payload_size_limit_kb, NULL)
)
WHERE webhook_enabled = true OR webhook_key IS NOT NULL;

-- Step 3: Drop dependent views that reference the columns we're about to drop
DROP VIEW IF EXISTS webhook_stats;
DROP VIEW IF EXISTS webhook_stats_detailed;

-- Step 4: Drop NOT NULL constraints on columns we're about to drop
ALTER TABLE trigger
    DROP CONSTRAINT IF EXISTS trigger_webhook_hmac_enabled_not_null,
    DROP CONSTRAINT IF EXISTS trigger_webhook_rate_limit_enabled_not_null,
    DROP CONSTRAINT IF EXISTS trigger_webhook_ip_whitelist_enabled_not_null;

-- Step 5: Drop old webhook columns (keeping webhook_enabled and webhook_key)
ALTER TABLE trigger
    DROP COLUMN IF EXISTS webhook_secret,
    DROP COLUMN IF EXISTS webhook_hmac_enabled,
    DROP COLUMN IF EXISTS webhook_hmac_secret,
    DROP COLUMN IF EXISTS webhook_hmac_algorithm,
    DROP COLUMN IF EXISTS webhook_rate_limit_enabled,
    DROP COLUMN IF EXISTS webhook_rate_limit_requests,
    DROP COLUMN IF EXISTS webhook_rate_limit_window_seconds,
    DROP COLUMN IF EXISTS webhook_ip_whitelist_enabled,
    DROP COLUMN IF EXISTS webhook_ip_whitelist,
    DROP COLUMN IF EXISTS webhook_payload_size_limit_kb;

-- Step 6: Drop old indexes that referenced removed columns
DROP INDEX IF EXISTS idx_trigger_webhook_enabled;

-- Step 7: Recreate index for webhook_enabled with better name
CREATE INDEX IF NOT EXISTS idx_trigger_webhook_enabled
    ON trigger(webhook_enabled)
    WHERE webhook_enabled = TRUE;

-- Index on webhook_key already exists from previous migration
-- CREATE INDEX IF NOT EXISTS idx_trigger_webhook_key ON trigger(webhook_key) WHERE webhook_key IS NOT NULL;

-- Step 8: Add GIN index for webhook_config JSONB queries
CREATE INDEX IF NOT EXISTS idx_trigger_webhook_config
    ON trigger USING gin(webhook_config)
    WHERE webhook_config IS NOT NULL AND webhook_config != '{}'::jsonb;

-- Step 9: Recreate webhook stats view with new schema
CREATE OR REPLACE VIEW webhook_stats AS
SELECT
    t.id as trigger_id,
    t.ref as trigger_ref,
    t.webhook_enabled,
    t.webhook_key,
    t.webhook_config,
    t.created as webhook_created_at,
    COUNT(e.id) as total_events,
    MAX(e.created) as last_event_at,
    MIN(e.created) as first_event_at
FROM trigger t
LEFT JOIN event e ON
    e.trigger = t.id
    AND (e.config->>'source') = 'webhook'
WHERE t.webhook_enabled = TRUE
GROUP BY t.id, t.ref, t.webhook_enabled, t.webhook_key, t.webhook_config, t.created;

COMMENT ON VIEW webhook_stats IS
    'Statistics for webhook-enabled triggers including event counts and timestamps.';

-- Step 10: Update helper functions to work with webhook_config

-- Update enable_trigger_webhook to work with new schema
CREATE OR REPLACE FUNCTION enable_trigger_webhook(
    p_trigger_id BIGINT,
    p_config JSONB DEFAULT '{}'::jsonb
)
RETURNS TABLE(
    webhook_enabled BOOLEAN,
    webhook_key VARCHAR(64),
    webhook_url TEXT,
    webhook_config JSONB
) AS $$
DECLARE
    v_new_key VARCHAR(64);
    v_existing_key VARCHAR(64);
    v_base_url TEXT;
    v_config JSONB;
BEGIN
    -- Check if trigger exists
    IF NOT EXISTS (SELECT 1 FROM trigger WHERE id = p_trigger_id) THEN
        RAISE EXCEPTION 'Trigger with id % does not exist', p_trigger_id;
    END IF;

    -- Get existing webhook key if any
    SELECT t.webhook_key INTO v_existing_key
    FROM trigger t
    WHERE t.id = p_trigger_id;

    -- Generate new key if one doesn't exist
    IF v_existing_key IS NULL THEN
        v_new_key := generate_webhook_key();
    ELSE
        v_new_key := v_existing_key;
    END IF;

    -- Merge provided config with defaults
    v_config := p_config || jsonb_build_object(
        'hmac', COALESCE(p_config->'hmac', jsonb_build_object('enabled', false, 'algorithm', 'sha256')),
        'rate_limit', COALESCE(p_config->'rate_limit', jsonb_build_object('enabled', false)),
        'ip_whitelist', COALESCE(p_config->'ip_whitelist', jsonb_build_object('enabled', false, 'ips', '[]'::jsonb))
    );

    -- Update trigger to enable webhooks
    UPDATE trigger
    SET
        webhook_enabled = TRUE,
        webhook_key = v_new_key,
        webhook_config = v_config,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Construct webhook URL
    v_base_url := '/api/v1/webhooks/' || v_new_key;

    -- Return result
    RETURN QUERY
    SELECT
        TRUE::BOOLEAN as webhook_enabled,
        v_new_key as webhook_key,
        v_base_url as webhook_url,
        v_config as webhook_config;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION enable_trigger_webhook(BIGINT, JSONB) IS
    'Enables webhooks for a trigger with optional configuration. Generates a new webhook key if one does not exist. Returns webhook details.';

-- Update disable_trigger_webhook (no changes needed, but recreate for consistency)
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
    -- Note: We keep the webhook_key and webhook_config for audit purposes
    UPDATE trigger
    SET
        webhook_enabled = FALSE,
        updated = NOW()
    WHERE id = p_trigger_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION disable_trigger_webhook(BIGINT) IS
    'Disables webhooks for a trigger. Webhook key and config are retained for audit purposes.';

-- Update regenerate_trigger_webhook_key (no changes to logic)
CREATE OR REPLACE FUNCTION regenerate_trigger_webhook_key(
    p_trigger_id BIGINT
)
RETURNS TABLE(
    webhook_key VARCHAR(64),
    previous_key_revoked BOOLEAN
) AS $$
DECLARE
    v_old_key VARCHAR(64);
    v_new_key VARCHAR(64);
BEGIN
    -- Check if trigger exists
    IF NOT EXISTS (SELECT 1 FROM trigger WHERE id = p_trigger_id) THEN
        RAISE EXCEPTION 'Trigger with id % does not exist', p_trigger_id;
    END IF;

    -- Get existing key
    SELECT t.webhook_key INTO v_old_key
    FROM trigger t
    WHERE t.id = p_trigger_id;

    -- Generate new key
    v_new_key := generate_webhook_key();

    -- Update trigger with new key
    UPDATE trigger
    SET
        webhook_key = v_new_key,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Return result
    RETURN QUERY
    SELECT
        v_new_key as webhook_key,
        (v_old_key IS NOT NULL)::BOOLEAN as previous_key_revoked;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION regenerate_trigger_webhook_key(BIGINT) IS
    'Regenerates the webhook key for a trigger. The old key is immediately revoked.';

-- Drop old webhook-specific functions that are no longer needed
DROP FUNCTION IF EXISTS enable_trigger_webhook_hmac(BIGINT, VARCHAR);
DROP FUNCTION IF EXISTS disable_trigger_webhook_hmac(BIGINT);

-- Migration complete messages
DO $$
BEGIN
    RAISE NOTICE 'Webhook configuration consolidation completed successfully';
    RAISE NOTICE 'Webhook settings now stored in webhook_config JSONB column';
    RAISE NOTICE 'Kept separate columns: webhook_enabled (indexed), webhook_key (indexed)';
END $$;
