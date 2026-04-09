-- Migration: 000012_pack_environment_coordination.sql
-- Purpose: Make pack_environment version-aware and add worker claim/lease metadata
--          so workers can coordinate dependency installation for shared pack
--          runtime environments.

ALTER TABLE pack_environment
    ADD COLUMN IF NOT EXISTS runtime_version BIGINT REFERENCES runtime_version(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS runtime_version_text TEXT,
    ADD COLUMN IF NOT EXISTS manifest_checksum TEXT,
    ADD COLUMN IF NOT EXISTS claimed_by_worker BIGINT REFERENCES worker(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS claim_expires_at TIMESTAMPTZ;

ALTER TABLE pack_environment
    ADD COLUMN IF NOT EXISTS env_key TEXT GENERATED ALWAYS AS (
        CASE
            WHEN runtime_version IS NULL THEN format('base:%s:%s', pack, runtime)
            ELSE format('version:%s:%s:%s', pack, runtime, runtime_version)
        END
    ) STORED;

ALTER TABLE pack_environment
    DROP CONSTRAINT IF EXISTS pack_environment_pack_runtime_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_pack_environment_env_key
    ON pack_environment(env_key);

CREATE INDEX IF NOT EXISTS idx_pack_environment_runtime_version
    ON pack_environment(runtime_version);

CREATE INDEX IF NOT EXISTS idx_pack_environment_claim_expires_at
    ON pack_environment(claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_pack_environment_claimed_by_worker
    ON pack_environment(claimed_by_worker);

COMMENT ON COLUMN pack_environment.runtime_version IS 'Optional runtime_version row for version-specific environments; NULL for the base runtime environment';
COMMENT ON COLUMN pack_environment.runtime_version_text IS 'Display/runtime version string for version-specific environments';
COMMENT ON COLUMN pack_environment.env_key IS 'Generated unique coordination key for the shared environment target';
COMMENT ON COLUMN pack_environment.manifest_checksum IS 'Checksum of the dependency manifest used to produce the installed environment';
COMMENT ON COLUMN pack_environment.claimed_by_worker IS 'Worker currently holding the install lease for this environment target';
COMMENT ON COLUMN pack_environment.claim_expires_at IS 'Lease expiry for the current install claim; expired claims may be reclaimed by another worker';
