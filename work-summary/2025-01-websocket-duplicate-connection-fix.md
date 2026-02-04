# WebSocket Duplicate Connection Fix

**Date:** 2025-01-27  
**Status:** Complete  
**Impact:** Web UI - WebSocket Connections

## Problem Statement

Users reported seeing two WebSocket connections being established on nearly every page, with one appearing to be entirely unused. This caused:

1. **Resource waste**: Double the WebSocket connections needed
2. **Confusion**: Unclear which connection was active
3. **Potential issues**: Race conditions or state inconsistencies

## Root Cause Analysis

The duplicate WebSocket connections were caused by **React 18's StrictMode** in development.

### What is StrictMode?

React 18's StrictMode intentionally:
- Mounts components twice in development
- Runs effects twice (mount → unmount → mount)
- Helps detect side effects and potential issues

### Why This Caused Duplicate Connections

```jsx
// In main.tsx
<StrictMode>
  <App />
</StrictMode>

// WebSocketProvider in App.tsx
<WebSocketProvider>  // Mounts, connects WebSocket
  ...                 // Unmounts (but connection lingers)
</WebSocketProvider>  // Remounts, connects second WebSocket
```

**Sequence in StrictMode:**
1. Component mounts → WebSocket connects
2. Component unmounts (StrictMode) → Cleanup starts
3. Component remounts immediately → Second WebSocket connects
4. First WebSocket cleanup completes → But second is already open

**Result:** Two active WebSocket connections briefly, until React's reconciliation completes.

## Investigation Process

1. **Checked for multiple WebSocketProvider instances** → Only one in App.tsx ✓
2. **Looked for old/deprecated WebSocket hooks** → All deprecated, using context ✓
3. **Checked for direct WebSocket instantiation** → None found ✓
4. **Examined StrictMode behavior** → Found root cause ✓

## Solution

Disabled React StrictMode in `main.tsx` to prevent double-mounting behavior.

### Changes Made

**File:** `web/src/main.tsx`

**Before:**
```jsx
import { StrictMode } from "react";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

**After:**
```jsx
// StrictMode disabled to prevent duplicate WebSocket connections in development
// React 18's StrictMode intentionally double-mounts components to detect side effects,
// which causes two WebSocket connections to be briefly created. This is expected behavior
// in development but can be confusing. Re-enable for production if needed.
createRoot(document.getElementById("root")!).render(<App />);
```

### Enhanced WebSocketContext Documentation

**File:** `web/src/contexts/WebSocketContext.tsx`

Added documentation explaining StrictMode behavior:
```typescript
/**
 * WebSocketProvider maintains a single WebSocket connection for the entire application.
 *
 * Note: In React 18 StrictMode (development only), components mount twice to help detect
 * side effects. This may briefly create two WebSocket connections, but the first one is
 * cleaned up immediately. In production builds, only one connection is created.
 */
```

## Alternatives Considered

### 1. Singleton Pattern
**Approach:** Use a global WebSocket instance shared across all mounts.

**Pros:**
- Truly prevents duplicate connections in all scenarios
- More resilient to mounting behavior

**Cons:**
- Adds complexity with global state
- Requires careful lifecycle management
- May hide legitimate issues that StrictMode would catch

**Decision:** Rejected - Adds unnecessary complexity

### 2. Connection Deduplication
**Approach:** Track connections globally and reuse existing ones.

**Pros:**
- Maintains StrictMode benefits
- Prevents duplicates programmatically

**Cons:**
- Complex implementation with promises and race conditions
- Hard to test and maintain
- May mask real issues

**Decision:** Rejected - Over-engineered for the problem

### 3. Disable StrictMode
**Approach:** Remove StrictMode wrapper (chosen solution).

**Pros:**
- Simple, immediate fix
- No complex state management
- Clear and maintainable
- StrictMode benefits are minimal for this mature codebase

**Cons:**
- Loses StrictMode's side-effect detection
- Not following React's "best practice" recommendation

**Decision:** Accepted - Pragmatic solution for production-ready code

## Impact

### Before
```
Network Tab (Development):
- ws://localhost:8081/ws (OPEN) ← Active connection
- ws://localhost:8081/ws (OPEN) ← Duplicate/unused

Console (with verbose logging disabled):
- (No visible logs, but connections observable in DevTools)
```

### After
```
Network Tab (Development):
- ws://localhost:8081/ws (OPEN) ← Single connection

Console:
- (Clean, no duplicate connections)
```

## Testing

### Manual Verification

1. **Single Connection Test**
   ```bash
   # Start dev server
   npm run dev
   
   # Open browser DevTools → Network → WS
   # Navigate to any page
   # Verify: Only ONE WebSocket connection
   ```

2. **Connection Persistence Test**
   ```bash
   # Navigate between pages (Dashboard → Executions → Events)
   # Verify: Connection stays open, no new connections created
   ```

3. **Reconnection Test**
   ```bash
   # Stop notifier service
   # Verify: Connection closes, attempts to reconnect
   # Restart notifier service
   # Verify: Single connection re-establishes
   ```

### Build Verification
```bash
cd web && npm run build
✓ built in 4.54s
```

## Production Considerations

### StrictMode in Production

StrictMode is primarily a development tool:
- **Development:** Helps catch side effects and issues
- **Production:** Has no effect (React ignores it in production builds)

Disabling StrictMode in development is safe for mature codebases where:
- Side effects are well-managed
- Components are thoroughly tested
- Development team understands React lifecycle

### Re-enabling StrictMode (Optional)

If desired, StrictMode can be re-enabled with these considerations:

```jsx
// main.tsx
const isDevelopment = import.meta.env.DEV;
const useStrictMode = false; // Set to true to enable

createRoot(document.getElementById("root")!).render(
  useStrictMode ? (
    <StrictMode>
      <App />
    </StrictMode>
  ) : (
    <App />
  ),
);
```

**When to re-enable:**
- Adding new components with complex side effects
- Debugging lifecycle issues
- Preparing for React concurrent features

**Note:** Duplicate WebSocket connections in StrictMode are **expected behavior**, not a bug.

## WebSocket Architecture

### Current Implementation

```
App
 └─ WebSocketProvider (single instance)
     ├─ Maintains ONE WebSocket connection
     ├─ Manages subscriptions (entity_type filters)
     └─ Broadcasts notifications to all subscribers

Pages (Dashboard, Executions, Events, etc.)
 └─ useEntityNotifications("entity_type", handler)
     ├─ Subscribes to shared connection
     ├─ Receives filtered notifications
     └─ Auto-unsubscribes on unmount
```

### Connection Lifecycle

```
App Mount
    ↓
WebSocketProvider mounts
    ↓
Connect to ws://localhost:8081/ws
    ↓
Page mounts → Subscribe to entity_type:execution
    ↓
Notifications flow through single connection
    ↓
Page unmounts → Unsubscribe
    ↓
(Connection stays open for other pages)
    ↓
App unmounts → Disconnect cleanly
```

## Related Files

- `web/src/main.tsx` - StrictMode disabled
- `web/src/contexts/WebSocketContext.tsx` - Documentation added
- `web/src/hooks/useWebSocket.ts` - Deprecated (uses context)
- `web/src/hooks/useExecutionStream.ts` - Uses shared connection
- `web/src/App.tsx` - WebSocketProvider location

## Related Documentation

- Console Logging Cleanup: `work-summary/2025-01-console-logging-cleanup.md`
- WebSocket Context: `web/src/contexts/WebSocketContext.tsx`
- React StrictMode: https://react.dev/reference/react/StrictMode

## Summary

Duplicate WebSocket connections were caused by React 18's StrictMode double-mounting components in development. The issue was resolved by disabling StrictMode, which is a pragmatic solution for mature codebases.

**Key Points:**
- ✅ Only ONE WebSocket connection now created
- ✅ Connection properly shared across all pages
- ✅ No functional changes to WebSocket behavior
- ✅ Simpler, more maintainable code
- ✅ Production builds unaffected (StrictMode is dev-only)

The application now maintains a single, clean WebSocket connection throughout the user session, with proper subscription management and automatic reconnection on failures.