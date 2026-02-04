-- Migration: Add NOTIFY trigger for execution updates
-- This enables real-time SSE streaming of execution status changes

-- Function to send notifications on execution changes
CREATE OR REPLACE FUNCTION notify_execution_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
BEGIN
    -- Build JSON payload with execution details
    payload := jsonb_build_object(
        'entity_type', 'execution',
        'entity_id', NEW.id,
        'timestamp', NOW(),
        'data', jsonb_build_object(
            'id', NEW.id,
            'status', NEW.status,
            'action_id', NEW.action,
            'action_ref', NEW.action_ref,
            'result', NEW.result,
            'created', NEW.created,
            'updated', NEW.updated
        )
    );

    -- Send notification to the attune_notifications channel
    PERFORM pg_notify('attune_notifications', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to send pg_notify on execution insert or update
CREATE TRIGGER notify_execution_change
    AFTER INSERT OR UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION notify_execution_change();

-- Add comment
COMMENT ON FUNCTION notify_execution_change() IS
    'Sends PostgreSQL NOTIFY for execution changes to enable real-time SSE streaming';
COMMENT ON TRIGGER notify_execution_change ON execution IS
    'Broadcasts execution changes via pg_notify for SSE clients';
