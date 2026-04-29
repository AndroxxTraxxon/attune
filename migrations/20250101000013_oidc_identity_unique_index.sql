-- Race-safe OIDC identity upsert: enforce one-row-per-(issuer, sub) for any
-- identity carrying OIDC attributes. The partial predicate keeps the index
-- scoped to OIDC rows so non-OIDC (local, LDAP, service account) identities
-- are completely unaffected.
--
-- The index expression evaluates to NULL when an OIDC row exists but is
-- missing `issuer` or `sub`. PostgreSQL allows multiple NULLs in a unique
-- index, so malformed rows still INSERT successfully — the index only
-- prevents true (issuer, sub) duplicates.
CREATE UNIQUE INDEX IF NOT EXISTS uq_identity_oidc_issuer_sub
ON identity (
    (attributes->'oidc'->>'issuer'),
    (attributes->'oidc'->>'sub')
)
WHERE attributes ? 'oidc';
