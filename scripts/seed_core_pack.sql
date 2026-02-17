-- Seed Core Pack with Generic Timer Triggers, Sensors, and Basic Actions
-- This script creates the core pack with the new trigger/sensor architecture

SET search_path TO attune, public;

-- Insert core pack
INSERT INTO attune.pack (ref, label, description, version)
VALUES (
    'core',
    'Core Pack',
    'Built-in core functionality including timer triggers and basic actions',
    '1.0.0'
)
ON CONFLICT (ref) DO UPDATE SET
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    version = EXCLUDED.version,
    updated = NOW();

-- Get pack ID for reference
DO $$
DECLARE
    v_pack_id BIGINT;
    v_action_runtime_id BIGINT;
    v_sensor_runtime_id BIGINT;
    v_intervaltimer_id BIGINT;
    v_crontimer_id BIGINT;
    v_datetimetimer_id BIGINT;
    v_echo_action_id BIGINT;
    v_sensor_10s_id BIGINT;
BEGIN
    -- Get core pack ID
    SELECT id INTO v_pack_id FROM attune.pack WHERE ref = 'core';

    -- Create shell runtime
    INSERT INTO attune.runtime (ref, pack, pack_ref, name, description, distributions)
    VALUES (
        'core.shell',
        v_pack_id,
        'core',
        'Shell',
        'Shell (bash/sh) runtime for script execution - always available',
        '{"verification": {"always_available": true}}'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        name = EXCLUDED.name,
        description = EXCLUDED.description,
        updated = NOW()
    RETURNING id INTO v_action_runtime_id;

    -- Create built-in runtime for sensors (no execution_config = not executable by worker)
    INSERT INTO attune.runtime (ref, pack, pack_ref, name, description, distributions)
    VALUES (
        'core.builtin',
        v_pack_id,
        'core',
        'Builtin',
        'Built-in sensor runtime for native Attune sensors (timers, webhooks, etc.)',
        '{"verification": {"always_available": true, "check_required": false}, "type": "builtin"}'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        name = EXCLUDED.name,
        description = EXCLUDED.description,
        updated = NOW()
    RETURNING id INTO v_sensor_runtime_id;

    -- Create generic timer triggers (these define trigger types, not instances)

    -- Interval Timer Trigger Type
    INSERT INTO attune.trigger (
        ref,
        pack,
        pack_ref,
        label,
        description,
        enabled,
        param_schema,
        out_schema
    )
    VALUES (
        'core.intervaltimer',
        v_pack_id,
        'core',
        'Interval Timer',
        'Fires at regular intervals based on specified time unit and interval',
        true,
        '{
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "enum": ["seconds", "minutes", "hours"],
                    "description": "Time unit for the interval"
                },
                "interval": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Number of time units between each trigger"
                }
            },
            "required": ["unit", "interval"]
        }'::jsonb,
        '{
            "type": "object",
            "properties": {
                "type": {"type": "string", "const": "interval"},
                "interval_seconds": {"type": "integer"},
                "fired_at": {"type": "string", "format": "date-time"}
            }
        }'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW()
    RETURNING id INTO v_intervaltimer_id;

    -- Cron Timer Trigger Type
    INSERT INTO attune.trigger (
        ref,
        pack,
        pack_ref,
        label,
        description,
        enabled,
        param_schema,
        out_schema
    )
    VALUES (
        'core.crontimer',
        v_pack_id,
        'core',
        'Cron Timer',
        'Fires based on a cron schedule expression',
        true,
        '{
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Cron expression (e.g., \"0 0 * * * *\" for every hour)"
                }
            },
            "required": ["expression"]
        }'::jsonb,
        '{
            "type": "object",
            "properties": {
                "type": {"type": "string", "const": "cron"},
                "fired_at": {"type": "string", "format": "date-time"},
                "scheduled_at": {"type": "string", "format": "date-time"}
            }
        }'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW()
    RETURNING id INTO v_crontimer_id;

    -- Datetime Timer Trigger Type
    INSERT INTO attune.trigger (
        ref,
        pack,
        pack_ref,
        label,
        description,
        enabled,
        param_schema,
        out_schema
    )
    VALUES (
        'core.datetimetimer',
        v_pack_id,
        'core',
        'Datetime Timer',
        'Fires once at a specific date and time',
        true,
        '{
            "type": "object",
            "properties": {
                "fire_at": {
                    "type": "string",
                    "format": "date-time",
                    "description": "ISO 8601 timestamp when the timer should fire"
                }
            },
            "required": ["fire_at"]
        }'::jsonb,
        '{
            "type": "object",
            "properties": {
                "type": {"type": "string", "const": "one_shot"},
                "fire_at": {"type": "string", "format": "date-time"},
                "fired_at": {"type": "string", "format": "date-time"}
            }
        }'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW()
    RETURNING id INTO v_datetimetimer_id;

    -- Create actions

    -- Echo action
    INSERT INTO attune.action (
        ref,
        pack,
        pack_ref,
        label,
        description,
        entrypoint,
        runtime,
        param_schema,
        out_schema
    )
    VALUES (
        'core.echo',
        v_pack_id,
        'core',
        'Echo',
        'Echo a message to stdout',
        'echo "${message}"',
        v_action_runtime_id,
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object(
                'message', jsonb_build_object(
                    'type', 'string',
                    'description', 'Message to echo',
                    'default', 'Hello World'
                )
            ),
            'required', jsonb_build_array('message')
        ),
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object(
                'stdout', jsonb_build_object('type', 'string'),
                'stderr', jsonb_build_object('type', 'string'),
                'exit_code', jsonb_build_object('type', 'integer')
            )
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        entrypoint = EXCLUDED.entrypoint,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW()
    RETURNING id INTO v_echo_action_id;

    -- Sleep action
    INSERT INTO attune.action (
        ref,
        pack,
        pack_ref,
        label,
        description,
        entrypoint,
        runtime,
        param_schema,
        out_schema
    )
    VALUES (
        'core.sleep',
        v_pack_id,
        'core',
        'Sleep',
        'Sleep for a specified number of seconds',
        'sleep ${seconds}',
        v_action_runtime_id,
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object(
                'seconds', jsonb_build_object(
                    'type', 'integer',
                    'description', 'Number of seconds to sleep',
                    'default', 1,
                    'minimum', 0
                )
            ),
            'required', jsonb_build_array('seconds')
        ),
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object(
                'exit_code', jsonb_build_object('type', 'integer')
            )
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        entrypoint = EXCLUDED.entrypoint,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW();

    -- Noop (no operation) action
    INSERT INTO attune.action (
        ref,
        pack,
        pack_ref,
        label,
        description,
        entrypoint,
        runtime,
        param_schema,
        out_schema
    )
    VALUES (
        'core.noop',
        v_pack_id,
        'core',
        'No Operation',
        'Does nothing - useful for testing',
        'exit 0',
        v_action_runtime_id,
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object()
        ),
        jsonb_build_object(
            'type', 'object',
            'properties', jsonb_build_object(
                'exit_code', jsonb_build_object('type', 'integer')
            )
        )
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        entrypoint = EXCLUDED.entrypoint,
        param_schema = EXCLUDED.param_schema,
        out_schema = EXCLUDED.out_schema,
        updated = NOW();

    -- Create example sensor: 10-second interval timer
    INSERT INTO attune.sensor (
        ref,
        pack,
        pack_ref,
        label,
        description,
        entrypoint,
        runtime,
        runtime_ref,
        trigger,
        trigger_ref,
        enabled,
        config
    )
    VALUES (
        'core.timer_10s_sensor',
        v_pack_id,
        'core',
        '10 Second Timer Sensor',
        'Timer sensor that fires every 10 seconds',
        'builtin:interval_timer',
        v_sensor_runtime_id,
        'core.builtin',
        v_intervaltimer_id,
        'core.intervaltimer',
        true,
        '{"unit": "seconds", "interval": 10}'::jsonb
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        config = EXCLUDED.config,
        updated = NOW()
    RETURNING id INTO v_sensor_10s_id;

    -- Create example rule: 10-second timer triggers echo action with "hello, world"
    INSERT INTO attune.rule (
        ref,
        pack,
        pack_ref,
        label,
        description,
        action,
        action_ref,
        trigger,
        trigger_ref,
        conditions,
        action_params,
        enabled
    )
    VALUES (
        'core.rule.timer_10s_echo',
        v_pack_id,
        'core',
        'Echo Hello World Every 10 Seconds',
        'Example rule that echoes "hello, world" every 10 seconds',
        v_echo_action_id,
        'core.echo',
        v_intervaltimer_id,
        'core.intervaltimer',
        jsonb_build_object(),  -- No conditions
        jsonb_build_object(
            'message', 'hello, world'
        ),
        true
    )
    ON CONFLICT (ref) DO UPDATE SET
        label = EXCLUDED.label,
        description = EXCLUDED.description,
        action_params = EXCLUDED.action_params,
        updated = NOW();

    RAISE NOTICE 'Core pack seeded successfully';
    RAISE NOTICE 'Pack ID: %', v_pack_id;
    RAISE NOTICE 'Action Runtime ID: %', v_action_runtime_id;
    RAISE NOTICE 'Sensor Runtime ID: %', v_sensor_runtime_id;
    RAISE NOTICE 'Trigger Types: intervaltimer=%, crontimer=%, datetimetimer=%', v_intervaltimer_id, v_crontimer_id, v_datetimetimer_id;
    RAISE NOTICE 'Actions: core.echo, core.sleep, core.noop';
    RAISE NOTICE 'Sensors: core.timer_10s_sensor (id=%)', v_sensor_10s_id;
    RAISE NOTICE 'Rules: core.rule.timer_10s_echo';
END $$;
