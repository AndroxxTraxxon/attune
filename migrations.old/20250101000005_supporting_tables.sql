-- Migration: Supporting Tables and Indexes
-- Description: Creates notification and artifact tables plus performance optimization indexes
-- Version: 20250101000005


-- ============================================================================
-- NOTIFICATION TABLE
-- ============================================================================

CREATE TABLE notification (
    id BIGSERIAL PRIMARY KEY,
    channel TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity TEXT NOT NULL,
    activity TEXT NOT NULL,
    state notification_status_enum NOT NULL DEFAULT 'created',
    content JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_notification_channel ON notification(channel);
CREATE INDEX idx_notification_entity_type ON notification(entity_type);
CREATE INDEX idx_notification_entity ON notification(entity);
CREATE INDEX idx_notification_state ON notification(state);
CREATE INDEX idx_notification_created ON notification(created DESC);
CREATE INDEX idx_notification_channel_state ON notification(channel, state);
CREATE INDEX idx_notification_entity_type_entity ON notification(entity_type, entity);
CREATE INDEX idx_notification_state_created ON notification(state, created DESC);
CREATE INDEX idx_notification_content_gin ON notification USING GIN (content);

-- Trigger
CREATE TRIGGER update_notification_updated
    BEFORE UPDATE ON notification
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Function for pg_notify on notification insert
CREATE OR REPLACE FUNCTION notify_on_insert()
RETURNS TRIGGER AS $$
DECLARE
    payload TEXT;
BEGIN
    -- Build JSON payload with id, entity, and activity
    payload := json_build_object(
        'id', NEW.id,
        'entity_type', NEW.entity_type,
        'entity', NEW.entity,
        'activity', NEW.activity
    )::text;

    -- Send notification to the specified channel
    PERFORM pg_notify(NEW.channel, payload);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to send pg_notify on notification insert
CREATE TRIGGER notify_on_notification_insert
    AFTER INSERT ON notification
    FOR EACH ROW
    EXECUTE FUNCTION notify_on_insert();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON notification TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE notification_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE notification IS 'System notifications about entity changes for real-time updates';
COMMENT ON COLUMN notification.channel IS 'Notification channel (typically table name)';
COMMENT ON COLUMN notification.entity_type IS 'Type of entity (table name)';
COMMENT ON COLUMN notification.entity IS 'Entity identifier (typically ID or ref)';
COMMENT ON COLUMN notification.activity IS 'Activity type (e.g., "created", "updated", "completed")';
COMMENT ON COLUMN notification.state IS 'Processing state of notification';
COMMENT ON COLUMN notification.content IS 'Optional notification payload data';

-- ============================================================================
-- ARTIFACT TABLE
-- ============================================================================

CREATE TABLE artifact (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL,
    scope owner_type_enum NOT NULL DEFAULT 'system',
    owner TEXT NOT NULL DEFAULT '',
    type artifact_type_enum NOT NULL,
    retention_policy artifact_retention_enum NOT NULL DEFAULT 'versions',
    retention_limit INTEGER NOT NULL DEFAULT 1,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_artifact_ref ON artifact(ref);
CREATE INDEX idx_artifact_scope ON artifact(scope);
CREATE INDEX idx_artifact_owner ON artifact(owner);
CREATE INDEX idx_artifact_type ON artifact(type);
CREATE INDEX idx_artifact_created ON artifact(created DESC);
CREATE INDEX idx_artifact_scope_owner ON artifact(scope, owner);
CREATE INDEX idx_artifact_type_created ON artifact(type, created DESC);

-- Trigger
CREATE TRIGGER update_artifact_updated
    BEFORE UPDATE ON artifact
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON artifact TO svc_attune;
GRANT USAGE, SELECT ON SEQUENCE artifact_id_seq TO svc_attune;

-- Comments
COMMENT ON TABLE artifact IS 'Artifacts track files, logs, and outputs from executions';
COMMENT ON COLUMN artifact.ref IS 'Artifact reference/path';
COMMENT ON COLUMN artifact.scope IS 'Owner type (system, identity, pack, action, sensor)';
COMMENT ON COLUMN artifact.owner IS 'Owner identifier';
COMMENT ON COLUMN artifact.type IS 'Artifact type (file, url, progress, etc.)';
COMMENT ON COLUMN artifact.retention_policy IS 'How to retain artifacts (versions, days, hours, minutes)';
COMMENT ON COLUMN artifact.retention_limit IS 'Numeric limit for retention policy';

-- ============================================================================
-- QUEUE_STATS TABLE
-- ============================================================================

CREATE TABLE queue_stats (
    action_id BIGINT PRIMARY KEY REFERENCES action(id) ON DELETE CASCADE,
    queue_length INTEGER NOT NULL DEFAULT 0,
    active_count INTEGER NOT NULL DEFAULT 0,
    max_concurrent INTEGER NOT NULL DEFAULT 1,
    oldest_enqueued_at TIMESTAMPTZ,
    total_enqueued BIGINT NOT NULL DEFAULT 0,
    total_completed BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_queue_stats_last_updated ON queue_stats(last_updated);

-- Permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON queue_stats TO svc_attune;

-- Comments
COMMENT ON TABLE queue_stats IS 'Real-time queue statistics for action execution ordering';
COMMENT ON COLUMN queue_stats.action_id IS 'Foreign key to action table';
COMMENT ON COLUMN queue_stats.queue_length IS 'Number of executions waiting in queue';
COMMENT ON COLUMN queue_stats.active_count IS 'Number of currently running executions';
COMMENT ON COLUMN queue_stats.max_concurrent IS 'Maximum concurrent executions allowed';
COMMENT ON COLUMN queue_stats.oldest_enqueued_at IS 'Timestamp of oldest queued execution (NULL if queue empty)';
COMMENT ON COLUMN queue_stats.total_enqueued IS 'Total executions enqueued since queue creation';
COMMENT ON COLUMN queue_stats.total_completed IS 'Total executions completed since queue creation';
COMMENT ON COLUMN queue_stats.last_updated IS 'Timestamp of last statistics update';
