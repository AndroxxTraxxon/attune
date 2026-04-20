-- Migration: Work Queue LISTEN/NOTIFY Triggers
-- Description: Emits PostgreSQL notifications for work queue definitions and items
-- Version: 20250101000014

-- ============================================================================
-- WORK QUEUE NOTIFICATIONS
-- ============================================================================

CREATE OR REPLACE FUNCTION notify_work_queue_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'work_queue',
        'entity_id', NEW.id,
        'id', NEW.id,
        'ref', NEW.ref,
        'pack_ref', NEW.pack_ref,
        'is_adhoc', NEW.is_adhoc,
        'label', NEW.label,
        'description', NEW.description,
        'enabled', NEW.enabled,
        'dispatch_action_ref', NEW.dispatch_action_ref,
        'default_priority', NEW.default_priority,
        'allow_pending_update', NEW.allow_pending_update,
        'update_strategy', NEW.update_strategy,
        'batch_mode', NEW.batch_mode,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('work_queue_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION notify_work_queue_updated()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        payload := json_build_object(
            'entity_type', 'work_queue',
            'entity_id', NEW.id,
            'id', NEW.id,
            'ref', NEW.ref,
            'pack_ref', NEW.pack_ref,
            'is_adhoc', NEW.is_adhoc,
            'label', NEW.label,
            'description', NEW.description,
            'enabled', NEW.enabled,
            'dispatch_action_ref', NEW.dispatch_action_ref,
            'default_priority', NEW.default_priority,
            'allow_pending_update', NEW.allow_pending_update,
            'update_strategy', NEW.update_strategy,
            'batch_mode', NEW.batch_mode,
            'created', NEW.created,
            'updated', NEW.updated
        );

        PERFORM pg_notify('work_queue_updated', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER work_queue_created_notify
    AFTER INSERT ON work_queue
    FOR EACH ROW
    EXECUTE FUNCTION notify_work_queue_created();

CREATE TRIGGER work_queue_updated_notify
    AFTER UPDATE ON work_queue
    FOR EACH ROW
    EXECUTE FUNCTION notify_work_queue_updated();

COMMENT ON FUNCTION notify_work_queue_created() IS 'Sends work queue creation notifications via PostgreSQL LISTEN/NOTIFY';
COMMENT ON FUNCTION notify_work_queue_updated() IS 'Sends work queue update notifications via PostgreSQL LISTEN/NOTIFY';

-- ============================================================================
-- WORK QUEUE ITEM NOTIFICATIONS
-- ============================================================================

CREATE OR REPLACE FUNCTION notify_work_queue_item_created()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    payload := json_build_object(
        'entity_type', 'work_queue_item',
        'entity_id', NEW.id,
        'id', NEW.id,
        'queue', NEW.queue,
        'queue_ref', NEW.queue_ref,
        'item_key', NEW.item_key,
        'priority', NEW.priority,
        'status', NEW.status,
        'enqueue_source', NEW.enqueue_source,
        'requested_by_identity', NEW.requested_by_identity,
        'requested_by_execution', NEW.requested_by_execution,
        'requested_by_enforcement', NEW.requested_by_enforcement,
        'leased_execution', NEW.leased_execution,
        'lease_token', NEW.lease_token,
        'lease_expires_at', NEW.lease_expires_at,
        'attempt_count', NEW.attempt_count,
        'last_error', NEW.last_error,
        'ack_summary', NEW.ack_summary,
        'created', NEW.created,
        'updated', NEW.updated
    );

    PERFORM pg_notify('work_queue_item_created', payload::text);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION notify_work_queue_item_updated()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        payload := json_build_object(
            'entity_type', 'work_queue_item',
            'entity_id', NEW.id,
            'id', NEW.id,
            'queue', NEW.queue,
            'queue_ref', NEW.queue_ref,
            'item_key', NEW.item_key,
            'priority', NEW.priority,
            'status', NEW.status,
            'old_status', OLD.status,
            'enqueue_source', NEW.enqueue_source,
            'requested_by_identity', NEW.requested_by_identity,
            'requested_by_execution', NEW.requested_by_execution,
            'requested_by_enforcement', NEW.requested_by_enforcement,
            'leased_execution', NEW.leased_execution,
            'lease_token', NEW.lease_token,
            'lease_expires_at', NEW.lease_expires_at,
            'attempt_count', NEW.attempt_count,
            'last_error', NEW.last_error,
            'ack_summary', NEW.ack_summary,
            'created', NEW.created,
            'updated', NEW.updated
        );

        PERFORM pg_notify('work_queue_item_updated', payload::text);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER work_queue_item_created_notify
    AFTER INSERT ON work_queue_item
    FOR EACH ROW
    EXECUTE FUNCTION notify_work_queue_item_created();

CREATE TRIGGER work_queue_item_updated_notify
    AFTER UPDATE ON work_queue_item
    FOR EACH ROW
    EXECUTE FUNCTION notify_work_queue_item_updated();

COMMENT ON FUNCTION notify_work_queue_item_created() IS 'Sends work queue item creation notifications via PostgreSQL LISTEN/NOTIFY';
COMMENT ON FUNCTION notify_work_queue_item_updated() IS 'Sends work queue item update notifications via PostgreSQL LISTEN/NOTIFY';
