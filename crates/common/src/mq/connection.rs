//! RabbitMQ Connection Management
//!
//! This module provides connection management for RabbitMQ, including:
//! - Connection pooling for efficient resource usage
//! - Automatic reconnection on connection failures
//! - Health checking for monitoring
//! - Channel creation and management

use lapin::{
    options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Channel, Connection as LapinConnection, ConnectionProperties, ExchangeKind,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{
    config::{ExchangeConfig, MessageQueueConfig, QueueConfig, RabbitMqConfig},
    error::{MqError, MqResult},
    ExchangeType,
};

/// RabbitMQ connection wrapper with reconnection support
#[derive(Clone)]
pub struct Connection {
    /// Underlying lapin connection (Arc-wrapped for sharing)
    connection: Arc<RwLock<Option<Arc<LapinConnection>>>>,
    /// Connection configuration
    config: RabbitMqConfig,
    /// Connection URL
    url: String,
}

impl Connection {
    /// Create a new connection from configuration
    pub async fn from_config(config: &MessageQueueConfig) -> MqResult<Self> {
        if !config.enabled {
            return Err(MqError::Config(
                "Message queue is disabled in configuration".to_string(),
            ));
        }

        config.rabbitmq.validate()?;

        let url = config.rabbitmq.connection_url();
        let connection = Self::connect_internal(&url, &config.rabbitmq).await?;

        Ok(Self {
            connection: Arc::new(RwLock::new(Some(Arc::new(connection)))),
            config: config.rabbitmq.clone(),
            url,
        })
    }

    /// Create a new connection with explicit URL
    pub async fn connect(url: &str) -> MqResult<Self> {
        let config = RabbitMqConfig::default();
        let connection = Self::connect_internal(url, &config).await?;

        Ok(Self {
            connection: Arc::new(RwLock::new(Some(Arc::new(connection)))),
            config,
            url: url.to_string(),
        })
    }

    /// Internal connection method
    async fn connect_internal(url: &str, _config: &RabbitMqConfig) -> MqResult<LapinConnection> {
        info!("Connecting to RabbitMQ at {}", url);

        let connection = LapinConnection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| MqError::Connection(format!("Failed to connect: {}", e)))?;

        info!("Successfully connected to RabbitMQ");
        Ok(connection)
    }

    /// Get or reconnect to RabbitMQ
    async fn get_connection(&self) -> MqResult<Arc<LapinConnection>> {
        let conn_guard = self.connection.read().await;
        if let Some(ref conn) = *conn_guard {
            if conn.status().connected() {
                return Ok(Arc::clone(conn));
            }
        }
        drop(conn_guard);

        // Connection is not available, attempt reconnect
        self.reconnect().await
    }

    /// Reconnect to RabbitMQ
    async fn reconnect(&self) -> MqResult<Arc<LapinConnection>> {
        let mut conn_guard = self.connection.write().await;

        // Double-check if another task already reconnected
        if let Some(ref conn) = *conn_guard {
            if conn.status().connected() {
                return Ok(Arc::clone(conn));
            }
        }

        warn!("Attempting to reconnect to RabbitMQ");

        let mut attempts = 0;
        let max_attempts = self.config.max_reconnect_attempts;

        loop {
            match Self::connect_internal(&self.url, &self.config).await {
                Ok(new_conn) => {
                    info!("Reconnected to RabbitMQ after {} attempts", attempts + 1);
                    let arc_conn = Arc::new(new_conn);
                    *conn_guard = Some(Arc::clone(&arc_conn));
                    return Ok(arc_conn);
                }
                Err(e) => {
                    attempts += 1;
                    if max_attempts > 0 && attempts >= max_attempts {
                        error!("Failed to reconnect after {} attempts: {}", attempts, e);
                        return Err(MqError::Connection(format!(
                            "Max reconnection attempts ({}) exceeded",
                            max_attempts
                        )));
                    }

                    warn!(
                        "Reconnection attempt {} failed: {}. Retrying in {:?}...",
                        attempts,
                        e,
                        self.config.reconnect_delay()
                    );
                    tokio::time::sleep(self.config.reconnect_delay()).await;
                }
            }
        }
    }

    /// Create a new channel
    pub async fn create_channel(&self) -> MqResult<Channel> {
        let connection = self.get_connection().await?;

        connection
            .create_channel()
            .await
            .map_err(|e| MqError::Channel(format!("Failed to create channel: {}", e)))
    }

    /// Check if connection is healthy
    pub async fn is_healthy(&self) -> bool {
        let conn_guard = self.connection.read().await;
        if let Some(ref conn) = *conn_guard {
            conn.status().connected()
        } else {
            false
        }
    }

    /// Close the connection
    pub async fn close(&self) -> MqResult<()> {
        let mut conn_guard = self.connection.write().await;
        if let Some(conn) = conn_guard.take() {
            conn.close(200, "Normal shutdown")
                .await
                .map_err(|e| MqError::Connection(format!("Failed to close connection: {}", e)))?;
            info!("Connection closed");
        }
        Ok(())
    }

    /// Declare an exchange
    pub async fn declare_exchange(&self, config: &ExchangeConfig) -> MqResult<()> {
        let channel = self.create_channel().await?;

        let kind = match config.r#type {
            ExchangeType::Direct => ExchangeKind::Direct,
            ExchangeType::Topic => ExchangeKind::Topic,
            ExchangeType::Fanout => ExchangeKind::Fanout,
            ExchangeType::Headers => ExchangeKind::Headers,
        };

        debug!(
            "Declaring exchange '{}' of type '{}'",
            config.name, config.r#type
        );

        channel
            .exchange_declare(
                &config.name,
                kind,
                ExchangeDeclareOptions {
                    durable: config.durable,
                    auto_delete: config.auto_delete,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                MqError::ExchangeDeclaration(format!(
                    "Failed to declare exchange '{}': {}",
                    config.name, e
                ))
            })?;

        info!("Exchange '{}' declared successfully", config.name);
        Ok(())
    }

    /// Declare a queue
    pub async fn declare_queue(&self, config: &QueueConfig) -> MqResult<()> {
        let channel = self.create_channel().await?;

        debug!("Declaring queue '{}'", config.name);

        channel
            .queue_declare(
                &config.name,
                QueueDeclareOptions {
                    durable: config.durable,
                    exclusive: config.exclusive,
                    auto_delete: config.auto_delete,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                MqError::QueueDeclaration(format!(
                    "Failed to declare queue '{}': {}",
                    config.name, e
                ))
            })?;

        info!("Queue '{}' declared successfully", config.name);
        Ok(())
    }

    /// Bind a queue to an exchange
    pub async fn bind_queue(&self, queue: &str, exchange: &str, routing_key: &str) -> MqResult<()> {
        let channel = self.create_channel().await?;

        debug!(
            "Binding queue '{}' to exchange '{}' with routing key '{}'",
            queue, exchange, routing_key
        );

        channel
            .queue_bind(
                queue,
                exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                MqError::QueueBinding(format!(
                    "Failed to bind queue '{}' to exchange '{}': {}",
                    queue, exchange, e
                ))
            })?;

        info!(
            "Queue '{}' bound to exchange '{}' with routing key '{}'",
            queue, exchange, routing_key
        );
        Ok(())
    }

    /// Declare a queue with dead letter exchange
    pub async fn declare_queue_with_dlx(
        &self,
        config: &QueueConfig,
        dlx_exchange: &str,
    ) -> MqResult<()> {
        let channel = self.create_channel().await?;

        debug!(
            "Declaring queue '{}' with dead letter exchange '{}'",
            config.name, dlx_exchange
        );

        let mut args = FieldTable::default();
        args.insert(
            "x-dead-letter-exchange".into(),
            lapin::types::AMQPValue::LongString(dlx_exchange.into()),
        );

        channel
            .queue_declare(
                &config.name,
                QueueDeclareOptions {
                    durable: config.durable,
                    exclusive: config.exclusive,
                    auto_delete: config.auto_delete,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|e| {
                MqError::QueueDeclaration(format!(
                    "Failed to declare queue '{}' with DLX: {}",
                    config.name, e
                ))
            })?;

        info!(
            "Queue '{}' declared with dead letter exchange '{}'",
            config.name, dlx_exchange
        );
        Ok(())
    }

    /// Setup complete infrastructure (exchanges, queues, bindings)
    pub async fn setup_infrastructure(&self, config: &MessageQueueConfig) -> MqResult<()> {
        info!("Setting up RabbitMQ infrastructure");

        // Declare exchanges
        self.declare_exchange(&config.rabbitmq.exchanges.events)
            .await?;
        self.declare_exchange(&config.rabbitmq.exchanges.executions)
            .await?;
        self.declare_exchange(&config.rabbitmq.exchanges.notifications)
            .await?;

        // Declare dead letter exchange if enabled
        if config.rabbitmq.dead_letter.enabled {
            let dlx_config = ExchangeConfig {
                name: config.rabbitmq.dead_letter.exchange.clone(),
                r#type: ExchangeType::Direct,
                durable: true,
                auto_delete: false,
            };
            self.declare_exchange(&dlx_config).await?;
        }

        // Declare queues with or without DLX
        let dlx_exchange = if config.rabbitmq.dead_letter.enabled {
            Some(config.rabbitmq.dead_letter.exchange.as_str())
        } else {
            None
        };

        if let Some(dlx) = dlx_exchange {
            self.declare_queue_with_dlx(&config.rabbitmq.queues.events, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.executions, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.enforcements, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.execution_requests, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.execution_status, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.execution_completed, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.inquiry_responses, dlx)
                .await?;
            self.declare_queue_with_dlx(&config.rabbitmq.queues.notifications, dlx)
                .await?;
        } else {
            self.declare_queue(&config.rabbitmq.queues.events).await?;
            self.declare_queue(&config.rabbitmq.queues.executions)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.enforcements)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.execution_requests)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.execution_status)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.execution_completed)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.inquiry_responses)
                .await?;
            self.declare_queue(&config.rabbitmq.queues.notifications)
                .await?;
        }

        // Bind queues to exchanges
        self.bind_queue(
            &config.rabbitmq.queues.events.name,
            &config.rabbitmq.exchanges.events.name,
            "#", // All events (topic exchange)
        )
        .await?;

        // LEGACY BINDING DISABLED: This was causing all messages to go to the legacy queue
        // instead of being routed to the new specific queues (execution_requests, enforcements, etc.)
        // self.bind_queue(
        //     &config.rabbitmq.queues.executions.name,
        //     &config.rabbitmq.exchanges.executions.name,
        //     "#", // All execution-related messages (topic exchange) - legacy, to be deprecated
        // )
        // .await?;

        // Bind new executor-specific queues
        self.bind_queue(
            &config.rabbitmq.queues.enforcements.name,
            &config.rabbitmq.exchanges.executions.name,
            "enforcement.#", // Enforcement messages
        )
        .await?;

        self.bind_queue(
            &config.rabbitmq.queues.execution_requests.name,
            &config.rabbitmq.exchanges.executions.name,
            "execution.requested", // Execution request messages
        )
        .await?;

        // Bind execution_status queue to status changed messages for ExecutionManager
        self.bind_queue(
            &config.rabbitmq.queues.execution_status.name,
            &config.rabbitmq.exchanges.executions.name,
            "execution.status.changed",
        )
        .await?;

        // Bind execution_completed queue to completed messages for CompletionListener
        self.bind_queue(
            &config.rabbitmq.queues.execution_completed.name,
            &config.rabbitmq.exchanges.executions.name,
            "execution.completed",
        )
        .await?;

        // Bind inquiry_responses queue to inquiry responded messages for InquiryHandler
        self.bind_queue(
            &config.rabbitmq.queues.inquiry_responses.name,
            &config.rabbitmq.exchanges.executions.name,
            "inquiry.responded",
        )
        .await?;

        self.bind_queue(
            &config.rabbitmq.queues.notifications.name,
            &config.rabbitmq.exchanges.notifications.name,
            "", // Fanout doesn't use routing key
        )
        .await?;

        info!("RabbitMQ infrastructure setup complete");
        Ok(())
    }
}

/// Connection pool for managing multiple RabbitMQ connections
pub struct ConnectionPool {
    /// Pool of connections
    connections: Vec<Connection>,
    /// Current index for round-robin selection
    current: Arc<RwLock<usize>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub async fn new(config: &MessageQueueConfig, size: usize) -> MqResult<Self> {
        let mut connections = Vec::with_capacity(size);

        for i in 0..size {
            debug!("Creating connection {} of {}", i + 1, size);
            let conn = Connection::from_config(config).await?;
            connections.push(conn);
        }

        info!("Connection pool created with {} connections", size);

        Ok(Self {
            connections,
            current: Arc::new(RwLock::new(0)),
        })
    }

    /// Get a connection from the pool (round-robin)
    pub async fn get(&self) -> MqResult<Connection> {
        if self.connections.is_empty() {
            return Err(MqError::Pool("Connection pool is empty".to_string()));
        }

        let mut current = self.current.write().await;
        let index = *current % self.connections.len();
        *current = (*current + 1) % self.connections.len();

        Ok(self.connections[index].clone())
    }

    /// Get pool size
    pub fn size(&self) -> usize {
        self.connections.len()
    }

    /// Check if all connections are healthy
    pub async fn is_healthy(&self) -> bool {
        for conn in &self.connections {
            if !conn.is_healthy().await {
                return false;
            }
        }
        true
    }

    /// Close all connections in the pool
    pub async fn close_all(&self) -> MqResult<()> {
        for conn in &self.connections {
            conn.close().await?;
        }
        info!("All connections in pool closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_url_parsing() {
        let config = RabbitMqConfig {
            host: "localhost".to_string(),
            port: 5672,
            username: "guest".to_string(),
            password: "guest".to_string(),
            vhost: "/".to_string(),
            ..Default::default()
        };

        let url = config.connection_url();
        assert_eq!(url, "amqp://guest:guest@localhost:5672//");
    }

    #[test]
    fn test_connection_validation() {
        let mut config = RabbitMqConfig::default();
        assert!(config.validate().is_ok());

        config.host = String::new();
        assert!(config.validate().is_err());
    }

    // Integration tests would go here (require running RabbitMQ instance)
    // These should be in a separate integration test file
}
