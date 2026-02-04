-- Migration: Add NOTIFY trigger for event creation
-- This enables real-time notifications when events are created

-- Function to send notifications on event creation
CREATE OR REPLACE FUNCTION notify_event_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
BEGIN
    -- Build JSON payload with event details
    payload := jsonb_build_object(
        'entity_type', 'event',
        'entity_id', NEW.id,
        'timestamp', NOW(),
        'data', jsonb_build_object(
            'id', NEW.id,
            'trigger', NEW.trigger,
            'trigger_ref', NEW.trigger_ref,
            'source', NEW.source,
            'source_ref', NEW.source_ref,
            'payload', NEW.payload,
            'created', NEW.created
        )
    );

    -- Send notification to the event_created channel
    PERFORM pg_notify('event_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to send pg_notify on event insert
CREATE TRIGGER notify_event_created
    AFTER INSERT ON event
    FOR EACH ROW
    EXECUTE FUNCTION notify_event_created();

-- Add comments
COMMENT ON FUNCTION notify_event_created() IS
    'Sends PostgreSQL NOTIFY for event creation to enable real-time notifications';
COMMENT ON TRIGGER notify_event_created ON event IS
    'Broadcasts event creation via pg_notify for real-time updates';
