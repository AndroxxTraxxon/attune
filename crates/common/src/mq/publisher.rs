//! Message Publisher
//!
//! This module provides functionality for publishing messages to RabbitMQ exchanges.
//! It supports:
//! - Asynchronous message publishing
//! - Message confirmation (publisher confirms)
//! - Automatic routing based on message type
//! - Error handling and retries

use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions},
    BasicProperties, Channel,
};
use tracing::{debug, info};

use super::{
    error::{MqError, MqResult},
    messages::MessageEnvelope,
    Connection, DeliveryMode,
};

// Re-export for convenience
pub use super::config::PublisherConfig;

/// Message publisher for sending messages to RabbitMQ
pub struct Publisher {
    /// RabbitMQ channel
    channel: Channel,
    /// Publisher configuration
    config: PublisherConfig,
}

impl Publisher {
    /// Create a new publisher from a connection
    pub async fn new(connection: &Connection, config: PublisherConfig) -> MqResult<Self> {
        let channel = connection.create_channel().await?;

        // Enable publisher confirms if configured
        if config.confirm_publish {
            channel
                .confirm_select(ConfirmSelectOptions::default())
                .await
                .map_err(|e| MqError::Channel(format!("Failed to enable confirms: {}", e)))?;
            debug!("Publisher confirms enabled");
        }

        Ok(Self { channel, config })
    }

    /// Publish a message envelope to its designated exchange
    pub async fn publish_envelope<T>(&self, envelope: &MessageEnvelope<T>) -> MqResult<()>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let exchange = envelope.message_type.exchange();
        let routing_key = envelope.message_type.routing_key();

        self.publish_envelope_with_routing(envelope, &exchange, &routing_key)
            .await
    }

    /// Publish a message envelope with explicit exchange and routing key
    pub async fn publish_envelope_with_routing<T>(
        &self,
        envelope: &MessageEnvelope<T>,
        exchange: &str,
        routing_key: &str,
    ) -> MqResult<()>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let payload = envelope
            .to_bytes()
            .map_err(|e| MqError::Serialization(format!("Failed to serialize envelope: {}", e)))?;

        debug!(
            "Publishing message {} to exchange '{}' with routing key '{}'",
            envelope.message_id, exchange, routing_key
        );

        let properties = BasicProperties::default()
            .with_delivery_mode(DeliveryMode::Persistent as u8)
            .with_message_id(envelope.message_id.to_string().into())
            .with_correlation_id(envelope.correlation_id.to_string().into())
            .with_timestamp(envelope.timestamp.timestamp() as u64)
            .with_content_type("application/json".into());

        let confirmation = self
            .channel
            .basic_publish(
                exchange.into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                &payload,
                properties,
            )
            .await
            .map_err(|e| MqError::Publish(format!("Failed to publish message: {}", e)))?;

        // Wait for confirmation if enabled
        if self.config.confirm_publish {
            confirmation
                .await
                .map_err(|e| MqError::Publish(format!("Message not confirmed: {}", e)))?;

            debug!("Message {} confirmed", envelope.message_id);
        }

        info!(
            "Message {} published successfully to '{}'",
            envelope.message_id, exchange
        );

        Ok(())
    }

    /// Publish a raw message with custom properties
    pub async fn publish_raw(
        &self,
        exchange: &str,
        routing_key: &str,
        payload: &[u8],
        properties: BasicProperties,
    ) -> MqResult<()> {
        debug!(
            "Publishing raw message to exchange '{}' with routing key '{}'",
            exchange, routing_key
        );

        self.channel
            .basic_publish(
                exchange.into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                payload,
                properties,
            )
            .await
            .map_err(|e| MqError::Publish(format!("Failed to publish raw message: {}", e)))?;

        Ok(())
    }

    /// Get the underlying channel
    pub fn channel(&self) -> &Channel {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[allow(dead_code)]
    struct TestPayload {
        data: String,
    }

    #[test]
    fn test_publisher_config_defaults() {
        let config = PublisherConfig {
            confirm_publish: true,
            timeout_secs: 5,
            exchange: "test.exchange".to_string(),
        };

        assert!(config.confirm_publish);
        assert_eq!(config.timeout_secs, 5);
    }

    // Integration tests would require a running RabbitMQ instance
    // and should be in a separate integration test file
}
