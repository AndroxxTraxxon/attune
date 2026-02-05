-- Migration: LISTEN/NOTIFY Triggers
-- Description: Consolidated PostgreSQL LISTEN/NOTIFY triggers for real-time event notifications
-- Version: 20250101000013

-- ============================================================================
-- EXECUTION CHANGE NOTIFICATION
-- ============================================================================

-- Function to notify on execution changes
CREATE OR REPLACE FUNCTION notify_execution_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'id', NEW.id,
        'ref', NEW.ref,
        'action_ref', NEW.action_ref,
        'status', NEW.status,
        'rule', NEW.rule,
        'rule_ref', NEW.rule_ref,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('execution_change', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on execution table
CREATE TRIGGER execution_change_notify
    AFTER INSERT OR UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION notify_execution_change();

COMMENT ON FUNCTION notify_execution_change() IS 'Sends execution change notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- EVENT CREATION NOTIFICATION
-- ============================================================================

-- Function to notify on event creation
CREATE OR REPLACE FUNCTION notify_event_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'id', NEW.id,
        'ref', NEW.ref,
        'trigger_ref', NEW.trigger_ref,
        'rule', NEW.rule,
        'rule_ref', NEW.rule_ref,
        'created', NEW.created
    );

    PERFORM pg_notify('event_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on event table
CREATE TRIGGER event_created_notify
    AFTER INSERT ON event
    FOR EACH ROW
    EXECUTE FUNCTION notify_event_created();

COMMENT ON FUNCTION notify_event_created() IS 'Sends event creation notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- ENFORCEMENT CHANGE NOTIFICATION
-- ============================================================================

-- Function to notify on enforcement changes
CREATE OR REPLACE FUNCTION notify_enforcement_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'id', NEW.id,
        'ref', NEW.ref,
        'rule_ref', NEW.rule_ref,
        'status', NEW.status,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('enforcement_change', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on enforcement table
CREATE TRIGGER enforcement_change_notify
    AFTER INSERT OR UPDATE ON enforcement
    FOR EACH ROW
    EXECUTE FUNCTION notify_enforcement_change();

COMMENT ON FUNCTION notify_enforcement_change() IS 'Sends enforcement change notifications via PostgreSQL LISTEN/NOTIFY';
