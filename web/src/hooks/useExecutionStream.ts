import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useEntityNotifications } from "@/contexts/WebSocketContext";

interface UseExecutionStreamOptions {
  /**
   * Optional execution ID to filter updates for a specific execution.
   * If not provided, receives updates for all executions.
   */
  executionId?: number;

  /**
   * Whether the stream should be active.
   * Defaults to true.
   */
  enabled?: boolean;
}

/**
 * Check if an execution matches the given query parameters
 * Only checks fields that are reliably present in WebSocket payloads
 */
function executionMatchesParams(execution: any, params: any): boolean {
  if (!params) return true;

  // Check status filter (from API query parameters)
  if (params.status && execution.status !== params.status) {
    return false;
  }

  // Check action filter (always present)
  if (params.actionRef && execution.action_ref !== params.actionRef) {
    return false;
  }

  // Check pack filter (always present via action_ref)
  if (
    params.packName &&
    !execution.action_ref?.startsWith(params.packName + ".")
  ) {
    return false;
  }

  // Check executor filter (may be present)
  if (params.executor !== undefined && execution.executor !== params.executor) {
    return false;
  }

  // Note: rule_ref and trigger_ref are NOT checked here because they may not be
  // present in WebSocket payloads (they come from enforcement data which is
  // populated separately by the API). For these filters, we only update existing
  // executions, never add new ones.

  return true;
}

/**
 * Check if query params include filters not present in WebSocket payloads
 */
function hasUnsupportedFilters(params: any): boolean {
  if (!params) return false;
  return !!(params.ruleRef || params.triggerRef);
}

/**
 * Hook to subscribe to real-time execution updates via WebSocket.
 *
 * Automatically reconnects on connection loss and updates React Query cache
 * when execution updates are received.
 *
 * @example
 * ```tsx
 * // Listen to all execution updates
 * useExecutionStream();
 *
 * // Listen to updates for a specific execution
 * useExecutionStream({ executionId: 123 });
 * ```
 */
export function useExecutionStream(options: UseExecutionStreamOptions = {}) {
  const { executionId, enabled = true } = options;
  const queryClient = useQueryClient();

  const handleNotification = useCallback(
    (notification: any) => {
      // Filter by execution ID if specified
      if (executionId && notification.entity_id !== executionId) {
        return;
      }

      // Extract execution data from notification payload
      // The payload has a nested "data" field with the actual execution data
      const executionData =
        (notification.payload as any).data || notification.payload;

      // Update specific execution query if it exists
      queryClient.setQueryData(
        ["executions", notification.entity_id],
        (old: any) => {
          if (!old) return old;
          return {
            ...old,
            data: {
              ...old.data,
              ...executionData,
            },
          };
        },
      );

      // Update execution list queries by modifying existing data
      // We need to iterate manually to access query keys for filtering
      const queries = queryClient
        .getQueriesData({ queryKey: ["executions"], exact: false })
        .filter(([, data]) => data && Array.isArray((data as any)?.data));

      queries.forEach(([queryKey, oldData]) => {
        // Extract query params from the query key (format: ["executions", params])
        const queryParams = queryKey[1];

        const old = oldData as any;

        // Check if execution already exists in the list
        const existingIndex = old.data.findIndex(
          (exec: any) => exec.id === notification.entity_id,
        );

        let updatedData;
        if (existingIndex >= 0) {
          // Always update existing execution in the list
          updatedData = [...old.data];
          updatedData[existingIndex] = {
            ...updatedData[existingIndex],
            ...executionData,
          };

          // Note: We don't remove executions from cache based on filters.
          // The cache represents what the API query returned.
          // Client-side filtering (in the page component) handles what's displayed.
        } else {
          // For new executions, be conservative with filters we can't verify
          // If filters include rule_ref/trigger_ref, don't add new executions
          // (these fields may not be in WebSocket payload)
          if (hasUnsupportedFilters(queryParams)) {
            // Don't add new execution when using filters we can't verify
            return;
          }

          // Only add new execution if it matches the query parameters
          // (not the display filters - those are handled client-side)
          if (executionMatchesParams(executionData, queryParams)) {
            // Add to beginning and cap at 50 items to prevent performance issues
            updatedData = [executionData, ...old.data].slice(0, 50);
          } else {
            // Don't modify the list if the new execution doesn't match the query
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
    },
    [executionId, queryClient],
  );

  const { connected } = useEntityNotifications(
    "execution",
    handleNotification,
    enabled,
  );

  return {
    isConnected: connected,
    error: null,
  };
}
