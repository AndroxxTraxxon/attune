-- Migration: Add rule association to event table
-- This enables events to be directly associated with specific rules,
-- improving query performance and enabling rule-specific event filtering.

-- Add rule and rule_ref columns to event table
ALTER TABLE event
    ADD COLUMN rule BIGINT,
    ADD COLUMN rule_ref TEXT;

-- Add foreign key constraint
ALTER TABLE event
    ADD CONSTRAINT event_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;

-- Add indexes for efficient querying
CREATE INDEX idx_event_rule ON event(rule);
CREATE INDEX idx_event_rule_ref ON event(rule_ref);
CREATE INDEX idx_event_rule_created ON event(rule, created DESC);
CREATE INDEX idx_event_trigger_rule ON event(trigger, rule);

-- Add comments
COMMENT ON COLUMN event.rule IS
    'Optional reference to the specific rule that generated this event. Used by sensors that emit events for specific rule instances (e.g., timer sensors with multiple interval rules).';

COMMENT ON COLUMN event.rule_ref IS
    'Human-readable reference to the rule (e.g., "core.echo_every_second"). Denormalized for query convenience.';

-- Update the notify trigger to include rule information if present
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
            'rule', NEW.rule,
            'rule_ref', NEW.rule_ref,
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

-- Add comment on updated function
COMMENT ON FUNCTION notify_event_created() IS
    'Sends PostgreSQL NOTIFY for event creation with optional rule association';
