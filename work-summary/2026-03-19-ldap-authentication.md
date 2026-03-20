# LDAP Authentication Support

**Date**: 2026-03-19

## Summary

Added LDAP as an authentication provider alongside the existing OIDC and local username/password login methods. LDAP authentication follows the same architectural patterns as OIDC — server-side credential verification, identity upsert with provider-specific claims stored in the `attributes` JSONB column, and JWT token issuance.

## Changes

### Backend (Rust)

#### New Files
- **`crates/api/src/auth/ldap.rs`** — LDAP authentication module using the `ldap3` crate (v0.12). Supports two authentication modes:
  - **Direct bind**: Constructs a DN from a configurable `bind_dn_template` (e.g., `uid={login},ou=users,dc=example,dc=com`) and binds directly as the user.
  - **Search-and-bind**: Binds as a service account (or anonymous), searches for the user entry using `user_search_base` + `user_filter`, then re-binds as the discovered DN with the user's password.
  - After successful authentication, fetches user attributes (login, email, display name, groups) and upserts an identity row with claims stored under `attributes.ldap`.

#### Modified Files
- **`crates/common/src/config.rs`**:
  - Added `LdapConfig` struct with fields for server URL, bind DN template, search base/filter, service account credentials, attribute mapping, TLS settings, and UI metadata (provider name/label/icon).
  - Added `ldap: Option<LdapConfig>` to `SecurityConfig`.
  - Added `show_ldap_login: bool` to `LoginPageConfig`.

- **`crates/common/src/repositories/identity.rs`**:
  - Added `find_by_ldap_dn()` method to `IdentityRepository`, querying `attributes->'ldap'->>'server_url'` and `attributes->'ldap'->>'dn'` (mirrors the existing `find_by_oidc_subject` pattern).

- **`crates/api/Cargo.toml`**:
  - Added `ldap3 = "0.12"` dependency.

- **`crates/api/src/auth/mod.rs`**:
  - Added `pub mod ldap;`.

- **`crates/api/src/routes/auth.rs`**:
  - Added `POST /auth/ldap/login` route and `ldap_login` handler (validates `LdapLoginRequest`, delegates to `ldap::authenticate`, returns `TokenResponse`).
  - Updated `auth_settings` handler to populate LDAP fields in the response.

- **`crates/api/src/dto/auth.rs`**:
  - Added `ldap_enabled`, `ldap_visible_by_default`, `ldap_provider_name`, `ldap_provider_label`, `ldap_provider_icon_url` fields to `AuthSettingsResponse`.

### Frontend (React/TypeScript)

- **`web/src/pages/auth/LoginPage.tsx`**:
  - Extended `AuthSettingsResponse` interface with LDAP fields.
  - Added LDAP login form (username/password) with emerald-colored submit button, error handling, and `?auth=ldap` override support.
  - Added divider between sections when multiple login methods are visible.

### Configuration

- **`config.example.yaml`**: Added full LDAP configuration example with comments explaining direct-bind vs search-and-bind modes.
- **`config.development.yaml`**: Added disabled LDAP section with direct-bind template.

### Documentation

- **`AGENTS.md`**: Updated Authentication & Security section to document both OIDC and LDAP providers, their config keys, routes, identity matching, and login page configuration.

## Architecture Notes

- LDAP authentication is a **synchronous POST** flow (no browser redirects), unlike OIDC which uses authorization code redirects. The user submits credentials to `POST /auth/ldap/login` and receives JWT tokens directly.
- Identity deduplication uses `server_url + dn` as the composite key (stored in `attributes.ldap`), analogous to OIDC's `issuer + sub`.
- Login name collision avoidance uses the same SHA-256 fallback pattern as OIDC (`ldap:<24-hex-chars>`).
- The `ldap3` crate connection is driven asynchronously on the Tokio runtime via `ldap3::drive!(conn)`.
- STARTTLS and TLS certificate verification skip are configurable per-deployment.