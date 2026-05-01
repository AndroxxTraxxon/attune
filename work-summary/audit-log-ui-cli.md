# Audit log search/filter UI + CLI

Surfaced the existing `audit_event` hypertable through the API, CLI, and web UI
so operators can search and filter audit events by common-sense values.

(Audit-event *emission* from services remains out of scope and unimplemented;
the page/CLI work against whatever rows are present in the table.)

## API (`crates/api`, `crates/common`)
- Extended `AuditEventFilters` with `actor_login_contains`, `http_status`,
  `http_method`, `http_path_contains`, and `include_total`.
- Added `AuditSearchResult { rows, total, has_next }` and
  `AuditRepository::search_with_meta()` mirroring the `EventRepository::search`
  pattern, factoring the `WHERE` clause into a private `push_filter_clauses()`.
- Added `utoipa::ToSchema` to `AuditCategory` and `AuditOutcome` so they can
  appear in OpenAPI schemas.
- New DTOs in `crates/api/src/dto/audit.rs`: `AuditEventResponse`,
  `AuditEventSummary`, `AuditEventQueryParams`. Lowercase string projection
  for the category/outcome enums.
- New routes in `crates/api/src/routes/audit.rs`:
  - `GET /api/v1/audit-events` — paginated list with all filters.
  - `GET /api/v1/audit-events/{id}` — single event.
  - `GET /api/v1/audit-events/by-request/{request_id}` — request-id chain.
- Each route requires `audit_log:read` via `AuthorizationService`.
- Routes wired into `routes/mod.rs`, `server.rs`, and the OpenAPI doc
  (paths + schemas + new `audit` tag).
- Granted `audit_log:read` to the admin permission set in
  `packs/core/permission_sets/admin.yaml`.

## CLI (`crates/cli`)
- New `attune audit` top-level command in `commands/audit.rs`:
  - `attune audit list` with flags for category, event_type, outcome,
    actor_login, actor_identity, resource_type/id/ref, http_method/status/path,
    request_id, after/before, page/per_page.
  - `attune audit show <id>` — full event record (incl. details JSON).
  - `attune audit chain <request-id>` — every event sharing a request id.
- Wired into `commands/mod.rs` and `main.rs`. Supports table / JSON / YAML
  output via the existing `OutputFormat` plumbing.

## Web UI (`web/`)
- Hand-written service `web/src/api/services/AuditLogService.ts` mirroring the
  generated services (uses `OpenAPI` config + `request` core). Exports
  `AuditCategory`, `AuditOutcome`, `AuditEventSummary`, `AuditEventResponse`,
  and the `ListAuditEventsParams` query type. Will be replaced by codegen once
  `npm run generate:api` is rerun.
- New TanStack Query hook `web/src/hooks/useAuditEvents.ts` exposing
  `useAuditEvents`, `useAuditEvent`, and `useAuditEventsByRequest`.
- New page `web/src/pages/audit/AuditLogPage.tsx`: filter sidebar (category,
  outcome, event_type, actor_login, resource type/ref, http method/status/path,
  request id, datetime range), expandable result rows, pagination with a
  page-size selector, and an opt-in "exact totals" toggle wired through
  `include_total`.
- Route added to `web/src/App.tsx` (`/audit-log`, lazy-loaded).
- Nav entry added to `MainLayout.tsx` next to "Access Control", with a new
  `auditLog: ScrollText` icon in `navIcons.tsx`.

## Verification
- `cargo check --workspace` — clean, zero warnings.
- `npx tsc -b` (web typecheck) — clean.

## Follow-up: Audit event emission

Initial delivery only built the *query* surface; nothing in the API populated
the table, so the page was empty for live activity. This pass wires emission:

- `AppState` now carries an `audit_emitter: AuditEmitter` (defaults to noop;
  `new_with_audit` is used at startup).
- `crates/api/src/main.rs` spawns the audit writer task at boot via
  `attune_common::audit::spawn_writer` and stores the emitter on `AppState`.
- New `crates/api/src/middleware/audit.rs` provides `audit_request` — a
  per-request middleware that:
    - Generates a `RequestId` UUID and inserts it into request extensions.
    - Best-effort decodes the bearer token to attribute the request.
    - Captures method, path, status, duration, IP (`X-Forwarded-For` /
      `X-Real-IP` / peer), and user agent.
    - Emits one `api.<method>.<outcome>` event per request, skipping
      `/health`, `/docs/*`, `/api-spec/*`, and `/api/v1/audit-events*` to
      avoid recursive log noise.
- `server.rs` layers the new middleware *outside* `log_request` so 401s from
  auth still get recorded. `axum::serve` switched to
  `into_make_service_with_connect_info::<SocketAddr>` so peer IP is
  available.
- `routes/auth.rs::login` now emits explicit `auth.login.success` /
  `auth.login.failure` events with reason codes.
- `routes/executions.rs::create_execution` emits an explicit
  `execution.requested` audit event with the action ref, action id, parent
  execution id, and executor identity in `details`.
- `crates/common/src/audit/writer.rs` flush path now appends an explicit
  `::inet` cast to the `actor_ip` bind so PostgreSQL accepts text-rendered
  `IpAddr` values (the previous implicit coercion failed with
  `column "actor_ip" is of type inet but expression is of type text`).

Verified end-to-end: `curl POST /auth/login` produces both an
`auth.login.success` event and an `api.post.success` HTTP event;
`POST /api/v1/executions/execute` produces `execution.requested` (with the
action ref) plus the matching HTTP event, both visible via
`GET /api/v1/audit-events`.
