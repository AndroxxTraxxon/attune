//! PostgreSQL LISTEN/NOTIFY listener for SSE broadcasting

use sqlx::postgres::{PgListener, PgPool};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Start listening to PostgreSQL notifications and broadcast them to SSE clients
pub async fn start_postgres_listener(
    db: PgPool,
    broadcast_tx: broadcast::Sender<String>,
) -> anyhow::Result<()> {
    info!("Starting PostgreSQL notification listener for SSE broadcasting");

    // Create a listener
    let mut listener = PgListener::connect_with(&db).await?;

    // Subscribe to the notifications channel
    listener.listen("attune_notifications").await?;

    info!("Listening on channel: attune_notifications");

    // Process notifications in a loop
    loop {
        match listener.recv().await {
            Ok(notification) => {
                let payload = notification.payload();
                debug!("Received notification: {}", payload);

                // Broadcast to all SSE clients
                match broadcast_tx.send(payload.to_string()) {
                    Ok(receiver_count) => {
                        debug!("Broadcasted notification to {} SSE clients", receiver_count);
                    }
                    Err(e) => {
                        // This happens when there are no active receivers, which is normal
                        debug!("No active SSE clients to receive notification: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Error receiving notification: {}", e);

                // If the connection is lost, try to reconnect
                warn!("Attempting to reconnect to PostgreSQL listener...");

                match PgListener::connect_with(&db).await {
                    Ok(mut new_listener) => {
                        match new_listener.listen("attune_notifications").await {
                            Ok(_) => {
                                info!("Successfully reconnected to PostgreSQL listener");
                                listener = new_listener;
                            }
                            Err(e) => {
                                error!("Failed to resubscribe after reconnect: {}", e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to reconnect to PostgreSQL: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }
}
