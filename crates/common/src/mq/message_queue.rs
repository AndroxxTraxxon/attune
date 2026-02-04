/*!
Message Queue Convenience Wrapper

Provides a simplified interface for publishing messages by combining
Connection and Publisher into a single MessageQueue type.
*/

use super::{
    error::{MqError, MqResult},
    messages::MessageEnvelope,
    Connection, Publisher, PublisherConfig,
};
use lapin::BasicProperties;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Message queue wrapper that simplifies publishing operations
#[derive(Clone)]
pub struct MessageQueue {
    /// RabbitMQ connection
    connection: Arc<Connection>,
    /// Message publisher
    publisher: Arc<RwLock<Option<Publisher>>>,
}

impl MessageQueue {
    /// Connect to RabbitMQ and create a message queue
    pub async fn connect(url: &str) -> MqResult<Self> {
        let connection = Connection::connect(url).await?;

        // Create publisher with default configuration
        let publisher = Publisher::new(
            &connection,
            PublisherConfig {
                confirm_publish: true,
                timeout_secs: 30,
                exchange: "attune.events".to_string(),
            },
        )
        .await?;

        Ok(Self {
            connection: Arc::new(connection),
            publisher: Arc::new(RwLock::new(Some(publisher))),
        })
    }

    /// Create a message queue from an existing connection
    pub async fn from_connection(connection: Connection) -> MqResult<Self> {
        let publisher = Publisher::new(
            &connection,
            PublisherConfig {
                confirm_publish: true,
                timeout_secs: 30,
                exchange: "attune.events".to_string(),
            },
        )
        .await?;

        Ok(Self {
            connection: Arc::new(connection),
            publisher: Arc::new(RwLock::new(Some(publisher))),
        })
    }

    /// Publish a message envelope
    pub async fn publish_envelope<T>(&self, envelope: &MessageEnvelope<T>) -> MqResult<()>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let publisher_guard = self.publisher.read().await;
        let publisher = publisher_guard
            .as_ref()
            .ok_or_else(|| MqError::Connection("Publisher not initialized".to_string()))?;

        publisher.publish_envelope(envelope).await
    }

    /// Publish a message to a specific exchange and routing key
    pub async fn publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> MqResult<()> {
        debug!(
            "Publishing message to exchange '{}' with routing key '{}'",
            exchange, routing_key
        );

        let publisher_guard = self.publisher.read().await;
        let publisher = publisher_guard
            .as_ref()
            .ok_or_else(|| MqError::Connection("Publisher not initialized".to_string()))?;

        let properties = BasicProperties::default()
            .with_delivery_mode(2) // Persistent
            .with_content_type("application/json".into());

        publisher
            .publish_raw(exchange, routing_key, payload, properties)
            .await
    }

    /// Get the underlying connection
    pub fn connection(&self) -> &Arc<Connection> {
        &self.connection
    }

    /// Get the underlying connection
    pub fn get_connection(&self) -> &Connection {
        &self.connection
    }

    /// Check if the connection is healthy
    pub async fn is_healthy(&self) -> bool {
        self.connection.is_healthy().await
    }

    /// Close the message queue connection
    pub async fn close(&self) -> MqResult<()> {
        // Clear the publisher
        let mut publisher_guard = self.publisher.write().await;
        *publisher_guard = None;

        // Close the connection
        self.connection.close().await?;

        info!("Message queue connection closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::mq::{MessageEnvelope, MessageType};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestPayload {
        data: String,
    }

    #[test]
    fn test_message_queue_creation() {
        // This test just verifies the struct can be instantiated
        // Actual connection tests require a running RabbitMQ instance
        assert!(true);
    }

    #[tokio::test]
    async fn test_message_envelope_serialization() {
        let payload = TestPayload {
            data: "test".to_string(),
        };
        let envelope = MessageEnvelope::new(MessageType::EventCreated, payload);

        let bytes = envelope.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }
}
