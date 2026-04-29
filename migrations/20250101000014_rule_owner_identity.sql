-- Migration: Add owner_identity to rule
-- Purpose: Track which identity registered a rule so rule-triggered executions
-- can be attributed to that identity instead of the hardcoded system identity.

ALTER TABLE rule
    ADD COLUMN owner_identity BIGINT REFERENCES identity(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_rule_owner_identity ON rule(owner_identity);

COMMENT ON COLUMN rule.owner_identity IS 'Identity that registered the rule. Used to attribute rule-triggered executions. NULL for system-loaded rules (init pack loader).';
