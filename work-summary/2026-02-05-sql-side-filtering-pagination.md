# SQL-Side Filtering & Pagination Audit

**Date**: 2026-02-05
**Scope**: All list/search API endpoints across every table resource

## Problem

Following the discovery that the execution search endpoint was performing filtering in memory rather than in the database, an audit of all other list/search endpoints revealed the same pattern was pervasive across the codebase. Two categories of issues were found:

### 1. In-Memory Filtering (fetch all rows, then `.retain()`)
- **Events** (`list_events`): `source` and `rule_ref` filters applied in memory
- **Enforcements** (`list_enforcements`): `trigger_ref` filter applied in memory; filters were mutually exclusive (if/else-if) rather than combinable
- **Keys** (`list_keys`): `owner` filter applied in memory
- **Inquiries** (`list_inquiries`): `assigned_to` filter applied in memory; `status`/`execution` were mutually exclusive

### 2. In-Memory Pagination (fetch ALL rows, then slice)
- **Actions**: `list_actions`, `list_actions_by_pack`
- **Triggers**: `list_triggers`, `list_enabled_triggers`, `list_triggers_by_pack`
- **Sensors**: `list_sensors`, `list_enabled_sensors`, `list_sensors_by_pack`, `list_sensors_by_trigger`
- **Rules**: `list_rules`, `list_enabled_rules`, `list_rules_by_pack`, `list_rules_by_action`, `list_rules_by_trigger`
- **Events**: `list_events`
- **Enforcements**: `list_enforcements`
- **Keys**: `list_keys`
- **Inquiries**: `list_inquiries`, `list_inquiries_by_status`, `list_inquiries_by_execution`
- **Executions**: `list_executions_by_status`, `list_executions_by_enforcement` (older path-based endpoints)
- **Workflows**: `list_workflows`, `list_workflows_by_pack`
- **Execution Stats**: `get_execution_stats` fetched all rows to count by status

Only `list_packs` was already correct (using `list_paginated` + `count`).

## Solution

Added `search()`/`list_search()` methods to every entity repository, following the pattern established by `ExecutionRepository::search()`:

- Uses `sqlx::QueryBuilder` to dynamically build WHERE clauses
- All filters are combinable (AND) rather than mutually exclusive
- Pagination (LIMIT/OFFSET) is applied in SQL
- A parallel COUNT query shares the same WHERE clause for accurate total counts
- Returns a `SearchResult { rows, total }` struct

### Repository Changes

| Repository | New Types | New Method |
|---|---|---|
| `EventRepository` | `EventSearchFilters`, `EventSearchResult` | `search()` |
| `EnforcementRepository` | `EnforcementSearchFilters`, `EnforcementSearchResult` | `search()` |
| `KeyRepository` | `KeySearchFilters`, `KeySearchResult` | `search()` |
| `InquiryRepository` | `InquirySearchFilters`, `InquirySearchResult` | `search()` |
| `ActionRepository` | `ActionSearchFilters`, `ActionSearchResult` | `list_search()` |
| `TriggerRepository` | `TriggerSearchFilters`, `TriggerSearchResult` | `list_search()` |
| `SensorRepository` | `SensorSearchFilters`, `SensorSearchResult` | `list_search()` |
| `RuleRepository` | `RuleSearchFilters`, `RuleSearchResult` | `list_search()` |
| `WorkflowDefinitionRepository` | `WorkflowSearchFilters`, `WorkflowSearchResult` | `list_search()` |

### Route Handler Changes

Every list endpoint was updated to:
1. Build a filters struct from query/path parameters
2. Call the repository's `search()`/`list_search()` method
3. Map the result rows to DTOs
4. Return `PaginatedResponse` using the SQL-provided total count

### Additional Fixes

- **`get_execution_stats`**: Replaced fetch-all-and-count-in-Rust with `SELECT status::text, COUNT(*) FROM execution GROUP BY status` — a single SQL query.
- **`EnforcementQueryParams`**: Added `rule_ref` filter field (was missing from the DTO).
- **Enforcement filters**: Changed from mutually exclusive (if/else-if for status/rule/event) to combinable (AND).
- **Inquiry filters**: Changed from mutually exclusive (if/else-if for status/execution) to combinable (AND).
- **Workflow tag filtering**: Replaced multi-query-then-dedup approach with PostgreSQL array overlap operator (`tags && ARRAY[...]`) in a single query.

## Files Changed

### Repositories (`crates/common/src/repositories/`)
- `event.rs` — Added `EventSearchFilters`, `EventSearchResult`, `EnforcementSearchFilters`, `EnforcementSearchResult`, `EventRepository::search()`, `EnforcementRepository::search()`
- `key.rs` — Added `KeySearchFilters`, `KeySearchResult`, `KeyRepository::search()`
- `inquiry.rs` — Added `InquirySearchFilters`, `InquirySearchResult`, `InquiryRepository::search()`
- `action.rs` — Added `ActionSearchFilters`, `ActionSearchResult`, `ActionRepository::list_search()`
- `trigger.rs` — Added `TriggerSearchFilters`, `TriggerSearchResult`, `SensorSearchFilters`, `SensorSearchResult`, `TriggerRepository::list_search()`, `SensorRepository::list_search()`
- `rule.rs` — Added `RuleSearchFilters`, `RuleSearchResult`, `RuleRepository::list_search()`
- `workflow.rs` — Added `WorkflowSearchFilters`, `WorkflowSearchResult`, `WorkflowDefinitionRepository::list_search()`

### Routes (`crates/api/src/routes/`)
- `actions.rs` — `list_actions`, `list_actions_by_pack`
- `triggers.rs` — All 9 list endpoints (triggers + sensors)
- `rules.rs` — All 5 list endpoints
- `events.rs` — `list_events`, `list_enforcements`
- `keys.rs` — `list_keys`
- `inquiries.rs` — `list_inquiries`, `list_inquiries_by_status`, `list_inquiries_by_execution`
- `executions.rs` — `list_executions_by_status`, `list_executions_by_enforcement`, `get_execution_stats`
- `workflows.rs` — `list_workflows`, `list_workflows_by_pack`

### DTOs (`crates/api/src/dto/`)
- `event.rs` — Added `rule_ref` field to `EnforcementQueryParams`

## Impact

- **Correctness**: Filters that were silently ignored or mutually exclusive now work correctly and are combinable
- **Performance**: Endpoints no longer fetch entire tables into memory — pagination and filtering happen in PostgreSQL
- **Scalability**: Total counts are accurate (not capped at 1000 by the `List` trait's hardcoded LIMIT)
- **Consistency**: Every list endpoint now follows the same pattern as the execution search