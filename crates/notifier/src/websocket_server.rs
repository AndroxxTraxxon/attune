//! WebSocket server for real-time notifications

use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

use attune_common::config::Config;

use crate::service::Notification;
use crate::subscriber_manager::{ClientId, SubscriberManager, SubscriptionFilter};

/// WebSocket server for handling client connections
pub struct WebSocketServer {
    config: Config,
    pub notification_tx: broadcast::Sender<Notification>,
    subscriber_manager: Arc<SubscriberManager>,
    shutdown_tx: broadcast::Sender<()>,
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(
        config: Config,
        notification_tx: broadcast::Sender<Notification>,
        subscriber_manager: Arc<SubscriberManager>,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            config,
            notification_tx,
            subscriber_manager,
            shutdown_tx,
        }
    }

    /// Clone method for spawning tasks
    pub fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            notification_tx: self.notification_tx.clone(),
            subscriber_manager: self.subscriber_manager.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }

    /// Start the WebSocket server
    pub async fn start(&self) -> Result<()> {
        let app_state = Arc::new(AppState {
            notification_tx: self.notification_tx.clone(),
            subscriber_manager: self.subscriber_manager.clone(),
        });

        // Build router with WebSocket endpoint
        let app = Router::new()
            .route("/ws", get(websocket_handler))
            .route("/health", get(health_handler))
            .route("/stats", get(stats_handler))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(app_state);

        let notifier_config = self
            .config
            .notifier
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Notifier configuration not found in config"))?;

        let addr = format!("{}:{}", notifier_config.host, notifier_config.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .context(format!("Failed to bind to {}", addr))?;

        info!("WebSocket server listening on {}", addr);

        axum::serve(listener, app)
            .await
            .context("WebSocket server error")?;

        Ok(())
    }
}

/// Shared application state
struct AppState {
    #[allow(dead_code)]
    notification_tx: broadcast::Sender<Notification>,
    subscriber_manager: Arc<SubscriberManager>,
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

/// Stats endpoint
async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = serde_json::json!({
        "connected_clients": state.subscriber_manager.client_count(),
        "total_subscriptions": state.subscriber_manager.subscription_count(),
    });
    (StatusCode::OK, Json(stats))
}

/// WebSocket handler - upgrades HTTP connection to WebSocket
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle individual WebSocket connection
async fn handle_websocket(socket: WebSocket, state: Arc<AppState>) {
    let client_id = state.subscriber_manager.generate_client_id();
    info!("New WebSocket connection: {}", client_id);

    // Split the socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Create channel for sending notifications to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<Notification>();

    // Register the subscriber
    state
        .subscriber_manager
        .register(client_id.clone(), None, tx);

    // Send welcome message
    let welcome = ClientMessage::Welcome {
        client_id: client_id.clone(),
        message: "Connected to Attune Notifier".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = ws_sender.send(Message::Text(json.into())).await;
    }

    // Spawn task to handle outgoing notifications
    let client_id_clone = client_id.clone();
    let subscriber_manager_clone = state.subscriber_manager.clone();
    let outgoing_task = tokio::spawn(async move {
        while let Some(notification) = rx.recv().await {
            // Serialize notification to JSON
            match serde_json::to_string(&notification) {
                Ok(json) => {
                    if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                        error!("Failed to send notification to {}: {}", client_id_clone, e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize notification: {}", e);
                }
            }
        }
        debug!("Outgoing task stopped for client: {}", client_id_clone);
        subscriber_manager_clone.unregister(&client_id_clone);
    });

    // Handle incoming messages from client (subscriptions, etc.)
    let subscriber_manager_clone = state.subscriber_manager.clone();
    let client_id_clone = client_id.clone();
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) =
                    handle_client_message(&client_id_clone, &text, &subscriber_manager_clone).await
                {
                    error!("Error handling client message: {}", e);
                }
            }
            Ok(Message::Binary(_)) => {
                warn!("Received binary message from {}, ignoring", client_id_clone);
            }
            Ok(Message::Close(_)) => {
                info!("Client {} closed connection", client_id_clone);
                break;
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                // Handled automatically by axum
            }
            Err(e) => {
                error!("WebSocket error for {}: {}", client_id_clone, e);
                break;
            }
        }
    }

    // Clean up
    subscriber_manager_clone.unregister(&client_id);
    outgoing_task.abort();
    info!("WebSocket connection closed: {}", client_id);
}

/// Handle incoming message from client
async fn handle_client_message(
    client_id: &ClientId,
    message: &str,
    subscriber_manager: &SubscriberManager,
) -> Result<()> {
    let msg: ServerMessage =
        serde_json::from_str(message).context("Failed to parse client message")?;

    match msg {
        ServerMessage::Subscribe { filter } => {
            let subscription_filter = parse_subscription_filter(&filter)?;
            subscriber_manager.subscribe(client_id, subscription_filter);
            info!("Client {} subscribed to: {:?}", client_id, filter);
        }
        ServerMessage::Unsubscribe { filter } => {
            let subscription_filter = parse_subscription_filter(&filter)?;
            subscriber_manager.unsubscribe(client_id, &subscription_filter);
            info!("Client {} unsubscribed from: {:?}", client_id, filter);
        }
        ServerMessage::Ping => {
            debug!("Received ping from {}", client_id);
            // Pong is handled automatically
        }
    }

    Ok(())
}

/// Parse subscription filter from string
fn parse_subscription_filter(filter_str: &str) -> Result<SubscriptionFilter> {
    // Format: "type:value" or "all"
    if filter_str == "all" {
        return Ok(SubscriptionFilter::All);
    }

    let parts: Vec<&str> = filter_str.split(':').collect();
    if parts.len() < 2 {
        anyhow::bail!("Invalid filter format: {}", filter_str);
    }

    match parts[0] {
        "entity_type" => Ok(SubscriptionFilter::EntityType(parts[1].to_string())),
        "notification_type" => Ok(SubscriptionFilter::NotificationType(parts[1].to_string())),
        "user" => {
            let user_id: i64 = parts[1].parse().context("Invalid user ID")?;
            Ok(SubscriptionFilter::User(user_id))
        }
        "entity" => {
            if parts.len() < 3 {
                anyhow::bail!("Entity filter requires type and id: entity:type:id");
            }
            let entity_id: i64 = parts[2].parse().context("Invalid entity ID")?;
            Ok(SubscriptionFilter::Entity {
                entity_type: parts[1].to_string(),
                entity_id,
            })
        }
        _ => anyhow::bail!("Unknown filter type: {}", parts[0]),
    }
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ClientMessage {
    #[serde(rename = "welcome")]
    Welcome { client_id: String, message: String },

    #[serde(rename = "notification")]
    Notification(Notification),

    #[serde(rename = "error")]
    Error { message: String },
}

/// Messages sent from client to server
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "subscribe")]
    Subscribe { filter: String },

    #[serde(rename = "unsubscribe")]
    Unsubscribe { filter: String },

    #[serde(rename = "ping")]
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subscription_filter_all() {
        let filter = parse_subscription_filter("all").unwrap();
        assert_eq!(filter, SubscriptionFilter::All);
    }

    #[test]
    fn test_parse_subscription_filter_entity_type() {
        let filter = parse_subscription_filter("entity_type:execution").unwrap();
        assert_eq!(
            filter,
            SubscriptionFilter::EntityType("execution".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_filter_notification_type() {
        let filter =
            parse_subscription_filter("notification_type:execution_status_changed").unwrap();
        assert_eq!(
            filter,
            SubscriptionFilter::NotificationType("execution_status_changed".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_filter_user() {
        let filter = parse_subscription_filter("user:123").unwrap();
        assert_eq!(filter, SubscriptionFilter::User(123));
    }

    #[test]
    fn test_parse_subscription_filter_entity() {
        let filter = parse_subscription_filter("entity:execution:456").unwrap();
        assert_eq!(
            filter,
            SubscriptionFilter::Entity {
                entity_type: "execution".to_string(),
                entity_id: 456
            }
        );
    }

    #[test]
    fn test_parse_subscription_filter_invalid() {
        let result = parse_subscription_filter("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_subscription_filter_invalid_user_id() {
        let result = parse_subscription_filter("user:not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_subscription_filter_entity_missing_id() {
        let result = parse_subscription_filter("entity:execution");
        assert!(result.is_err());
    }
}
