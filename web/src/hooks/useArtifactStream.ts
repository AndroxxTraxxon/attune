import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useEntityNotifications } from "@/contexts/WebSocketContext";

interface UseArtifactStreamOptions {
  /**
   * Optional execution ID to filter artifact updates for a specific execution.
   * If not provided, receives updates for all artifacts.
   */
  executionId?: number;

  /**
   * Whether the stream should be active.
   * Defaults to true.
   */
  enabled?: boolean;
}

/** Shape of data coming from WebSocket notifications for artifacts */
interface ArtifactNotification {
  entity_id: number;
  entity_type: string;
  notification_type: string;
  payload: ArtifactNotificationPayload;
  timestamp: string;
}

/** The raw payload from the PostgreSQL trigger for artifact notifications */
interface ArtifactNotificationPayload {
  execution?: number;
  type?: string;
  name?: string | null;
  progress_percent?: number | null;
  progress_message?: string | null;
  progress_entries?: number | null;
  [key: string]: unknown;
}

/**
 * Hook to subscribe to real-time artifact updates via WebSocket.
 *
 * Listens to `artifact_created` and `artifact_updated` notifications from the
 * PostgreSQL LISTEN/NOTIFY system, and invalidates relevant React Query caches
 * so that artifact lists and detail views update in real time.
 *
 * For progress-type artifacts, the notification payload includes a progress
 * summary (`progress_percent`, `progress_message`, `progress_entries`) extracted
 * by the database trigger so that the UI can update inline progress indicators
 * without a separate API call.
 *
 * @example
 * ```tsx
 * // Listen to all artifact updates
 * useArtifactStream();
 *
 * // Listen to artifacts for a specific execution
 * useArtifactStream({ executionId: 123 });
 * ```
 */
export function useArtifactStream(options: UseArtifactStreamOptions = {}) {
  const { executionId, enabled = true } = options;
  const queryClient = useQueryClient();

  const handleNotification = useCallback(
    (notification: ArtifactNotification) => {
      const payload = notification.payload;

      // If we're filtering by execution ID, only process matching artifacts
      if (executionId && payload?.execution !== executionId) {
        return;
      }

      const artifactId = notification.entity_id;
      const artifactExecution = payload?.execution;

      // Invalidate the specific artifact query (used by ProgressDetail, TextFileDetail)
      queryClient.invalidateQueries({
        queryKey: ["artifacts", artifactId],
      });

      // Invalidate the execution artifacts list query
      if (artifactExecution) {
        queryClient.invalidateQueries({
          queryKey: ["artifacts", "execution", artifactExecution],
        });
      }

      // For progress artifacts, also update cached data directly with the
      // summary from the notification payload to provide instant feedback
      // before the invalidation refetch completes.
      if (payload?.type === "progress" && payload?.progress_percent != null) {
        queryClient.setQueryData(
          ["artifact_progress", artifactExecution],
          (old: ArtifactProgressSummary | undefined) => ({
            ...old,
            artifactId,
            name: payload.name ?? null,
            percent: payload.progress_percent as number,
            message: payload.progress_message ?? null,
            entries: payload.progress_entries ?? 0,
            timestamp: notification.timestamp,
          }),
        );
      }
    },
    [executionId, queryClient],
  );

  const { connected } = useEntityNotifications(
    "artifact",
    handleNotification,
    enabled,
  );

  return {
    isConnected: connected,
  };
}

/**
 * Lightweight progress summary extracted from artifact WebSocket notifications.
 * Available immediately via the `artifact_progress` query key without an API call.
 */
export interface ArtifactProgressSummary {
  artifactId: number;
  name: string | null;
  percent: number;
  message: string | null;
  entries: number;
  timestamp: string;
}

/**
 * Hook to read the latest progress summary pushed by WebSocket notifications.
 *
 * This does NOT make any API calls — it only reads from the React Query cache
 * which is populated by `useArtifactStream`. Returns `null` if no progress
 * notification has been received yet for the given execution.
 *
 * For the initial load (before any WebSocket message arrives), the component
 * should fall back to the polling-based `useExecutionArtifacts` data.
 */
export function useArtifactProgress(
  executionId: number | undefined,
): ArtifactProgressSummary | null {
  const queryClient = useQueryClient();

  if (!executionId) return null;

  const data = queryClient.getQueryData<ArtifactProgressSummary>([
    "artifact_progress",
    executionId,
  ]);

  return data ?? null;
}
