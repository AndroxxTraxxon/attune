-- Migration: Relax key ref format constraint
-- Description: Allow multi-segment dot-separated key refs (e.g., "pack.prefix.name")
-- The original constraint only allowed at most one dot: '^([^.]+\.)?[^.]+$'
-- Sensors create refs like "python_example.counter.rule_ref" which have multiple dots.

ALTER TABLE key DROP CONSTRAINT key_ref_format;
ALTER TABLE key ADD CONSTRAINT key_ref_format CHECK (ref ~ '^[^.]+(\.[^.]+)*$');
