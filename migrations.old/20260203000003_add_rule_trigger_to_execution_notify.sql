-- Migration: Add rule_ref and trigger_ref to execution notification payload
-- This includes enforcement information in real-time notifications to avoid additional API calls

-- Drop the existing trigger first
DROP TRIGGER IF EXISTS notify_execution_change ON execution;

-- Replace the notification function to include enforcement details
CREATE OR REPLACE FUNCTION notify_execution_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
    enforcement_rule_ref TEXT;
    enforcement_trigger_ref TEXT;
BEGIN
    -- Lookup enforcement details if this execution is linked to an enforcement
    IF NEW.enforcement IS NOT NULL THEN
        SELECT rule_ref, trigger_ref
        INTO enforcement_rule_ref, enforcement_trigger_ref
        FROM enforcement
        WHERE id = NEW.enforcement;
    END IF;

    -- Build JSON payload with execution details including rule/trigger info
    payload := jsonb_build_object(
        'entity_type', 'execution',
        'entity_id', NEW.id,
        'timestamp', NOW(),
        'data', jsonb_build_object(
            'id', NEW.id,
            'status', NEW.status,
            'action_id', NEW.action,
            'action_ref', NEW.action_ref,
            'enforcement', NEW.enforcement,
            'rule_ref', enforcement_rule_ref,
            'trigger_ref', enforcement_trigger_ref,
            'parent', NEW.parent,
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

-- Recreate the trigger
CREATE TRIGGER notify_execution_change
    AFTER INSERT OR UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION notify_execution_change();

-- Update comment
COMMENT ON FUNCTION notify_execution_change() IS
    'Sends PostgreSQL NOTIFY for execution changes with enforcement details (rule_ref, trigger_ref) to enable real-time SSE streaming without additional API calls';
