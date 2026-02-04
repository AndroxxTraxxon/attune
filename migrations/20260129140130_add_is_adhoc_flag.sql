-- Migration: Add is_adhoc flag to action, rule, and trigger tables
-- Description: Distinguishes between pack-installed components (is_adhoc=false) and manually created ad-hoc components (is_adhoc=true)
-- Version: 20260129140130

-- ============================================================================
-- Add is_adhoc column to action table
-- ============================================================================

ALTER TABLE action ADD COLUMN is_adhoc BOOLEAN DEFAULT false NOT NULL;

-- Index for filtering ad-hoc actions
CREATE INDEX idx_action_is_adhoc ON action(is_adhoc) WHERE is_adhoc = true;

COMMENT ON COLUMN action.is_adhoc IS 'True if action was manually created (ad-hoc), false if installed from pack';

-- ============================================================================
-- Add is_adhoc column to rule table
-- ============================================================================

ALTER TABLE rule ADD COLUMN is_adhoc BOOLEAN DEFAULT false NOT NULL;

-- Index for filtering ad-hoc rules
CREATE INDEX idx_rule_is_adhoc ON rule(is_adhoc) WHERE is_adhoc = true;

COMMENT ON COLUMN rule.is_adhoc IS 'True if rule was manually created (ad-hoc), false if installed from pack';

-- ============================================================================
-- Add is_adhoc column to trigger table
-- ============================================================================

ALTER TABLE trigger ADD COLUMN is_adhoc BOOLEAN DEFAULT false NOT NULL;

-- Index for filtering ad-hoc triggers
CREATE INDEX idx_trigger_is_adhoc ON trigger(is_adhoc) WHERE is_adhoc = true;

COMMENT ON COLUMN trigger.is_adhoc IS 'True if trigger was manually created (ad-hoc), false if installed from pack';

-- ============================================================================
-- Notes
-- ============================================================================
-- - Default is false (not ad-hoc) for backward compatibility with existing pack-installed components
-- - Ad-hoc components are eligible for deletion by users with appropriate permissions
-- - Pack-installed components (is_adhoc=false) should not be deletable directly, only via pack uninstallation
