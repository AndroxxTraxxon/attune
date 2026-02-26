# Entity History Phase 3 — Analytics Dashboard

**Date**: 2026-02-26

## Summary

Implemented the final phase of the TimescaleDB entity history plan: continuous aggregates, analytics API endpoints, and dashboard visualization widgets. This completes the full history tracking pipeline from database triggers → hypertables → continuous aggregates → API → UI.

## Changes

### New Files

1. **`migrations/20260226200000_continuous_aggregates.sql`** — TimescaleDB continuous aggregates migration:
   - `execution_status_hourly` — execution status transitions per hour by action_ref and status
   - `execution_throughput_hourly` — execution creation volume per hour by action_ref
   - `event_volume_hourly` — event creation volume per hour by trigger_ref
   - `worker_status_hourly` — worker status transitions per hour by worker name
   - `enforcement_volume_hourly` — enforcement creation volume per hour by rule_ref
   - Auto-refresh policies: 30-min interval for most, 1-hour for workers; 7-day lookback window
   - Initial `CALL refresh_continuous_aggregate()` for all five views

2. **`crates/common/src/repositories/analytics.rs`** — Analytics repository:
   - Row types: `ExecutionStatusBucket`, `ExecutionThroughputBucket`, `EventVolumeBucket`, `WorkerStatusBucket`, `EnforcementVolumeBucket`, `FailureRateSummary`
   - `AnalyticsTimeRange` with `default()` (24h), `last_hours()`, `last_days()` constructors
   - Query methods for each aggregate (global and per-entity-ref variants)
   - `execution_failure_rate()` — derives failure percentage from terminal-state transitions
   - Unit tests for time range construction and failure rate math

3. **`crates/api/src/dto/analytics.rs`** — Analytics DTOs:
   - `AnalyticsQueryParams` (since, until, hours) with `to_time_range()` conversion
   - Response types: `DashboardAnalyticsResponse`, `ExecutionStatusTimeSeriesResponse`, `ExecutionThroughputResponse`, `EventVolumeResponse`, `WorkerStatusTimeSeriesResponse`, `EnforcementVolumeResponse`, `FailureRateResponse`
   - `TimeSeriesPoint` — universal (bucket, label, value) data point
   - `From` conversions from all repository bucket types to `TimeSeriesPoint`
   - Unit tests for query param defaults, clamping, explicit ranges, and conversions

4. **`crates/api/src/routes/analytics.rs`** — 7 API endpoints:
   - `GET /api/v1/analytics/dashboard` — combined payload (all metrics in one call, concurrent queries via `tokio::try_join!`)
   - `GET /api/v1/analytics/executions/status` — status transitions over time
   - `GET /api/v1/analytics/executions/throughput` — creation throughput over time
   - `GET /api/v1/analytics/executions/failure-rate` — failure rate summary
   - `GET /api/v1/analytics/events/volume` — event creation volume
   - `GET /api/v1/analytics/workers/status` — worker status transitions
   - `GET /api/v1/analytics/enforcements/volume` — enforcement creation volume
   - All endpoints: authenticated, utoipa-documented, accept `since`/`until`/`hours` query params

5. **`web/src/hooks/useAnalytics.ts`** — React Query hooks:
   - `useDashboardAnalytics()` — fetches combined dashboard payload (1-min stale, 2-min auto-refresh)
   - Individual hooks: `useExecutionStatusAnalytics()`, `useExecutionThroughputAnalytics()`, `useFailureRateAnalytics()`, `useEventVolumeAnalytics()`, `useWorkerStatusAnalytics()`, `useEnforcementVolumeAnalytics()`
   - Types: `DashboardAnalytics`, `TimeSeriesPoint`, `FailureRateSummary`, `TimeSeriesResponse`, `AnalyticsQueryParams`

6. **`web/src/components/common/AnalyticsWidgets.tsx`** — Dashboard visualization components:
   - `AnalyticsDashboard` — composite widget rendering all charts and metrics
   - `MiniBarChart` — pure-CSS bar chart with hover tooltips and adaptive x-axis labels
   - `StackedBarChart` — stacked bar chart for status breakdowns with auto-generated legend
   - `FailureRateCard` — SVG ring gauge showing success/failure/timeout breakdown
   - `StatCard` — simple metric card with icon, label, and value
   - `TimeRangeSelector` — segmented button group (6h, 12h, 24h, 2d, 7d)
   - No external chart library dependency — all visualization is pure CSS/SVG

### Modified Files

7. **`crates/common/src/repositories/mod.rs`** — Registered `analytics` module, re-exported `AnalyticsRepository`

8. **`crates/api/src/dto/mod.rs`** — Registered `analytics` module, re-exported key DTO types

9. **`crates/api/src/routes/mod.rs`** — Registered `analytics` module, re-exported `analytics_routes`

10. **`crates/api/src/server.rs`** — Merged `analytics_routes()` into the API v1 router

11. **`web/src/pages/dashboard/DashboardPage.tsx`** — Added `AnalyticsDashboard` widget below existing metrics/activity sections with `TimeRangeHours` state management

12. **`docs/plans/timescaledb-entity-history.md`** — Marked Phase 2 continuous aggregates and Phase 3 analytics items as ✅ complete

13. **`AGENTS.md`** — Updated development status (continuous aggregates + analytics in Complete, removed from Planned)

## Design Decisions

- **Combined dashboard endpoint**: `GET /analytics/dashboard` fetches all 6 aggregate queries concurrently with `tokio::try_join!`, returning everything in one response. This avoids 6+ waterfall requests from the dashboard page.
- **No chart library**: All visualization uses pure CSS (flex-based bars) and inline SVG (ring gauge). This avoids adding a heavy chart dependency for what are essentially bar charts and a donut. A dedicated charting library can be introduced later if more sophisticated visualizations are needed.
- **Time range selector**: The dashboard defaults to 24 hours and offers 6h/12h/24h/2d/7d presets. The `hours` query param provides a simpler interface than specifying ISO timestamps for the common case.
- **Auto-refresh**: The dashboard analytics hook has `refetchInterval: 120000` (2 minutes) so the dashboard stays reasonably current without hammering the API. The continuous aggregates themselves refresh every 30 minutes on the server side.
- **Stale time**: Analytics hooks use 60-second stale time since the underlying aggregates only refresh every 30 minutes — there's no benefit to re-fetching more often.
- **Continuous aggregate refresh policies**: 30-minute schedule for execution/event/enforcement aggregates (higher expected volume), 1-hour for workers (low volume). All with a 1-hour `end_offset` to avoid refreshing the currently-filling bucket, and 7-day `start_offset` to limit refresh scope.

## Remaining (not in scope)

- **Configurable retention periods via admin settings** — retention policies are set in the migration; admin UI for changing them is deferred
- **Export/archival to external storage** — deferred to a future phase