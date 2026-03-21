-- Migration: 000012_agent_runtime_detection
-- Adds columns to support agent auto-detected runtimes

-- Track whether a runtime was auto-registered by an agent
-- (vs. loaded from a pack's YAML file during pack registration)
ALTER TABLE runtime ADD COLUMN IF NOT EXISTS auto_detected BOOLEAN NOT NULL DEFAULT FALSE;

-- Store detection configuration for auto-discovered runtimes.
-- Used by agents to identify how they discovered the runtime and
-- enables re-verification on restart.
-- Example: { "binaries": ["ruby", "ruby3.2"], "version_command": "--version",
--            "version_regex": "ruby (\\d+\\.\\d+\\.\\d+)",
--            "detected_path": "/usr/bin/ruby",
--            "detected_version": "3.3.0" }
ALTER TABLE runtime ADD COLUMN IF NOT EXISTS detection_config JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Index for filtering auto-detected vs. pack-registered runtimes
CREATE INDEX IF NOT EXISTS idx_runtime_auto_detected ON runtime(auto_detected);

-- Comments
COMMENT ON COLUMN runtime.auto_detected IS 'Whether this runtime was auto-registered by an agent (true) vs. loaded from a pack YAML (false)';
COMMENT ON COLUMN runtime.detection_config IS 'Detection metadata for auto-discovered runtimes: binaries probed, version regex, detected path/version';
