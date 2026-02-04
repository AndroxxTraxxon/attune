# Enforcement WebSocket Streaming Implementation

**Date**: 2026-02-04  
**Feature**: Real-time enforcement monitoring via WebSocket  
**Status**: ✅ Implemented and Deployed

## Overview

Implemented real-time WebSocket streaming for the enforcements page, matching the pattern used by the executions page. This allows users to see enforcement updates in real-time without manual page refreshes.

## Problem

The enforcements page was using a simple invalidation approach for WebSocket notifications:
- When a notification arrived, it would invalidate the entire query cache and refetch
- This was inefficient and didn't support intelligent filtering
- The page lacked the sophisticated filtering and streaming capabilities present in the executions page

## Solution

Created a dedicated `useEnforcementStream` hook and refactored the enforcements page to match the executions page pattern.

### Changes Made

#### 1. Created `useEnforcementStream` Hook

**File**: `attune/web/src/hooks/useEnforcementStream.ts` (new file)

Key features:
- Subscribes to enforcement entity notifications via WebSocket
- Updates React Query cache in real-time without full refetches
- Supports filtering by enforcement ID (for detail pages)
- Intelligently matches enforcement data against query parameters
- Updates existing enforcements in-place
- Adds new enforcements to lists when they match filters
- Invalidates related queries (rules and events)

**Smart filtering logic**:
```typescript
function enforcementMatchesParams(enforcement: any, params: any): boolean {
  // Check status filter
  if (params.status && enforcement.status !== params.status) return false;
  
  // Check event filter
  if (params.event !== undefined && enforcement.event !== params.event) return false;
  
  // Check rule filter
  if (params.rule !== undefined && enforcement.rule !== params.rule) return false;
  
  // Check trigger_ref filter
  if (params.triggerRef && enforcement.trigger_ref !== params.triggerRef) return false;
  
  return true;
}
```

#### 2. Refactored Enforcements Page

**File**: `attune/web/src/pages/enforcements/EnforcementsPage.tsx`

**Before**:
- Simple filter inputs with immediate state updates
- Direct query invalidation on WebSocket notifications
- Limited filtering capabilities

**After**:
- Memoized filter components to prevent re-renders on WebSocket updates
- Debounced filter inputs (500ms for text, 300ms for selections)
- Multi-select status filter with client-side filtering for multiple selections
- Integrated `useEnforcementStream` hook
- Real-time connection status indicator ("Live Updates")
- Consistent UI with executions page

**Key improvements**:
1. **Debouncing**: Prevents excessive API calls while typing
2. **Memoization**: Prevents unnecessary re-renders of filter inputs
3. **Multi-status filtering**: API supports single status, client-side handles multiple
4. **Client-side rule_ref filtering**: Supplements API filtering for unsupported fields
5. **Real-time updates**: New enforcements appear instantly, existing ones update in-place

## Architecture Pattern

The implementation follows the same pattern as executions:

```
WebSocket Notification → useEnforcementStream → React Query Cache Update → UI Re-render
                                                                           ↓
                                                                  (No API refetch needed)
```

**Benefits**:
- Instant updates without full page refreshes
- Efficient cache management (update in-place, not refetch)
- Respects current filters and pagination
- Minimal network traffic
- Seamless user experience

## User Experience

### Real-time Features
- ✅ New enforcements appear at the top of the list instantly
- ✅ Status changes update in real-time
- ✅ Green "Live Updates" indicator shows connection status
- ✅ Filtering is responsive with debouncing
- ✅ Multiple status selections supported

### Filter Options
1. **Rule** (client-side): Filter by rule reference (e.g., `core.on_timer`)
2. **Trigger** (API): Filter by trigger reference (e.g., `core.webhook`)
3. **Event ID** (API): Filter by event ID (e.g., `123`)
4. **Status** (API + client-side): Single status via API, multiple via client

### UI Consistency
- Matches executions page design and behavior
- Uses same filter components and styling
- Same "Live Updates" indicator
- Consistent table layout and navigation

## Technical Details

### WebSocket Integration
- Uses existing `useEntityNotifications` context
- Listens to "enforcement" entity type
- Processes notifications with stable callbacks
- Handles connection/disconnection gracefully

### Query Cache Management
```typescript
// Update specific enforcement query
queryClient.setQueryData(["enforcements", enforcementId], ...);

// Update enforcement lists
queryClient.getQueriesData({ queryKey: ["enforcements"], exact: false })
  .forEach(([queryKey, oldData]) => {
    // Smart merge logic here
  });

// Invalidate related queries
queryClient.invalidateQueries({ queryKey: ["rules", ruleId, "enforcements"] });
queryClient.invalidateQueries({ queryKey: ["events", eventId, "enforcements"] });
```

### Performance Optimizations
1. **Memoized components**: Filter inputs don't re-render on WebSocket updates
2. **Debounced filters**: Reduce API calls during typing
3. **Selective cache updates**: Only update relevant queries
4. **Client-side filtering**: Reduce server load for multi-status filters

## Deployment

```bash
# Build web UI
cd web && npm run build

# Rebuild and restart web container
docker compose build web
docker compose restart web
```

## Testing

### Manual Testing Steps
1. Open enforcements page
2. Verify "Live Updates" indicator appears (green with pulse)
3. Trigger a webhook or rule that creates an enforcement
4. Verify new enforcement appears at the top instantly
5. Apply various filters and verify they work correctly
6. Select multiple statuses and verify client-side filtering works
7. Check that existing enforcements update in real-time

### Expected Behavior
- New enforcements appear without page refresh
- Status changes update immediately
- Filters are responsive with no lag
- Connection indicator shows accurate status
- Page remains usable during high-frequency updates

## Comparison with Executions Page

| Feature | Executions Page | Enforcements Page |
|---------|----------------|-------------------|
| Real-time streaming | ✅ | ✅ |
| Debounced filters | ✅ | ✅ |
| Multi-select status | ✅ | ✅ |
| Memoized components | ✅ | ✅ |
| Live indicator | ✅ | ✅ |
| Smart cache updates | ✅ | ✅ |
| Client-side filtering | ✅ | ✅ |

## Files Changed

1. **Created**: `attune/web/src/hooks/useEnforcementStream.ts` (185 lines)
2. **Refactored**: `attune/web/src/pages/enforcements/EnforcementsPage.tsx` (456 → 384 lines, cleaner code)

## Related Work

- Based on pattern from `useExecutionStream` hook
- Uses existing `useEntityNotifications` context
- Leverages `MultiSelect` component from executions page
- Consistent with overall WebSocket architecture

## Future Improvements

Possible enhancements:
1. Add pagination support with streaming (like executions)
2. Add sorting options
3. Add bulk actions (cancel multiple enforcements)
4. Add export functionality
5. Add advanced filtering (date ranges, payload inspection)

## Impact

- ✅ Users can monitor enforcements in real-time
- ✅ Reduced server load (fewer polling requests)
- ✅ Improved user experience (instant updates)
- ✅ Consistent UI/UX across pages
- ✅ Better filtering capabilities
- ✅ More responsive interface

## Lessons Learned

1. **Pattern reuse is valuable**: The executions page pattern was well-designed and easily adaptable
2. **Debouncing is essential**: Prevents filter spam during typing
3. **Memoization matters**: Prevents unnecessary re-renders during streaming
4. **Client-side filtering complements API**: Allows more flexible multi-filtering
5. **Consistent UX is important**: Users benefit from familiar patterns across pages