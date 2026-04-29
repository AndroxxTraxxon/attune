-- Migration: Audit Log
-- Description: Creates the audit_event TimescaleDB hypertable that captures
--              security- and compliance-relevant events across Attune services
--              (API requests, auth, RBAC denials, secret access, admin/config
--              changes, execution lifecycle, pack registration).
--
--              The audit table is a hypertable partitioned on `created`. Like
--              other hypertables in the system (event/enforcement/execution),
--              it CANNOT be the target of FK constraints. Therefore actor and
--              resource references are plain BIGINT columns with denormalized
--              text fields (`actor_login`, `resource_ref`) so records survive
--              the deletion of the referenced row.
-- Version: 20250101000013

-- ============================================================================
-- ENUMS
-- ============================================================================

CREATE TYPE audit_category_enum AS ENUM (
    'api',         -- HTTP request/response audit
    'auth',        -- login/logout/token-refresh/token-expiry
    'rbac',        -- authorization decisions (denials always; allows optional)
    'secret',      -- key reads (especially decrypts), creates, updates, deletes
    'admin',       -- identity, role, permission-set changes; pack/rule toggles
    'execution',   -- execution lifecycle (requested, started, completed, failed, cancelled)
    'pack'         -- pack uploads, registration, deletion
);

CREATE TYPE audit_outcome_enum AS ENUM (
    'success',
    'failure',
    'denied'
);

-- ============================================================================
-- TABLE
-- ============================================================================

CREATE TABLE audit_event (
    id                  BIGSERIAL                           NOT NULL,
    created             TIMESTAMPTZ                         NOT NULL DEFAULT NOW(),

    -- Classification
    category            audit_category_enum                 NOT NULL,
    event_type          TEXT                                NOT NULL,
    outcome             audit_outcome_enum                  NOT NULL,

    -- Actor (denormalized; no FK because hypertables cannot be FK targets)
    actor_identity      BIGINT,
    actor_login         TEXT,
    actor_token_type    TEXT,
    actor_ip            INET,
    actor_user_agent    TEXT,

    -- Correlation (set by request_id middleware on API events; propagated)
    request_id          UUID,

    -- Resource (denormalized; no FK)
    resource_type       TEXT,
    resource_id         BIGINT,
    resource_ref        TEXT,

    -- API-specific (NULL for non-API events)
    http_method         TEXT,
    http_path           TEXT,
    http_status         INTEGER,
    duration_ms         INTEGER,

    -- Event-specific metadata (secrets MUST be masked before insertion)
    details             JSONB,

    -- Optional cascade chain ({rule_id, enforcement_id, execution_id, parent_request_id})
    correlation_chain   JSONB,

    -- Composite PK is required by TimescaleDB when partitioning column is not the first PK column
    PRIMARY KEY (id, created)
);

COMMENT ON TABLE  audit_event IS 'Security-grade audit trail (TimescaleDB hypertable, partitioned on created).';
COMMENT ON COLUMN audit_event.category          IS 'Top-level category of the audit event.';
COMMENT ON COLUMN audit_event.event_type        IS 'Dotted event-type identifier, e.g. auth.login.success, rbac.denied, key.read.';
COMMENT ON COLUMN audit_event.outcome           IS 'Outcome of the action: success, failure, or denied.';
COMMENT ON COLUMN audit_event.actor_identity    IS 'identity.id of the actor (NULL for anonymous/pre-auth events). No FK; hypertables cannot reference tables that may delete rows referenced from history.';
COMMENT ON COLUMN audit_event.actor_login       IS 'Snapshot of identity.login at the time of the event (forensic).';
COMMENT ON COLUMN audit_event.actor_token_type  IS 'Type of token presented: access, execution, sensor, refresh, or NULL.';
COMMENT ON COLUMN audit_event.request_id        IS 'UUID correlation ID assigned by the API request middleware; propagated to downstream events when available.';
COMMENT ON COLUMN audit_event.resource_type     IS 'Logical type of the affected resource, e.g. pack, key, action, execution, rule.';
COMMENT ON COLUMN audit_event.resource_ref      IS 'Snapshot of the resource ref at the time of the event (forensic; survives deletes).';
COMMENT ON COLUMN audit_event.details           IS 'Event-specific structured metadata. Secret values MUST be redacted before insertion.';
COMMENT ON COLUMN audit_event.correlation_chain IS 'Optional cascade lineage: {rule_id, enforcement_id, execution_id, parent_request_id} for events caused by a chain.';

-- ============================================================================
-- HYPERTABLE
-- ============================================================================

SELECT create_hypertable('audit_event', 'created',
    chunk_time_interval => INTERVAL '1 day');

-- ============================================================================
-- INDEXES
-- ============================================================================

-- Hypertable already creates a (created DESC) index on each chunk.

CREATE INDEX idx_audit_event_actor
    ON audit_event (actor_identity, created DESC)
    WHERE actor_identity IS NOT NULL;

CREATE INDEX idx_audit_event_category
    ON audit_event (category, created DESC);

CREATE INDEX idx_audit_event_event_type
    ON audit_event (event_type, created DESC);

CREATE INDEX idx_audit_event_outcome
    ON audit_event (outcome, created DESC);

CREATE INDEX idx_audit_event_resource
    ON audit_event (resource_type, resource_id, created DESC)
    WHERE resource_type IS NOT NULL;

CREATE INDEX idx_audit_event_resource_ref
    ON audit_event (resource_ref, created DESC)
    WHERE resource_ref IS NOT NULL;

CREATE INDEX idx_audit_event_request
    ON audit_event (request_id)
    WHERE request_id IS NOT NULL;

CREATE INDEX idx_audit_event_details
    ON audit_event USING GIN (details);

-- ============================================================================
-- COMPRESSION + RETENTION
-- ============================================================================

ALTER TABLE audit_event SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'category, actor_identity',
    timescaledb.compress_orderby   = 'created DESC, id DESC'
);

SELECT add_compression_policy('audit_event', INTERVAL '7 days');

-- 365-day default retention. Override at deployment time via the
-- timescaledb.retention policy if a different window is required.
SELECT add_retention_policy('audit_event', INTERVAL '365 days');
