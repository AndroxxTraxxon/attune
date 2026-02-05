import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useEntityNotifications } from "@/contexts/WebSocketContext";

interface UseEnforcementStreamOptions {
  /**
   * Optional enforcement ID to filter updates for a specific enforcement.
   * If not provided, receives updates for all enforcements.
   */
  enforcementId?: number;

  /**
   * Whether the stream should be active.
   * Defaults to true.
   */
  enabled?: boolean;
}

/**
 * Check if an enforcement matches the given query parameters
 * Only checks fields that are reliably present in WebSocket payloads
 */
function enforcementMatchesParams(enforcement: any, params: any): boolean {
  if (!params) return true;

  // Check status filter
  if (params.status && enforcement.status !== params.status) {
    return false;
  }

  // Check event filter
  if (params.event !== undefined && enforcement.event !== params.event) {
    return false;
  }

  // Check rule filter
  if (params.rule !== undefined && enforcement.rule !== params.rule) {
    return false;
  }

  // Check trigger_ref filter (always present)
  if (params.triggerRef && enforcement.trigger_ref !== params.triggerRef) {
    return false;
  }

  // Note: rule_ref is NOT checked here for new enforcements because it may not be
  // present in WebSocket payloads. For this filter, we only update existing
  // enforcements, never add new ones.

  return true;
}

/**
 * Check if query params include filters not present in WebSocket payloads
 */
function hasUnsupportedFilters(params: any): boolean {
  if (!params) return false;
  // Currently all enforcement filters are supported in WebSocket payloads
  return false;
}

/**
 * Hook to subscribe to real-time enforcement updates via WebSocket.
 *
 * Automatically reconnects on connection loss and updates React Query cache
 * when enforcement updates are received.
 *
 * @example
 * ```tsx
 * // Listen to all enforcement updates
 * useEnforcementStream();
 *
 * // Listen to updates for a specific enforcement
 * useEnforcementStream({ enforcementId: 123 });
 * ```
 */
export function useEnforcementStream(
  options: UseEnforcementStreamOptions = {},
) {
  const { enforcementId, enabled = true } = options;
  const queryClient = useQueryClient();

  const handleNotification = useCallback(
    (notification: any) => {
      // Filter by enforcement ID if specified
      if (enforcementId && notification.entity_id !== enforcementId) {
        return;
      }

      // Extract enforcement data from notification payload (flat structure)
      const enforcementData = notification.payload as any;

      // Update specific enforcement query if it exists
      queryClient.setQueryData(
        ["enforcements", notification.entity_id],
        (old: any) => {
          if (!old) return old;
          return {
            ...old,
            data: {
              ...old.data,
              ...enforcementData,
            },
          };
        },
      );

      // Update enforcement list queries by modifying existing data
      // We need to iterate manually to access query keys for filtering
      const queries = queryClient
        .getQueriesData({ queryKey: ["enforcements"], exact: false })
        .filter(([, data]) => data && Array.isArray((data as any)?.data));

      queries.forEach(([queryKey, oldData]) => {
        // Extract query params from the query key (format: ["enforcements", params])
        const queryParams = queryKey[1];

        const old = oldData as any;

        // Check if enforcement already exists in the list
        const existingIndex = old.data.findIndex(
          (enf: any) => enf.id === notification.entity_id,
        );

        let updatedData;
        if (existingIndex >= 0) {
          // Always update existing enforcement in the list
          updatedData = [...old.data];
          updatedData[existingIndex] = {
            ...updatedData[existingIndex],
            ...enforcementData,
          };

          // Note: We don't remove enforcements from cache based on filters.
          // The cache represents what the API query returned.
          // Client-side filtering (in the page component) handles what's displayed.
        } else {
          // For new enforcements, be conservative with filters we can't verify
          if (hasUnsupportedFilters(queryParams)) {
            // Don't add new enforcement when using filters we can't verify
            return;
          }

          // Only add new enforcement if it matches the query parameters
          if (enforcementMatchesParams(enforcementData, queryParams)) {
            // Add to beginning and cap at 50 items to prevent performance issues
            updatedData = [enforcementData, ...old.data].slice(0, 50);
          } else {
            // Don't modify the list if the new enforcement doesn't match the query
            return;
          }
        }

        // Update the query with the new data
        queryClient.setQueryData(queryKey, {
          ...old,
          data: updatedData,
          pagination: {
            ...old.pagination,
            total_items: (old.pagination?.total_items || 0) + 1,
          },
        });
      });

      // Also update related queries (rules and events enforcements)
      if (enforcementData.rule) {
        queryClient.invalidateQueries({
          queryKey: ["rules", enforcementData.rule, "enforcements"],
        });
      }
      if (enforcementData.event) {
        queryClient.invalidateQueries({
          queryKey: ["events", enforcementData.event, "enforcements"],
        });
      }
    },
    [enforcementId, queryClient],
  );

  const { connected } = useEntityNotifications(
    "enforcement",
    handleNotification,
    enabled,
  );

  return {
    isConnected: connected,
    error: null,
  };
}
