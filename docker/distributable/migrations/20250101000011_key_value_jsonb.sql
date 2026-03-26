-- Migration: Convert key.value from TEXT to JSONB
--
-- This allows keys to store structured data (objects, arrays, numbers, booleans)
-- in addition to plain strings. Existing string values are wrapped in JSON string
-- literals so they remain valid and accessible.
--
-- Before: value TEXT NOT NULL  (e.g., 'my-secret-token')
-- After:  value JSONB NOT NULL (e.g., '"my-secret-token"' or '{"user":"admin","pass":"s3cret"}')

-- Step 1: Convert existing TEXT values to JSONB.
-- to_jsonb(text) wraps a plain string as a JSON string literal, e.g.:
--   'hello'  ->  '"hello"'
-- This preserves all existing values perfectly — encrypted values (base64 strings)
-- become JSON strings, and plain text values become JSON strings.
ALTER TABLE key
    ALTER COLUMN value TYPE JSONB
    USING to_jsonb(value);
