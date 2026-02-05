-- Migration: Add NOTIFY trigger for enforcement creation
-- This enables real-time notifications when enforcements are created or updated

-- Function to send notifications on enforcement changes
CREATE OR REPLACE FUNCTION notify_enforcement_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
    operation TEXT;
BEGIN
    -- Determine operation type
    IF TG_OP = 'INSERT' THEN
        operation := 'created';
    ELSIF TG_OP = 'UPDATE' THEN
        operation := 'updated';
    ELSE
        operation := 'deleted';
    END IF;

    -- Build JSON payload with enforcement details
    payload := jsonb_build_object(
        'entity_type', 'enforcement',
        'entity_id', NEW.id,
        'operation', operation,
        'timestamp', NOW(),
        'data', jsonb_build_object(
            'id', NEW.id,
            'rule', NEW.rule,
            'rule_ref', NEW.rule_ref,
            'trigger_ref', NEW.trigger_ref,
            'event', NEW.event,
            'status', NEW.status,
            'condition', NEW.condition,
            'conditions', NEW.conditions,
            'config', NEW.config,
            'payload', NEW.payload,
            'created', NEW.created,
            'updated', NEW.updated
        )
    );

    -- Send notification to the attune_notifications channel
    PERFORM pg_notify('attune_notifications', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to send pg_notify on enforcement insert
CREATE TRIGGER notify_enforcement_change
    AFTER INSERT OR UPDATE ON enforcement
    FOR EACH ROW
    EXECUTE FUNCTION notify_enforcement_change();

-- Add comments
COMMENT ON FUNCTION notify_enforcement_change() IS
    'Sends PostgreSQL NOTIFY for enforcement changes to enable real-time notifications';
COMMENT ON TRIGGER notify_enforcement_change ON enforcement IS
    'Broadcasts enforcement changes via pg_notify for real-time updates';
