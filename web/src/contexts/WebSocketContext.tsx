/* eslint-disable react-refresh/only-export-components -- exporting hooks alongside WebSocketProvider is standard React pattern */
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useCallback,
  ReactNode,
} from "react";

export interface Notification {
  notification_type: string;
  entity_type: string;
  entity_id: number;
  user_id?: number;
  payload: unknown;
  timestamp: string;
}

export type NotificationHandler = (notification: Notification) => void;

interface WebSocketContextValue {
  connected: boolean;
  subscribe: (filter: string, handler: NotificationHandler) => void;
  unsubscribe: (filter: string, handler: NotificationHandler) => void;
}

const WebSocketContext = createContext<WebSocketContextValue | null>(null);

interface WebSocketProviderProps {
  children: ReactNode;
  url?: string;
  autoConnect?: boolean;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

/**
 * WebSocketProvider maintains a single WebSocket connection for the entire application.
 *
 * Note: In React 18 StrictMode (development only), components mount twice to help detect
 * side effects. This may briefly create two WebSocket connections, but the first one is
 * cleaned up immediately. In production builds, only one connection is created.
 */
export function WebSocketProvider({
  children,
  url: providedUrl,
  autoConnect = true,
  reconnectInterval = 5000,
  maxReconnectAttempts = 10,
}: WebSocketProviderProps) {
  // Construct WebSocket URL from base (add /ws path if not present)
  const baseUrl =
    providedUrl || import.meta.env.VITE_WS_URL || "ws://localhost:8081";
  const url = baseUrl.endsWith("/ws") ? baseUrl : `${baseUrl}/ws`;

  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const shouldConnectRef = useRef(autoConnect);
  const reconnectAttemptsRef = useRef(0);
  const isConnectingRef = useRef(false);

  // Map of filter -> Set of handlers
  const subscriptionsRef = useRef<Map<string, Set<NotificationHandler>>>(
    new Map(),
  );

  const connect = useCallback(() => {
    // Don't reconnect if we're already connected, connecting, or explicitly disconnected
    if (
      wsRef.current?.readyState === WebSocket.OPEN ||
      isConnectingRef.current ||
      !shouldConnectRef.current
    ) {
      return;
    }

    // Check max reconnect attempts
    if (reconnectAttemptsRef.current >= maxReconnectAttempts) {
      console.error(
        `[WebSocket] Max reconnection attempts (${maxReconnectAttempts}) reached. Giving up.`,
      );
      shouldConnectRef.current = false;
      return;
    }

    const attemptConnect = () => {
      try {
        isConnectingRef.current = true;
        const ws = new WebSocket(url);

        ws.onopen = () => {
          setConnected(true);
          isConnectingRef.current = false;
          reconnectAttemptsRef.current = 0; // Reset attempts on successful connection

          // Re-subscribe to all filters
          subscriptionsRef.current.forEach((_, filter) => {
            ws.send(
              JSON.stringify({
                type: "subscribe",
                filter,
              }),
            );
          });
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);

            // Handle different message types
            if (message.type === "welcome") {
              // Connection acknowledged
            } else if (message.notification_type) {
              // This is a notification - dispatch to all relevant handlers
              const notification = message as Notification;

              // Call handlers for entity_type:* subscriptions
              const entityFilter = `entity_type:${notification.entity_type}`;
              const handlers = subscriptionsRef.current.get(entityFilter);
              if (handlers) {
                handlers.forEach((handler) => {
                  try {
                    handler(notification);
                  } catch (error) {
                    console.error("[WebSocket] Handler error:", error);
                  }
                });
              }
            }
          } catch (error) {
            console.error("[WebSocket] Failed to parse message:", error);
          }
        };

        ws.onerror = (error) => {
          console.error("[WebSocket] Error:", error);
        };

        ws.onclose = () => {
          setConnected(false);
          isConnectingRef.current = false;
          wsRef.current = null;

          // Attempt to reconnect if we should still be connected
          if (
            shouldConnectRef.current &&
            reconnectAttemptsRef.current < maxReconnectAttempts
          ) {
            reconnectAttemptsRef.current += 1;
            const delay = Math.min(
              reconnectInterval * reconnectAttemptsRef.current,
              30000,
            );
            // Attempting reconnection with backoff
            reconnectTimeoutRef.current = setTimeout(() => {
              attemptConnect();
            }, delay);
          }
        };

        wsRef.current = ws;
      } catch (error) {
        console.error("[WebSocket] Failed to connect:", error);
        isConnectingRef.current = false;

        // Retry connection with backoff
        if (
          shouldConnectRef.current &&
          reconnectAttemptsRef.current < maxReconnectAttempts
        ) {
          reconnectAttemptsRef.current += 1;
          const delay = Math.min(
            reconnectInterval * reconnectAttemptsRef.current,
            30000,
          );
          reconnectTimeoutRef.current = setTimeout(() => {
            attemptConnect();
          }, delay);
        }
      }
    };

    attemptConnect();
  }, [url, reconnectInterval, maxReconnectAttempts]);

  const disconnect = useCallback(() => {
    shouldConnectRef.current = false;
    isConnectingRef.current = false;
    reconnectAttemptsRef.current = 0;

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (wsRef.current) {
      // Close connection cleanly
      if (wsRef.current.readyState === WebSocket.OPEN) {
        wsRef.current.close(1000, "Client disconnecting");
      }
      wsRef.current = null;
    }

    setConnected(false);
  }, []);

  const subscribe = useCallback(
    (filter: string, handler: NotificationHandler) => {
      // Add handler to the set for this filter
      if (!subscriptionsRef.current.has(filter)) {
        subscriptionsRef.current.set(filter, new Set());
      }
      const handlers = subscriptionsRef.current.get(filter)!;
      const hadHandlers = handlers.size > 0;
      handlers.add(handler);

      // Only send subscribe message if this is the first handler for this filter
      if (!hadHandlers && wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(
          JSON.stringify({
            type: "subscribe",
            filter,
          }),
        );
      }
    },
    [],
  );

  const unsubscribe = useCallback(
    (filter: string, handler: NotificationHandler) => {
      const handlers = subscriptionsRef.current.get(filter);
      if (!handlers) return;

      handlers.delete(handler);

      // If no more handlers for this filter, unsubscribe from the server
      if (handlers.size === 0) {
        subscriptionsRef.current.delete(filter);

        if (wsRef.current?.readyState === WebSocket.OPEN) {
          wsRef.current.send(
            JSON.stringify({
              type: "unsubscribe",
              filter,
            }),
          );
        }
      }
    },
    [],
  );

  // Connect on mount if autoConnect is enabled
  useEffect(() => {
    if (autoConnect) {
      shouldConnectRef.current = true;
      connect();
    }

    return () => {
      disconnect();
    };
  }, [autoConnect, connect, disconnect]);

  const value: WebSocketContextValue = {
    connected,
    subscribe,
    unsubscribe,
  };

  return (
    <WebSocketContext.Provider value={value}>
      {children}
    </WebSocketContext.Provider>
  );
}

export function useWebSocketContext(): WebSocketContextValue {
  const context = useContext(WebSocketContext);
  if (!context) {
    throw new Error(
      "useWebSocketContext must be used within WebSocketProvider",
    );
  }
  return context;
}

/**
 * Hook for subscribing to specific entity type notifications
 * Uses the shared WebSocket connection from context
 *
 * @example
 * // Subscribe to all execution updates
 * useEntityNotifications('execution', () => {
 *   queryClient.invalidateQueries(['executions']);
 * });
 */
export function useEntityNotifications(
  entityType: string,
  onNotification: NotificationHandler,
  enabled = true,
) {
  const { connected, subscribe, unsubscribe } = useWebSocketContext();

  // Stable reference to the handler — updated on every render via effect
  const handlerRef = useRef(onNotification);

  // Update ref when handler changes (but don't cause re-subscription)
  useEffect(() => {
    handlerRef.current = onNotification;
  }, [onNotification]);

  // Create a stable wrapper function once via useMemo (no ref access during render)
  const stableHandler = useMemo<NotificationHandler>(
    () => (notification) => {
      handlerRef.current(notification);
    },
    [], // intentionally empty — handlerRef is stable
  );

  useEffect(() => {
    if (!connected || !enabled) return;

    const filter = `entity_type:${entityType}`;

    subscribe(filter, stableHandler);

    return () => {
      unsubscribe(filter, stableHandler);
    };
  }, [connected, enabled, entityType, subscribe, unsubscribe, stableHandler]);

  return { connected };
}
