//! Message Consumer
//!
//! This module provides functionality for consuming messages from RabbitMQ queues.
//! It supports:
//! - Asynchronous message consumption
//! - Manual and automatic acknowledgments
//! - Message deserialization
//! - Error handling and retries
//! - Graceful shutdown

use futures::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions},
    types::FieldTable,
    Channel, Consumer as LapinConsumer,
};
use tracing::{debug, error, info, warn};

use super::{
    error::{MqError, MqResult},
    messages::MessageEnvelope,
    Connection,
};

// Re-export for convenience
pub use super::config::ConsumerConfig;

/// Message consumer for receiving messages from RabbitMQ
pub struct Consumer {
    /// RabbitMQ channel
    channel: Channel,
    /// Consumer configuration
    config: ConsumerConfig,
}

impl Consumer {
    /// Create a new consumer from a connection
    pub async fn new(connection: &Connection, config: ConsumerConfig) -> MqResult<Self> {
        let channel = connection.create_channel().await?;

        // Set prefetch count (QoS)
        channel
            .basic_qos(config.prefetch_count, BasicQosOptions::default())
            .await
            .map_err(|e| MqError::Channel(format!("Failed to set QoS: {}", e)))?;

        debug!(
            "Consumer created for queue '{}' with prefetch count {}",
            config.queue, config.prefetch_count
        );

        Ok(Self { channel, config })
    }

    /// Start consuming messages from the queue
    pub async fn start(&self) -> MqResult<LapinConsumer> {
        info!("Starting consumer for queue '{}'", self.config.queue);

        let consumer = self
            .channel
            .basic_consume(
                &self.config.queue,
                &self.config.tag,
                BasicConsumeOptions {
                    no_ack: self.config.auto_ack,
                    exclusive: self.config.exclusive,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                MqError::Consume(format!(
                    "Failed to start consuming from queue '{}': {}",
                    self.config.queue, e
                ))
            })?;

        info!(
            "Consumer started for queue '{}' with tag '{}'",
            self.config.queue, self.config.tag
        );

        Ok(consumer)
    }

    /// Consume messages with a handler function
    pub async fn consume_with_handler<T, F, Fut>(&self, mut handler: F) -> MqResult<()>
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + Send + 'static,
        F: FnMut(MessageEnvelope<T>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = MqResult<()>> + Send,
    {
        let mut consumer = self.start().await?;

        info!("Consuming messages from queue '{}'", self.config.queue);

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;

                    debug!(
                        "Received message with delivery tag {} from queue '{}'",
                        delivery_tag, self.config.queue
                    );

                    // Deserialize message envelope
                    let envelope = match MessageEnvelope::<T>::from_bytes(&delivery.data) {
                        Ok(env) => env,
                        Err(e) => {
                            error!("Failed to deserialize message: {}. Rejecting message.", e);

                            if !self.config.auto_ack {
                                // Reject message without requeue (send to DLQ)
                                if let Err(nack_err) = self
                                    .channel
                                    .basic_nack(
                                        delivery_tag,
                                        BasicNackOptions {
                                            requeue: false,
                                            multiple: false,
                                        },
                                    )
                                    .await
                                {
                                    error!("Failed to nack message: {}", nack_err);
                                }
                            }
                            continue;
                        }
                    };

                    debug!(
                        "Processing message {} of type {:?}",
                        envelope.message_id, envelope.message_type
                    );

                    // Call handler
                    match handler(envelope.clone()).await {
                        Ok(()) => {
                            debug!("Message {} processed successfully", envelope.message_id);

                            if !self.config.auto_ack {
                                // Acknowledge message
                                if let Err(e) = self
                                    .channel
                                    .basic_ack(delivery_tag, BasicAckOptions::default())
                                    .await
                                {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Handler failed for message {}: {}", envelope.message_id, e);

                            if !self.config.auto_ack {
                                // Reject message - will be requeued or sent to DLQ
                                let requeue = e.is_retriable();

                                warn!(
                                    "Rejecting message {} (requeue: {})",
                                    envelope.message_id, requeue
                                );

                                if let Err(nack_err) = self
                                    .channel
                                    .basic_nack(
                                        delivery_tag,
                                        BasicNackOptions {
                                            requeue,
                                            multiple: false,
                                        },
                                    )
                                    .await
                                {
                                    error!("Failed to nack message: {}", nack_err);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    // Continue processing, connection issues will trigger reconnection
                }
            }
        }

        warn!("Consumer for queue '{}' stopped", self.config.queue);
        Ok(())
    }

    /// Get the underlying channel
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Get the queue name
    pub fn queue(&self) -> &str {
        &self.config.queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consumer_config() {
        let config = ConsumerConfig {
            queue: "test.queue".to_string(),
            tag: "test-consumer".to_string(),
            prefetch_count: 10,
            auto_ack: false,
            exclusive: false,
        };

        assert_eq!(config.queue, "test.queue");
        assert_eq!(config.tag, "test-consumer");
        assert_eq!(config.prefetch_count, 10);
        assert!(!config.auto_ack);
        assert!(!config.exclusive);
    }

    // Integration tests would require a running RabbitMQ instance
    // and should be in a separate integration test file
}
