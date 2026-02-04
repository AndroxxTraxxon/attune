-- Migration: Add advanced webhook features (HMAC, rate limiting, IP whitelist)
-- Created: 2026-01-20
-- Phase: 3 - Advanced Security Features

-- Add advanced webhook configuration columns to trigger table
ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_hmac_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_hmac_secret VARCHAR(128);

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_hmac_algorithm VARCHAR(32) DEFAULT 'sha256';

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_rate_limit_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_rate_limit_requests INTEGER DEFAULT 100;

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_rate_limit_window_seconds INTEGER DEFAULT 60;

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_ip_whitelist_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_ip_whitelist TEXT[]; -- Array of IP addresses/CIDR blocks

ALTER TABLE trigger ADD COLUMN IF NOT EXISTS
    webhook_payload_size_limit_kb INTEGER DEFAULT 1024; -- Default 1MB

COMMENT ON COLUMN trigger.webhook_hmac_enabled IS 'Whether HMAC signature verification is required';
COMMENT ON COLUMN trigger.webhook_hmac_secret IS 'Secret key for HMAC signature verification';
COMMENT ON COLUMN trigger.webhook_hmac_algorithm IS 'HMAC algorithm (sha256, sha512, etc.)';
COMMENT ON COLUMN trigger.webhook_rate_limit_enabled IS 'Whether rate limiting is enabled';
COMMENT ON COLUMN trigger.webhook_rate_limit_requests IS 'Max requests allowed per window';
COMMENT ON COLUMN trigger.webhook_rate_limit_window_seconds IS 'Rate limit time window in seconds';
COMMENT ON COLUMN trigger.webhook_ip_whitelist_enabled IS 'Whether IP whitelist is enabled';
COMMENT ON COLUMN trigger.webhook_ip_whitelist IS 'Array of allowed IP addresses/CIDR blocks';
COMMENT ON COLUMN trigger.webhook_payload_size_limit_kb IS 'Maximum webhook payload size in KB';

-- Create webhook event log table for auditing and analytics
CREATE TABLE IF NOT EXISTS webhook_event_log (
    id BIGSERIAL PRIMARY KEY,
    trigger_id BIGINT NOT NULL REFERENCES trigger(id) ON DELETE CASCADE,
    trigger_ref VARCHAR(255) NOT NULL,
    webhook_key VARCHAR(64) NOT NULL,
    event_id BIGINT REFERENCES event(id) ON DELETE SET NULL,
    source_ip INET,
    user_agent TEXT,
    payload_size_bytes INTEGER,
    headers JSONB,
    status_code INTEGER NOT NULL,
    error_message TEXT,
    processing_time_ms INTEGER,
    hmac_verified BOOLEAN,
    rate_limited BOOLEAN DEFAULT FALSE,
    ip_allowed BOOLEAN,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_event_log_trigger_id ON webhook_event_log(trigger_id);
CREATE INDEX idx_webhook_event_log_webhook_key ON webhook_event_log(webhook_key);
CREATE INDEX idx_webhook_event_log_created ON webhook_event_log(created DESC);
CREATE INDEX idx_webhook_event_log_status ON webhook_event_log(status_code);
CREATE INDEX idx_webhook_event_log_source_ip ON webhook_event_log(source_ip);

COMMENT ON TABLE webhook_event_log IS 'Audit log of all webhook requests';
COMMENT ON COLUMN webhook_event_log.status_code IS 'HTTP status code returned (200, 400, 403, 429, etc.)';
COMMENT ON COLUMN webhook_event_log.error_message IS 'Error message if request failed';
COMMENT ON COLUMN webhook_event_log.processing_time_ms IS 'Time taken to process webhook in milliseconds';
COMMENT ON COLUMN webhook_event_log.hmac_verified IS 'Whether HMAC signature was verified successfully';
COMMENT ON COLUMN webhook_event_log.rate_limited IS 'Whether request was rate limited';
COMMENT ON COLUMN webhook_event_log.ip_allowed IS 'Whether source IP was in whitelist (if enabled)';

-- Create webhook rate limit tracking table
CREATE TABLE IF NOT EXISTS webhook_rate_limit (
    id BIGSERIAL PRIMARY KEY,
    webhook_key VARCHAR(64) NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 1,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(webhook_key, window_start)
);

CREATE INDEX idx_webhook_rate_limit_key ON webhook_rate_limit(webhook_key);
CREATE INDEX idx_webhook_rate_limit_window ON webhook_rate_limit(window_start DESC);

COMMENT ON TABLE webhook_rate_limit IS 'Tracks webhook request counts for rate limiting';
COMMENT ON COLUMN webhook_rate_limit.window_start IS 'Start of the rate limit time window';
COMMENT ON COLUMN webhook_rate_limit.request_count IS 'Number of requests in this window';

-- Function to generate HMAC secret
CREATE OR REPLACE FUNCTION generate_webhook_hmac_secret()
RETURNS VARCHAR(128) AS $$
DECLARE
    secret VARCHAR(128);
BEGIN
    -- Generate 64-byte (128 hex chars) random secret
    SELECT encode(gen_random_bytes(64), 'hex') INTO secret;
    RETURN secret;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION generate_webhook_hmac_secret() IS 'Generate a secure random HMAC secret';

-- Function to enable HMAC for a trigger
CREATE OR REPLACE FUNCTION enable_trigger_webhook_hmac(
    p_trigger_id BIGINT,
    p_algorithm VARCHAR(32) DEFAULT 'sha256'
)
RETURNS TABLE(
    webhook_hmac_enabled BOOLEAN,
    webhook_hmac_secret VARCHAR(128),
    webhook_hmac_algorithm VARCHAR(32)
) AS $$
DECLARE
    v_webhook_enabled BOOLEAN;
    v_secret VARCHAR(128);
BEGIN
    -- Check if webhooks are enabled
    SELECT t.webhook_enabled INTO v_webhook_enabled
    FROM trigger t
    WHERE t.id = p_trigger_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Trigger with id % not found', p_trigger_id;
    END IF;

    IF NOT v_webhook_enabled THEN
        RAISE EXCEPTION 'Webhooks must be enabled before enabling HMAC verification';
    END IF;

    -- Validate algorithm
    IF p_algorithm NOT IN ('sha256', 'sha512', 'sha1') THEN
        RAISE EXCEPTION 'Invalid HMAC algorithm. Supported: sha256, sha512, sha1';
    END IF;

    -- Generate new secret
    v_secret := generate_webhook_hmac_secret();

    -- Update trigger
    UPDATE trigger
    SET
        webhook_hmac_enabled = TRUE,
        webhook_hmac_secret = v_secret,
        webhook_hmac_algorithm = p_algorithm,
        updated = NOW()
    WHERE id = p_trigger_id;

    -- Return result
    RETURN QUERY
    SELECT
        TRUE AS webhook_hmac_enabled,
        v_secret AS webhook_hmac_secret,
        p_algorithm AS webhook_hmac_algorithm;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION enable_trigger_webhook_hmac(BIGINT, VARCHAR) IS 'Enable HMAC signature verification for a trigger';

-- Function to disable HMAC for a trigger
CREATE OR REPLACE FUNCTION disable_trigger_webhook_hmac(p_trigger_id BIGINT)
RETURNS BOOLEAN AS $$
BEGIN
    UPDATE trigger
    SET
        webhook_hmac_enabled = FALSE,
        webhook_hmac_secret = NULL,
        updated = NOW()
    WHERE id = p_trigger_id;

    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION disable_trigger_webhook_hmac(BIGINT) IS 'Disable HMAC verification for a trigger';

-- Function to configure rate limiting
CREATE OR REPLACE FUNCTION configure_trigger_webhook_rate_limit(
    p_trigger_id BIGINT,
    p_enabled BOOLEAN,
    p_requests INTEGER DEFAULT 100,
    p_window_seconds INTEGER DEFAULT 60
)
RETURNS TABLE(
    rate_limit_enabled BOOLEAN,
    rate_limit_requests INTEGER,
    rate_limit_window_seconds INTEGER
) AS $$
BEGIN
    -- Validate inputs
    IF p_requests < 1 OR p_requests > 10000 THEN
        RAISE EXCEPTION 'Rate limit requests must be between 1 and 10000';
    END IF;

    IF p_window_seconds < 1 OR p_window_seconds > 3600 THEN
        RAISE EXCEPTION 'Rate limit window must be between 1 and 3600 seconds';
    END IF;

    -- Update trigger
    UPDATE trigger
    SET
        webhook_rate_limit_enabled = p_enabled,
        webhook_rate_limit_requests = p_requests,
        webhook_rate_limit_window_seconds = p_window_seconds,
        updated = NOW()
    WHERE id = p_trigger_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Trigger with id % not found', p_trigger_id;
    END IF;

    -- Return configuration
    RETURN QUERY
    SELECT
        p_enabled AS rate_limit_enabled,
        p_requests AS rate_limit_requests,
        p_window_seconds AS rate_limit_window_seconds;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION configure_trigger_webhook_rate_limit(BIGINT, BOOLEAN, INTEGER, INTEGER) IS 'Configure rate limiting for a trigger webhook';

-- Function to configure IP whitelist
CREATE OR REPLACE FUNCTION configure_trigger_webhook_ip_whitelist(
    p_trigger_id BIGINT,
    p_enabled BOOLEAN,
    p_ip_list TEXT[] DEFAULT ARRAY[]::TEXT[]
)
RETURNS TABLE(
    ip_whitelist_enabled BOOLEAN,
    ip_whitelist TEXT[]
) AS $$
BEGIN
    -- Update trigger
    UPDATE trigger
    SET
        webhook_ip_whitelist_enabled = p_enabled,
        webhook_ip_whitelist = p_ip_list,
        updated = NOW()
    WHERE id = p_trigger_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Trigger with id % not found', p_trigger_id;
    END IF;

    -- Return configuration
    RETURN QUERY
    SELECT
        p_enabled AS ip_whitelist_enabled,
        p_ip_list AS ip_whitelist;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION configure_trigger_webhook_ip_whitelist(BIGINT, BOOLEAN, TEXT[]) IS 'Configure IP whitelist for a trigger webhook';

-- Function to check rate limit (call before processing webhook)
CREATE OR REPLACE FUNCTION check_webhook_rate_limit(
    p_webhook_key VARCHAR(64),
    p_max_requests INTEGER,
    p_window_seconds INTEGER
)
RETURNS BOOLEAN AS $$
DECLARE
    v_window_start TIMESTAMPTZ;
    v_request_count INTEGER;
BEGIN
    -- Calculate current window start (truncated to window boundary)
    v_window_start := date_trunc('minute', NOW()) -
                      ((EXTRACT(EPOCH FROM date_trunc('minute', NOW()))::INTEGER % p_window_seconds) || ' seconds')::INTERVAL;

    -- Get or create rate limit record
    INSERT INTO webhook_rate_limit (webhook_key, window_start, request_count)
    VALUES (p_webhook_key, v_window_start, 1)
    ON CONFLICT (webhook_key, window_start)
    DO UPDATE SET
        request_count = webhook_rate_limit.request_count + 1,
        updated = NOW()
    RETURNING request_count INTO v_request_count;

    -- Clean up old rate limit records (older than 1 hour)
    DELETE FROM webhook_rate_limit
    WHERE window_start < NOW() - INTERVAL '1 hour';

    -- Return TRUE if within limit, FALSE if exceeded
    RETURN v_request_count <= p_max_requests;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION check_webhook_rate_limit(VARCHAR, INTEGER, INTEGER) IS 'Check if webhook request is within rate limit';

-- Function to check if IP is in whitelist (supports CIDR notation)
CREATE OR REPLACE FUNCTION check_webhook_ip_whitelist(
    p_source_ip INET,
    p_whitelist TEXT[]
)
RETURNS BOOLEAN AS $$
DECLARE
    v_allowed_cidr TEXT;
BEGIN
    -- If whitelist is empty, deny access
    IF p_whitelist IS NULL OR array_length(p_whitelist, 1) IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Check if source IP matches any entry in whitelist
    FOREACH v_allowed_cidr IN ARRAY p_whitelist
    LOOP
        -- Handle both single IPs and CIDR notation
        IF p_source_ip <<= v_allowed_cidr::INET THEN
            RETURN TRUE;
        END IF;
    END LOOP;

    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION check_webhook_ip_whitelist(INET, TEXT[]) IS 'Check if source IP is in whitelist (supports CIDR notation)';

-- View for webhook statistics
CREATE OR REPLACE VIEW webhook_stats_detailed AS
SELECT
    t.id AS trigger_id,
    t.ref AS trigger_ref,
    t.label AS trigger_label,
    t.webhook_enabled,
    t.webhook_key,
    t.webhook_hmac_enabled,
    t.webhook_rate_limit_enabled,
    t.webhook_rate_limit_requests,
    t.webhook_rate_limit_window_seconds,
    t.webhook_ip_whitelist_enabled,
    COUNT(DISTINCT wel.id) AS total_requests,
    COUNT(DISTINCT wel.id) FILTER (WHERE wel.status_code = 200) AS successful_requests,
    COUNT(DISTINCT wel.id) FILTER (WHERE wel.status_code >= 400) AS failed_requests,
    COUNT(DISTINCT wel.id) FILTER (WHERE wel.rate_limited = TRUE) AS rate_limited_requests,
    COUNT(DISTINCT wel.id) FILTER (WHERE wel.hmac_verified = FALSE AND t.webhook_hmac_enabled = TRUE) AS hmac_failures,
    COUNT(DISTINCT wel.id) FILTER (WHERE wel.ip_allowed = FALSE AND t.webhook_ip_whitelist_enabled = TRUE) AS ip_blocked_requests,
    COUNT(DISTINCT wel.event_id) AS events_created,
    AVG(wel.processing_time_ms) AS avg_processing_time_ms,
    MAX(wel.created) AS last_request_at,
    t.created AS webhook_enabled_at
FROM trigger t
LEFT JOIN webhook_event_log wel ON wel.trigger_id = t.id
WHERE t.webhook_enabled = TRUE
GROUP BY t.id, t.ref, t.label, t.webhook_enabled, t.webhook_key,
         t.webhook_hmac_enabled, t.webhook_rate_limit_enabled,
         t.webhook_rate_limit_requests, t.webhook_rate_limit_window_seconds,
         t.webhook_ip_whitelist_enabled, t.created;

COMMENT ON VIEW webhook_stats_detailed IS 'Detailed statistics for webhook-enabled triggers';

-- Grant permissions (adjust as needed for your security model)
GRANT SELECT, INSERT ON webhook_event_log TO attune_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON webhook_rate_limit TO attune_api;
GRANT SELECT ON webhook_stats_detailed TO attune_api;
GRANT USAGE, SELECT ON SEQUENCE webhook_event_log_id_seq TO attune_api;
GRANT USAGE, SELECT ON SEQUENCE webhook_rate_limit_id_seq TO attune_api;
