-- Migration: Pack Installation Metadata
-- Description: Tracks pack installation sources, checksums, and metadata
-- Created: 2026-01-22

-- Pack installation metadata table
CREATE TABLE IF NOT EXISTS pack_installation (
    id BIGSERIAL PRIMARY KEY,
    pack_id BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,

    -- Installation source information
    source_type VARCHAR(50) NOT NULL CHECK (source_type IN ('git', 'archive', 'local_directory', 'local_archive', 'registry')),
    source_url TEXT,
    source_ref TEXT,  -- git ref (branch/tag/commit) or registry version

    -- Verification
    checksum VARCHAR(64),  -- SHA256 checksum of installed pack
    checksum_verified BOOLEAN DEFAULT FALSE,

    -- Installation metadata
    installed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    installed_by BIGINT REFERENCES identity(id) ON DELETE SET NULL,
    installation_method VARCHAR(50) DEFAULT 'manual' CHECK (installation_method IN ('manual', 'api', 'cli', 'auto')),

    -- Storage information
    storage_path TEXT NOT NULL,

    -- Additional metadata
    meta JSONB DEFAULT '{}'::jsonb,

    created TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    -- Constraints
    CONSTRAINT pack_installation_unique_pack UNIQUE (pack_id)
);

-- Indexes
CREATE INDEX idx_pack_installation_pack_id ON pack_installation(pack_id);
CREATE INDEX idx_pack_installation_source_type ON pack_installation(source_type);
CREATE INDEX idx_pack_installation_installed_at ON pack_installation(installed_at);
CREATE INDEX idx_pack_installation_installed_by ON pack_installation(installed_by);

-- Trigger for updated timestamp
CREATE TRIGGER pack_installation_updated_trigger
    BEFORE UPDATE ON pack_installation
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_column();

-- Comments
COMMENT ON TABLE pack_installation IS 'Tracks pack installation metadata including source, checksum, and storage location';
COMMENT ON COLUMN pack_installation.source_type IS 'Type of installation source (git, archive, local_directory, local_archive, registry)';
COMMENT ON COLUMN pack_installation.source_url IS 'URL or path of the installation source';
COMMENT ON COLUMN pack_installation.source_ref IS 'Git reference (branch/tag/commit) or registry version';
COMMENT ON COLUMN pack_installation.checksum IS 'SHA256 checksum of the installed pack contents';
COMMENT ON COLUMN pack_installation.checksum_verified IS 'Whether the checksum was verified during installation';
COMMENT ON COLUMN pack_installation.installed_by IS 'Identity that installed the pack';
COMMENT ON COLUMN pack_installation.installation_method IS 'Method used to install (manual, api, cli, auto)';
COMMENT ON COLUMN pack_installation.storage_path IS 'File system path where pack is stored';
COMMENT ON COLUMN pack_installation.meta IS 'Additional installation metadata (dependencies resolved, warnings, etc.)';
