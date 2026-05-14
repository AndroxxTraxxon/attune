-- Migration: Supervisor Run Lifecycle
-- Description: Adds an idempotent lifecycle marker table used by
--              attune-supervisor to detect prior dirty shutdowns.
-- Version: 20250101000015

SET search_path TO attune, public;

CREATE TABLE IF NOT EXISTS supervisor_run (
    id TEXT PRIMARY KEY,
    service_name TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stopped_at TIMESTAMPTZ,
    clean_shutdown BOOLEAN NOT NULL DEFAULT FALSE,
    stop_reason TEXT,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_supervisor_run_service_started
    ON supervisor_run (service_name, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_supervisor_run_unclean
    ON supervisor_run (service_name, started_at DESC)
    WHERE clean_shutdown = FALSE AND stopped_at IS NULL;

COMMENT ON TABLE supervisor_run IS
    'Supervisor process lifecycle markers used to detect prior dirty shutdowns and make startup recovery explicit.';
