# WebSocket Connection Fix - January 2026

## Problem

The web UI was creating hundreds of concurrent WebSocket connections, particularly noticeable on pages like the Events page. The connections would begin immediately when the page loaded and continue indefinitely.

### Root Causes

1. **Multiple WebSocket instances per component**: Each component that used `useEntityNotifications` created its own separate WebSocket connection via the `useWebSocket` hook
2. **Unstable callback dependencies**: Notification handlers passed to `useEntityNotifications` were not wrapped in `useCallback`, causing them to be recreated on every render
3. **Reconnection loops**: The recreated callbacks triggered the `connect` function to be recreated (since `onNotification` was in its dependency array), potentially causing reconnection attempts
4. **No connection reuse**: There was no global WebSocket context - each component independently managed its own connection to the notifier service

## Solution

Implemented a context-based WebSocket provider that maintains a **single shared connection** across the entire application.

### Key Changes

1. **Created `WebSocketContext.tsx`** (`web/src/contexts/WebSocketContext.tsx`)
   - `WebSocketProvider`: Manages a single WebSocket connection for the entire app
   - Multiple components can subscribe to different entity types on the same connection
   - Uses a Map of filter -> Set<handlers> to support multiple handlers per filter
   - Only sends subscribe/unsubscribe messages when the first/last handler is added/removed

2. **Updated `App.tsx`**
   - Wrapped the application with `<WebSocketProvider>` alongside `AuthProvider` and `QueryClientProvider`
   - This ensures one connection is established for all authenticated users

3. **Updated `EventsPage.tsx`**
   - Changed import from `@/hooks/useWebSocket` to `@/contexts/WebSocketContext`
   - Wrapped notification handler in `useCallback` for stable reference
   - No functional changes to the page behavior

4. **Deprecated old `useWebSocket.ts`**
   - Replaced implementation with re-exports from the new context
   - Added detailed deprecation notice with migration instructions
   - Maintains backwards compatibility for any code still importing from the old location

### Architecture

**Before:**
```
EventsPage → useEntityNotifications → useWebSocket → new WebSocket()
ExecutionsPage → useEntityNotifications → useWebSocket → new WebSocket()
DashboardPage → useEntityNotifications → useWebSocket → new WebSocket()
// Result: 3 separate WebSocket connections
```

**After:**
```
App → WebSocketProvider → single WebSocket connection
  ↓
EventsPage → useEntityNotifications → useWebSocketContext → shared connection
ExecutionsPage → useEntityNotifications → useWebSocketContext → shared connection
DashboardPage → useEntityNotifications → useWebSocketContext → shared connection
// Result: 1 shared WebSocket connection with multiple subscriptions
```

### Technical Details

**Stable Handler Pattern:**
The new `useEntityNotifications` uses a ref-based pattern to avoid re-subscriptions:
- Stores the handler in a ref that updates on each render
- Creates a stable wrapper function that calls the current handler from the ref
- Only subscribes/unsubscribes when `connected`, `enabled`, or `entityType` changes

**Subscription Management:**
- Tracks `Map<filter, Set<NotificationHandler>>`
- Only sends WebSocket subscribe message when first handler for a filter is added
- Only sends WebSocket unsubscribe message when last handler for a filter is removed
- Allows multiple components to listen to the same entity type efficiently

## Testing

1. **Build verification**: `npm run build` - successful ✓
2. **Dev server**: Started on port 3001 ✓
3. **Expected behavior**: 
   - Only 1 WebSocket connection per browser tab
   - Connection persists across page navigation
   - Multiple pages can subscribe to different entity types on the same connection
   - Console should show single "[WebSocket] Connected" message

## Migration Guide

For any future pages that need real-time updates:

```typescript
import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useEntityNotifications } from "@/contexts/WebSocketContext";

function MyPage() {
  const queryClient = useQueryClient();
  
  // Wrap handler in useCallback for stable reference
  const handleNotification = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["myEntity"] });
  }, [queryClient]);
  
  const { connected } = useEntityNotifications("myEntity", handleNotification);
  
  // Rest of component...
}
```

## Performance Impact

- **Reduced WebSocket connections**: From hundreds → 1 per browser tab
- **Reduced network overhead**: Single connection handles all subscriptions
- **Reduced server load**: Notifier service handles 1 connection instead of N connections per user
- **Improved stability**: No reconnection storms from unstable callbacks

## Files Changed

- **Created**: `web/src/contexts/WebSocketContext.tsx` (315 lines)
- **Modified**: `web/src/App.tsx` (added WebSocketProvider wrapper)
- **Modified**: `web/src/pages/events/EventsPage.tsx` (updated to use context + useCallback)
- **Modified**: `web/src/hooks/useWebSocket.ts` (deprecated, now re-exports from context)

## Related Issues

This fix addresses the fundamental architectural issue with WebSocket connection management in the web UI. Any page displaying real-time data should now use the context-based approach.

## Next Steps

1. Monitor WebSocket connection count in browser DevTools (Network tab, WS filter)
2. Verify real-time updates still work correctly on all pages
3. Consider adding WebSocket connection status indicator in the UI header
4. Update any other pages that would benefit from real-time updates (executions, dashboard, etc.)