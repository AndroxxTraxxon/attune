# Executions Page Search Functionality

**Date:** 2025-01-30  
**Status:** Complete

## Overview
Added comprehensive search/filter functionality to the Executions page, allowing users to filter executions by pack, rule, action, trigger, and executor ID. This makes it much easier to find specific executions in a large list.

## Changes Made

### 1. Backend API (`attune/crates/api`)

#### Updated `dto/execution.rs`
Added new query parameters to `ExecutionQueryParams`:
- `rule_ref: Option<String>` - Filter by rule reference (e.g., "core.on_timer")
- `trigger_ref: Option<String>` - Filter by trigger reference (e.g., "core.timer")
- `executor: Option<i64>` - Filter by executor ID

These join the existing filters:
- `status` - Filter by execution status
- `action_ref` - Filter by action reference
- `pack_name` - Filter by pack name
- `result_contains` - Search in result JSON
- `enforcement` - Filter by enforcement ID
- `parent` - Filter by parent execution ID

#### Updated `routes/executions.rs`
Enhanced the `list_executions` endpoint to support the new filters:

```rust
// Import EnforcementRepository for rule/trigger filtering
use attune_common::repositories::{
    action::ActionRepository,
    execution::{CreateExecutionInput, ExecutionRepository},
    Create, EnforcementRepository, FindById, FindByRef, List,
};
```

**Filtering Logic:**
1. **Executor filtering**: Simple in-memory filter on `execution.executor` field
2. **Rule/Trigger filtering**: More complex, requires joining with enforcement data:
   - Collects all enforcement IDs from filtered executions
   - Fetches enforcements in bulk from database
   - Creates a HashMap for quick lookups
   - Filters executions based on enforcement's `rule_ref` or `trigger_ref`

**Performance Note:** Current implementation fetches all enforcements and filters in memory. This works well for moderate datasets but could be optimized with database joins for large-scale deployments.

### 2. Frontend API Client (`web/src/api`)

Regenerated OpenAPI TypeScript client to include new parameters:
- `ruleRef?: string | null`
- `triggerRef?: string | null`  
- `executor?: number | null`

The generated `ExecutionsService.listExecutions()` method now accepts these parameters and maps them to the correct query parameter names (`rule_ref`, `trigger_ref`, `executor`).

### 3. Frontend Hooks (`web/src/hooks/useExecutions.ts`)

Updated `ExecutionsQueryParams` interface:
```typescript
interface ExecutionsQueryParams {
  page?: number;
  pageSize?: number;
  status?: ExecutionStatus;
  actionRef?: string;
  packName?: string;      // Added
  ruleRef?: string;       // Added
  triggerRef?: string;    // Added
  executor?: number;      // Added
}
```

Updated `useExecutions` hook to pass new parameters to the API service.

### 4. Frontend UI (`web/src/pages/executions/ExecutionsPage.tsx`)

Added comprehensive search UI with the following features:

#### Search State Management
```typescript
const [searchFilters, setSearchFilters] = useState({
  pack: "",
  rule: "",
  action: "",
  trigger: "",
  executor: "",
});
```

#### Filter Panel UI
- **Layout**: Responsive grid (1 column on mobile, 2 on tablet, 5 on desktop)
- **Search Icon**: Visual indicator for filtering section
- **Clear Button**: Appears when any filter is active
- **5 Search Fields**:
  1. **Pack** - e.g., "core"
  2. **Rule** - e.g., "core.on_timer"
  3. **Action** - e.g., "core.echo"
  4. **Trigger** - e.g., "core.timer"
  5. **Executor ID** - e.g., "1"

#### Features
- **Real-time filtering**: Filters are applied as user types (with React Query caching)
- **Placeholder hints**: Each field shows example values
- **Clear all filters**: Single button to reset all filters
- **Active filter indicator**: Clear button only shows when filters are active
- **Responsive design**: Adapts to different screen sizes

## User Experience

### Filter Workflow
1. User navigates to Executions page (`/executions`)
2. Sees filter panel above the executions table
3. Enters search terms in any combination of fields
4. Results update automatically via React Query
5. Can clear all filters with one click

### Example Use Cases

**Find all executions from the core pack:**
- Enter "core" in Pack field

**Find executions triggered by a specific rule:**
- Enter "core.on_timer" in Rule field

**Find executions for a specific action:**
- Enter "core.echo" in Action field

**Find executions handled by a specific executor:**
- Enter executor ID (e.g., "1") in Executor ID field

**Combine filters:**
- Pack: "core" + Trigger: "core.timer" = All core pack executions triggered by timer

### Visual Design
- Clean, card-based layout with shadow
- Consistent input styling with focus states
- Lucide icons for visual clarity
- Gray color scheme for secondary UI elements
- Blue accents for interactive elements

## Technical Details

### Backend Filtering Strategy
- **Primary filters** (status, enforcement): Database-level filtering
- **Secondary filters** (pack, action, rule, trigger, executor): In-memory filtering
- **Rationale**: Balances query complexity with performance for typical use cases

### Frontend State Management
- Uses `useState` for filter inputs (local UI state)
- Uses `useMemo` to convert filter strings to query parameters
- Uses React Query for server state with automatic caching
- Filters trigger new API calls via React Query's key invalidation

### Type Safety
- TypeScript interfaces ensure type safety throughout
- OpenAPI-generated client provides compile-time validation
- Rust backend validates all parameters

## API Endpoint

### Request
```
GET /api/v1/executions?pack_name=core&rule_ref=core.on_timer&action_ref=core.echo&trigger_ref=core.timer&executor=1&page=1&per_page=50
```

### Response
```json
{
  "data": [
    {
      "id": 123,
      "action_ref": "core.echo",
      "status": "completed",
      "parent": null,
      "enforcement": 456,
      "created": "2025-01-30T10:00:00Z",
      "updated": "2025-01-30T10:00:05Z"
    }
  ],
  "meta": {
    "page": 1,
    "per_page": 50,
    "total": 1
  }
}
```

## Testing Recommendations

### Manual Testing
1. **Individual filters**:
   - Test each filter field independently
   - Verify results match the filter criteria

2. **Combined filters**:
   - Test multiple filters together
   - Verify AND logic (all conditions must match)

3. **Clear filters**:
   - Apply filters, then click "Clear Filters"
   - Verify all fields reset and full list returns

4. **Edge cases**:
   - Empty results (no matches)
   - Special characters in filter values
   - Invalid executor ID (non-numeric)

5. **Performance**:
   - Test with large execution lists
   - Verify filtering remains responsive

### Integration Testing
- Verify API endpoint accepts all parameters
- Verify database queries return correct results
- Verify enforcement joins work correctly for rule/trigger filters
- Verify pagination works with filters applied

## Future Enhancements

1. **Status Filter Dropdown**: Add dropdown for execution status instead of/in addition to other filters
2. **Date Range Filtering**: Filter by creation/completion date
3. **Advanced Search**: Support wildcards, regex, or "contains" searches
4. **Save Filters**: Remember user's last filter settings
5. **Export Filtered Results**: Download filtered execution list as CSV/JSON
6. **Database Optimization**: Move in-memory filtering to database queries for better performance at scale
7. **URL Query Parameters**: Persist filters in URL for bookmarking/sharing
8. **Filter Presets**: Common filter combinations as quick-select buttons

## Dependencies
- React 19
- TanStack Query (React Query)
- Tailwind CSS
- Lucide React (icons)
- OpenAPI TypeScript Codegen

## Build Status
✅ Backend compilation successful  
✅ Frontend TypeScript compilation successful  
✅ Vite build successful  
✅ No errors or warnings

## Related Files
- Backend: `crates/api/src/dto/execution.rs`, `crates/api/src/routes/executions.rs`
- Frontend: `web/src/hooks/useExecutions.ts`, `web/src/pages/executions/ExecutionsPage.tsx`
- API Client: `web/src/api/services/ExecutionsService.ts` (generated)