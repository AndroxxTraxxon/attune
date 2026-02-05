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

-- Comments
COMMENT ON TABLE notification IS 'System notifications about entity changes for real-time updates';
COMMENT ON COLUMN notification.channel IS 'Notification channel (typically table name)';
COMMENT ON COLUMN notification.entity_type IS 'Type of entity (table name)';
COMMENT ON COLUMN notification.entity IS 'Entity identifier (typically ID or ref)';
COMMENT ON COLUMN notification.activity IS 'Activity type (e.g., "created", "updated", "completed")';
COMMENT ON COLUMN notification.state IS 'Processing state of notification';
COMMENT ON COLUMN notification.content IS 'Optional notification payload data';

-- ============================================================================
