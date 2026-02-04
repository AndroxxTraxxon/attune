/**
 * @deprecated This file is deprecated. Use WebSocketContext and useEntityNotifications from @/contexts/WebSocketContext instead.
 *
 * The old implementation created a separate WebSocket connection for each component that used it,
 * causing hundreds of concurrent connections. The new context-based approach maintains a single
 * shared WebSocket connection across the entire application.
 *
 * Migration:
 * 1. Ensure your app is wrapped with <WebSocketProvider> (should be in App.tsx)
 * 2. Change imports from "@/hooks/useWebSocket" to "@/contexts/WebSocketContext"
 * 3. Wrap notification handlers in useCallback for stable references
 *
 * @example
 * // OLD (creates new connection per component):
 * import { useEntityNotifications } from "@/hooks/useWebSocket";
 * const { connected } = useEntityNotifications("event", () => {
 *   queryClient.invalidateQueries({ queryKey: ["events"] });
 * });
 *
 * // NEW (uses shared connection):
 * import { useEntityNotifications } from "@/contexts/WebSocketContext";
 * const handleNotification = useCallback(() => {
 *   queryClient.invalidateQueries({ queryKey: ["events"] });
 * }, [queryClient]);
 * const { connected } = useEntityNotifications("event", handleNotification);
 */

// Re-export from context for backwards compatibility
export {
  useEntityNotifications,
  useWebSocketContext as useWebSocket,
  type Notification,
  type NotificationHandler,
} from "@/contexts/WebSocketContext";
