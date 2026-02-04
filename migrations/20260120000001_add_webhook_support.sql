-- Migration: Add Webhook Support to Triggers
-- Date: 2026-01-20
-- Description: Adds webhook capabilities to the trigger system, allowing any trigger
--              to be webhook-enabled with a unique webhook key for external integrations.


-- Add webhook columns to trigger table
ALTER TABLE trigger
    ADD COLUMN IF NOT EXISTS webhook_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS webhook_key VARCHAR(64) UNIQUE,
    ADD COLUMN IF NOT EXISTS webhook_secret VARCHAR(128);

-- Add comments for documentation
COMMENT ON COLUMN trigger.webhook_enabled IS
    'Whether webhooks are enabled for this trigger. When enabled, external systems can POST to the webhook URL to create events.';

COMMENT ON COLUMN trigger.webhook_key IS
    'Unique webhook key used in the webhook URL. Format: wh_[32 alphanumeric chars]. Acts as a bearer token for webhook authentication.';

COMMENT ON COLUMN trigger.webhook_secret IS
    'Optional secret for HMAC signature verification. When set, webhook requests must include a valid X-Webhook-Signature header.';

-- Create index for fast webhook key lookup
CREATE INDEX IF NOT EXISTS idx_trigger_webhook_key
    ON trigger(webhook_key)
    WHERE webhook_key IS NOT NULL;

-- Create index for querying webhook-enabled triggers
CREATE INDEX IF NOT EXISTS idx_trigger_webhook_enabled
    ON trigger(webhook_enabled)
    WHERE webhook_enabled = TRUE;

-- Add webhook-related metadata tracking to events
-- Events use the 'config' JSONB column for metadata
-- We'll add indexes to efficiently query webhook-sourced events

-- Create index for webhook-sourced events (using config column)
CREATE INDEX IF NOT EXISTS idx_event_webhook_source
    ON event((config->>'source'))
    WHERE (config->>'source') = 'webhook';

-- Create index for webhook key lookup in event config
CREATE INDEX IF NOT EXISTS idx_event_webhook_key
    ON event((config->>'webhook_key'))
    WHERE config->>'webhook_key' IS NOT NULL;

-- Function to generate webhook key
CREATE OR REPLACE FUNCTION generate_webhook_key()
RETURNS VARCHAR(64) AS $$
DECLARE
    key_prefix VARCHAR(3) := 'wh_';
    random_suffix VARCHAR(32);
    new_key VARCHAR(64);
    max_attempts INT := 10;
    attempt INT := 0;
BEGIN
    LOOP
        -- Generate 32 random alphanumeric characters
        random_suffix := encode(gen_random_bytes(24), 'base64');
        random_suffix := REPLACE(random_suffix, '/', '');
        random_suffix := REPLACE(random_suffix, '+', '');
        random_suffix := REPLACE(random_suffix, '=', '');
        random_suffix := LOWER(LEFT(random_suffix, 32));

        -- Construct full key
        new_key := key_prefix || random_suffix;

        -- Check if key already exists
        IF NOT EXISTS (SELECT 1 FROM trigger WHERE webhook_key = new_key) THEN
            RETURN new_key;
        END IF;

        -- Increment attempt counter
        attempt := attempt + 1;
        IF attempt >= max_attempts THEN
            RAISE EXCEPTION 'Failed to generate unique webhook key after % attempts', max_attempts;
        END IF;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION generate_webhook_key() IS
    'Generates a unique webhook key with format wh_[32 alphanumeric chars]. Ensures uniqueness by checking existing keys.';

-- Function to enable webhooks for a trigger
CREATE OR REPLACE FUNCTION enable_trigger_webhook(
    p_trigger_id BIGINT
)
RETURNS TABLE(
    webhook_enabled BOOLEAN,
    webhook_key VARCHAR(64),
    webhook_url TEXT
) AS $$
DECLARE
    v_new_key VARCHAR(64);
    v_existing_key VARCHAR(64);
    v_base_url TEXT;
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

    -- Update trigger to enable webhooks
    UPDATE trigger
    SET
        webhook_enabled = TRUE,
        webhook_key = v_new_key,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Construct webhook URL (base URL should be configured elsewhere)
    -- For now, return just the path
    v_base_url := '/api/v1/webhooks/' || v_new_key;

    -- Return result
    RETURN QUERY
    SELECT
        TRUE::BOOLEAN as webhook_enabled,
        v_new_key as webhook_key,
        v_base_url as webhook_url;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION enable_trigger_webhook(BIGINT) IS
    'Enables webhooks for a trigger. Generates a new webhook key if one does not exist. Returns webhook details.';

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
    -- Note: We keep the webhook_key for audit purposes
    UPDATE trigger
    SET
        webhook_enabled = FALSE,
        updated = NOW()
    WHERE id = p_trigger_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION disable_trigger_webhook(BIGINT) IS
    'Disables webhooks for a trigger. Webhook key is retained for audit purposes.';

-- Function to regenerate webhook key for a trigger
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

-- Create a view for webhook statistics
CREATE OR REPLACE VIEW webhook_stats AS
SELECT
    t.id as trigger_id,
    t.ref as trigger_ref,
    t.webhook_enabled,
    t.webhook_key,
    t.created as webhook_created_at,
    COUNT(e.id) as total_events,
    MAX(e.created) as last_event_at,
    MIN(e.created) as first_event_at
FROM trigger t
LEFT JOIN event e ON
    e.trigger = t.id
    AND (e.config->>'source') = 'webhook'
WHERE t.webhook_enabled = TRUE
GROUP BY t.id, t.ref, t.webhook_enabled, t.webhook_key, t.created;

COMMENT ON VIEW webhook_stats IS
    'Statistics for webhook-enabled triggers including event counts and timestamps.';

-- Grant permissions (adjust as needed for your RBAC setup)
-- GRANT SELECT ON webhook_stats TO attune_api;
-- GRANT EXECUTE ON FUNCTION generate_webhook_key() TO attune_api;
-- GRANT EXECUTE ON FUNCTION enable_trigger_webhook(BIGINT) TO attune_api;
-- GRANT EXECUTE ON FUNCTION disable_trigger_webhook(BIGINT) TO attune_api;
-- GRANT EXECUTE ON FUNCTION regenerate_trigger_webhook_key(BIGINT) TO attune_api;

-- Trigger update timestamp is already handled by existing triggers
-- No need to add it again

-- Migration complete messages
DO $$
BEGIN
    RAISE NOTICE 'Webhook support migration completed successfully';
    RAISE NOTICE 'Webhook-enabled triggers can now receive events via POST /api/v1/webhooks/:webhook_key';
END $$;
