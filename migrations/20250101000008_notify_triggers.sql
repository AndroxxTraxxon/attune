-- Migration: LISTEN/NOTIFY Triggers
-- Description: Consolidated PostgreSQL LISTEN/NOTIFY triggers for real-time event notifications
-- Version: 20250101000008

-- ============================================================================
-- EXECUTION CHANGE NOTIFICATION
-- ============================================================================

-- Function to notify on execution creation
CREATE OR REPLACE FUNCTION notify_execution_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
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

    payload := json_build_object(
        'entity_type', 'execution',
        'entity_id', NEW.id,
        'id', NEW.id,
        'action_id', NEW.action,
        'action_ref', NEW.action_ref,
        'status', NEW.status,
        'enforcement', NEW.enforcement,
        'rule_ref', enforcement_rule_ref,
        'trigger_ref', enforcement_trigger_ref,
        'parent', NEW.parent,
        'started_at', NEW.started_at,
        'workflow_task', NEW.workflow_task,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('execution_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to notify on execution status changes
CREATE OR REPLACE FUNCTION notify_execution_status_changed()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
    enforcement_rule_ref TEXT;
    enforcement_trigger_ref TEXT;
BEGIN
    -- Only notify on updates, not inserts
    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        -- Lookup enforcement details if this execution is linked to an enforcement
        IF NEW.enforcement IS NOT NULL THEN
            SELECT rule_ref, trigger_ref
            INTO enforcement_rule_ref, enforcement_trigger_ref
            FROM enforcement
            WHERE id = NEW.enforcement;
        END IF;

        payload := json_build_object(
            'entity_type', 'execution',
            'entity_id', NEW.id,
            'id', NEW.id,
            'action_id', NEW.action,
            'action_ref', NEW.action_ref,
            'status', NEW.status,
            'old_status', OLD.status,
            'enforcement', NEW.enforcement,
            'rule_ref', enforcement_rule_ref,
            'trigger_ref', enforcement_trigger_ref,
            'parent', NEW.parent,
            'started_at', NEW.started_at,
            'workflow_task', NEW.workflow_task,
            'created', NEW.created,
            'updated', NEW.updated
        );

        PERFORM pg_notify('execution_status_changed', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on execution table for creation
CREATE TRIGGER execution_created_notify
    AFTER INSERT ON execution
    FOR EACH ROW
    EXECUTE FUNCTION notify_execution_created();

-- Trigger on execution table for status changes
CREATE TRIGGER execution_status_changed_notify
    AFTER UPDATE ON execution
    FOR EACH ROW
    EXECUTE FUNCTION notify_execution_status_changed();

COMMENT ON FUNCTION notify_execution_created() IS 'Sends execution creation notifications via PostgreSQL LISTEN/NOTIFY';
COMMENT ON FUNCTION notify_execution_status_changed() IS 'Sends execution status change notifications via PostgreSQL LISTEN/NOTIFY';

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
        'entity_type', 'event',
        'entity_id', NEW.id,
        'id', NEW.id,
        'trigger', NEW.trigger,
        'trigger_ref', NEW.trigger_ref,
        'source', NEW.source,
        'source_ref', NEW.source_ref,
        'rule', NEW.rule,
        'rule_ref', NEW.rule_ref,
        'has_payload', NEW.payload IS NOT NULL,
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

-- Function to notify on enforcement creation
CREATE OR REPLACE FUNCTION notify_enforcement_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'enforcement',
        'entity_id', NEW.id,
        'id', NEW.id,
        'rule', NEW.rule,
        'rule_ref', NEW.rule_ref,
        'trigger_ref', NEW.trigger_ref,
        'event', NEW.event,
        'status', NEW.status,
        'condition', NEW.condition,
        'created', NEW.created,
        'resolved_at', NEW.resolved_at
    );

    PERFORM pg_notify('enforcement_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on enforcement table
CREATE TRIGGER enforcement_created_notify
    AFTER INSERT ON enforcement
    FOR EACH ROW
    EXECUTE FUNCTION notify_enforcement_created();

COMMENT ON FUNCTION notify_enforcement_created() IS 'Sends enforcement creation notifications via PostgreSQL LISTEN/NOTIFY';

-- Function to notify on enforcement status changes
CREATE OR REPLACE FUNCTION notify_enforcement_status_changed()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    -- Only notify on updates when status actually changed
    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        payload := json_build_object(
            'entity_type', 'enforcement',
            'entity_id', NEW.id,
            'id', NEW.id,
            'rule', NEW.rule,
            'rule_ref', NEW.rule_ref,
            'trigger_ref', NEW.trigger_ref,
            'event', NEW.event,
            'status', NEW.status,
            'old_status', OLD.status,
            'condition', NEW.condition,
            'created', NEW.created,
            'resolved_at', NEW.resolved_at
        );

        PERFORM pg_notify('enforcement_status_changed', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on enforcement table for status changes
CREATE TRIGGER enforcement_status_changed_notify
    AFTER UPDATE ON enforcement
    FOR EACH ROW
    EXECUTE FUNCTION notify_enforcement_status_changed();

COMMENT ON FUNCTION notify_enforcement_status_changed() IS 'Sends enforcement status change notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- INQUIRY NOTIFICATIONS
-- ============================================================================

-- Function to notify on inquiry creation
CREATE OR REPLACE FUNCTION notify_inquiry_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'inquiry',
        'entity_id', NEW.id,
        'id', NEW.id,
        'execution', NEW.execution,
        'status', NEW.status,
        'ttl', NEW.ttl,
        'created', NEW.created
    );

    PERFORM pg_notify('inquiry_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to notify on inquiry response
CREATE OR REPLACE FUNCTION notify_inquiry_responded()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    -- Only notify when status changes to 'responded'
    IF TG_OP = 'UPDATE' AND NEW.status = 'responded' AND OLD.status != 'responded' THEN
        payload := json_build_object(
            'entity_type', 'inquiry',
            'entity_id', NEW.id,
            'id', NEW.id,
            'execution', NEW.execution,
            'status', NEW.status,
            'updated', NEW.updated
        );

        PERFORM pg_notify('inquiry_responded', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on inquiry table for creation
CREATE TRIGGER inquiry_created_notify
    AFTER INSERT ON inquiry
    FOR EACH ROW
    EXECUTE FUNCTION notify_inquiry_created();

-- Trigger on inquiry table for responses
CREATE TRIGGER inquiry_responded_notify
    AFTER UPDATE ON inquiry
    FOR EACH ROW
    EXECUTE FUNCTION notify_inquiry_responded();

COMMENT ON FUNCTION notify_inquiry_created() IS 'Sends inquiry creation notifications via PostgreSQL LISTEN/NOTIFY';
COMMENT ON FUNCTION notify_inquiry_responded() IS 'Sends inquiry response notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- WORKFLOW EXECUTION NOTIFICATIONS
-- ============================================================================

-- Function to notify on workflow execution status changes
CREATE OR REPLACE FUNCTION notify_workflow_execution_status_changed()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'execution',
        'entity_id', NEW.id,
        'id', NEW.id,
        'action_ref', NEW.action_ref,
        'status', NEW.status,
        'old_status', OLD.status,
        'workflow_def', NEW.workflow_def,
        'parent', NEW.parent,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('workflow_execution_status_changed', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on execution table for workflow status changes
CREATE TRIGGER workflow_execution_status_changed_notify
    AFTER UPDATE ON execution
    FOR EACH ROW
    WHEN (OLD.status IS DISTINCT FROM NEW.status AND NEW.workflow_def IS NOT NULL)
    EXECUTE FUNCTION notify_workflow_execution_status_changed();

COMMENT ON FUNCTION notify_workflow_execution_status_changed() IS 'Sends workflow execution status change notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- ARTIFACT NOTIFICATIONS
-- ============================================================================

-- Function to notify on artifact creation
CREATE OR REPLACE FUNCTION notify_artifact_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'artifact',
        'entity_id', NEW.id,
        'id', NEW.id,
        'ref', NEW.ref,
        'type', NEW.type,
        'visibility', NEW.visibility,
        'name', NEW.name,
        'execution', NEW.execution,
        'scope', NEW.scope,
        'owner', NEW.owner,
        'content_type', NEW.content_type,
        'size_bytes', NEW.size_bytes,
        'created', NEW.created
    );

    PERFORM pg_notify('artifact_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on artifact table for creation
CREATE TRIGGER artifact_created_notify
    AFTER INSERT ON artifact
    FOR EACH ROW
    EXECUTE FUNCTION notify_artifact_created();

COMMENT ON FUNCTION notify_artifact_created() IS 'Sends artifact creation notifications via PostgreSQL LISTEN/NOTIFY';

-- Function to notify on artifact updates (progress appends, data changes)
CREATE OR REPLACE FUNCTION notify_artifact_updated()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
    latest_percent DOUBLE PRECISION;
    latest_message TEXT;
    entry_count INTEGER;
BEGIN
    -- Only notify on actual changes
    IF TG_OP = 'UPDATE' THEN
        -- Extract progress summary from data array if this is a progress artifact
        IF NEW.type = 'progress' AND NEW.data IS NOT NULL AND jsonb_typeof(NEW.data) = 'array' THEN
            entry_count := jsonb_array_length(NEW.data);
            IF entry_count > 0 THEN
                latest_percent := (NEW.data -> (entry_count - 1) ->> 'percent')::DOUBLE PRECISION;
                latest_message := NEW.data -> (entry_count - 1) ->> 'message';
            END IF;
        END IF;

        payload := json_build_object(
            'entity_type', 'artifact',
            'entity_id', NEW.id,
            'id', NEW.id,
            'ref', NEW.ref,
            'type', NEW.type,
            'visibility', NEW.visibility,
            'name', NEW.name,
            'execution', NEW.execution,
            'scope', NEW.scope,
            'owner', NEW.owner,
            'content_type', NEW.content_type,
            'size_bytes', NEW.size_bytes,
            'progress_percent', latest_percent,
            'progress_message', latest_message,
            'progress_entries', entry_count,
            'created', NEW.created,
            'updated', NEW.updated
        );

        PERFORM pg_notify('artifact_updated', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on artifact table for updates
CREATE TRIGGER artifact_updated_notify
    AFTER UPDATE ON artifact
    FOR EACH ROW
    EXECUTE FUNCTION notify_artifact_updated();

COMMENT ON FUNCTION notify_artifact_updated() IS 'Sends artifact update notifications via PostgreSQL LISTEN/NOTIFY (includes progress summary for progress-type artifacts)';
