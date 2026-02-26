# Entity History UI Panels — Phase 2 Completion

**Date**: 2026-02-26

## Summary

Completed the remaining Phase 2 work for the TimescaleDB entity history feature by building the Web UI history panels and integrating them into entity detail pages.

## Changes

### New Files

1. **`web/src/hooks/useHistory.ts`** — React Query hooks for fetching entity history from the API:
   - `useEntityHistory()` — generic hook accepting entity type, ID, and query params
   - `useExecutionHistory()`, `useWorkerHistory()`, `useEnforcementHistory()`, `useEventHistory()` — convenience wrappers
   - Types: `HistoryRecord`, `PaginatedHistoryResponse`, `HistoryQueryParams`, `HistoryEntityType`
   - Uses `apiClient` (with auth interceptors) to call `GET /api/v1/{entities}/{id}/history`

2. **`web/src/components/common/EntityHistoryPanel.tsx`** — Reusable collapsible panel component:
   - Starts collapsed by default to avoid unnecessary API calls on page load
   - Fetches history only when expanded (via `enabled` flag on React Query)
   - **Filters**: Operation type dropdown (INSERT/UPDATE/DELETE) and changed field text input
   - **Pagination**: First/prev/next/last page navigation with total count
   - **Timeline rows**: Each record is expandable to show field-level details
   - **Field diffs**: For UPDATE operations, shows old → new values side by side; simple scalar values use inline red/green format, complex objects use side-by-side JSON blocks
   - **INSERT/DELETE handling**: Shows initial values or values at deletion respectively
   - Operation badges color-coded: green (INSERT), blue (UPDATE), red (DELETE)
   - Relative timestamps with full ISO 8601 tooltip

### Modified Files

3. **`web/src/pages/executions/ExecutionDetailPage.tsx`** — Added `EntityHistoryPanel` below the main content grid with `entityType="execution"`

4. **`web/src/pages/enforcements/EnforcementDetailPage.tsx`** — Added `EntityHistoryPanel` below the main content grid with `entityType="enforcement"`

5. **`web/src/pages/events/EventDetailPage.tsx`** — Added `EntityHistoryPanel` below the main content grid with `entityType="event"`

6. **`docs/plans/timescaledb-entity-history.md`** — Marked Phase 2 as ✅ complete with details of the UI implementation

7. **`AGENTS.md`** — Updated development status: moved "History UI panels" from Planned to Complete

### Not Modified

- **Worker detail page** does not exist yet in the web UI, so no worker history panel was added. The `useWorkerHistory()` hook and the `entityType="worker"` option are ready for when a worker detail page is created.

## Design Decisions

- **Collapsed by default**: History panels start collapsed to avoid unnecessary API requests on every page load. The query only fires when the user expands the panel.
- **Uses `apiClient` directly**: Since the history endpoints aren't part of the generated OpenAPI client (they would need a client regeneration), the hook uses `apiClient` from `lib/api-client.ts` which already handles JWT auth and token refresh.
- **Configurable page size**: Defaults to 10 records per page (suitable for a detail-page sidebar), but can be overridden via prop.
- **Full-width placement**: The history panel is placed below the main two-column grid layout on each detail page, spanning full width for readability.