# Rule Enable/Disable Toggle Switch Implementation

**Date:** 2025-01-30  
**Status:** Complete

## Overview
Added a toggle switch to the Rule Detail page that allows authenticated users to enable or disable rules directly from the UI, with proper permission checking and visual feedback.

## Changes Made

### 1. Backend API (Already Existed)
The backend already had the necessary endpoints:
- `POST /api/v1/rules/{ref}/enable` - Enable a rule
- `POST /api/v1/rules/{ref}/disable` - Disable a rule

Both endpoints:
- Require authentication (`RequireAuth` middleware)
- Update the rule's `enabled` status in the database
- Publish RabbitMQ messages (`RuleEnabled`/`RuleDisabled`) to notify the sensor service
- Return the updated rule in the response

### 2. Frontend Hooks (`web/src/hooks/useRules.ts`)
Added two new React hooks for toggling rule status:

```typescript
// Enable rule hook
export function useEnableRule() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (ref: string) => {
      const response = await RulesService.enableRule({ ref });
      return response;
    },
    onSuccess: (_, ref) => {
      queryClient.invalidateQueries({ queryKey: ["rules"] });
      queryClient.invalidateQueries({ queryKey: ["rules", ref] });
    },
  });
}

// Disable rule hook
export function useDisableRule() {
  // Similar implementation for disabling
}
```

These hooks:
- Use TanStack Query's `useMutation` for state management
- Automatically invalidate relevant queries after success to refresh the UI
- Handle API calls to the enable/disable endpoints

### 3. UI Component (`web/src/pages/rules/RulesPage.tsx`)

#### Updated Imports
- Added `useEnableRule` and `useDisableRule` hooks
- Added `useAuth` context for permission checking

#### Added State and Handler
```typescript
const { isAuthenticated } = useAuth();
const enableRule = useEnableRule();
const disableRule = useDisableRule();
const [isTogglingEnabled, setIsTogglingEnabled] = useState(false);

const handleToggleEnabled = async () => {
  if (!rule?.data) return;
  
  setIsTogglingEnabled(true);
  try {
    if (rule.data.enabled) {
      await disableRule.mutateAsync(ruleRef);
    } else {
      await enableRule.mutateAsync(ruleRef);
    }
  } catch (err) {
    console.error("Failed to toggle rule enabled status:", err);
  } finally {
    setIsTogglingEnabled(false);
  }
};
```

#### Toggle Switch UI
Replaced the static status badge with an interactive toggle switch:
- **Toggle Switch**: Tailwind CSS-based toggle with proper styling
- **Status Label**: Shows "Enabled" (green) / "Disabled" (gray) / "Updating..." (gray)
- **Disabled State**: Toggle is disabled when:
  - User is not authenticated (`!isAuthenticated`)
  - Toggle operation is in progress (`isTogglingEnabled`)
- **Visual Feedback**:
  - Focus ring on keyboard interaction
  - Smooth transition animation
  - Color change (gray → blue when enabled)
  - Loading state during API call

## User Experience

### Toggle Location
The toggle switch is positioned near the top of the Rule Detail page, next to the rule title, making it easily accessible and immediately visible.

### Permission Handling
- **Authenticated Users**: Can toggle the switch freely
- **Unauthenticated Users**: Toggle is disabled (grayed out)
- **During Update**: Toggle shows "Updating..." and is disabled to prevent double-clicks

### Visual States
1. **Enabled**: Blue toggle, green "Enabled" label
2. **Disabled**: Gray toggle, gray "Disabled" label
3. **Loading**: Gray toggle (disabled), "Updating..." label
4. **No Permission**: Gray toggle (disabled), current status label

### Real-time Updates
- On successful toggle, the UI immediately updates via React Query cache invalidation
- The sensor service receives notifications via RabbitMQ and adjusts monitoring accordingly
- Rule list view automatically reflects the new status

## Technical Details

### Permission System
Currently uses simple authentication check (`isAuthenticated`):
- Any authenticated user can enable/disable rules
- Backend validates JWT token via `RequireAuth` middleware
- Future: Can be extended to check fine-grained permissions when RBAC is fully implemented

### State Management
- Uses TanStack Query for server state management
- Optimistic updates are not used (waiting for server confirmation)
- Cache invalidation ensures all views stay in sync
- Local `isTogglingEnabled` state prevents UI race conditions

### Error Handling
- Errors are logged to console
- Toggle returns to previous state on failure
- User sees the actual current state from the server

## Testing Recommendations

### Manual Testing
1. **Toggle Enabled → Disabled**:
   - Navigate to an enabled rule
   - Click the toggle switch
   - Verify it shows "Updating..."
   - Verify it changes to "Disabled" after API response
   - Check that sensor service stops monitoring the trigger

2. **Toggle Disabled → Enabled**:
   - Navigate to a disabled rule
   - Click the toggle switch
   - Verify it changes to "Enabled"
   - Check that sensor service starts monitoring the trigger

3. **Permission Check**:
   - Log out
   - Navigate to a rule detail page
   - Verify toggle is disabled

4. **Double-click Prevention**:
   - Toggle a rule
   - Try clicking again during update
   - Verify second click is ignored

### Integration Testing
- Verify RabbitMQ messages are published correctly
- Verify sensor service receives and processes messages
- Verify database `enabled` field is updated
- Verify rule executions respect the enabled flag

## Future Enhancements

1. **Fine-grained Permissions**: Add RBAC checks for `rule:enable` and `rule:disable` permissions
2. **Optimistic Updates**: Update UI immediately, rollback on error
3. **Toast Notifications**: Show success/error messages
4. **Bulk Operations**: Add ability to enable/disable multiple rules at once
5. **Audit Logging**: Track who enabled/disabled rules and when
6. **Confirmation Dialog**: Optional confirmation for critical rules

## Dependencies
- React 19
- TanStack Query (React Query)
- Tailwind CSS
- Existing API endpoints (already implemented)

## Build Status
✅ TypeScript compilation successful  
✅ Vite build successful  
✅ No ESLint warnings