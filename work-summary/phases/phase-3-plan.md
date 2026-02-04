# Phase 3: Message Queue Infrastructure - Implementation Plan

**Date:** 2024-01-13  
**Status:** Planning  
**Priority:** HIGH

## Overview

Phase 3 focuses on implementing a robust message queue infrastructure using RabbitMQ to enable asynchronous, distributed communication between Attune services. This is critical for decoupling the API service from execution services and enabling scalable, reliable automation workflows.

## Goals

1. **Decouple Services**: Enable services to communicate asynchronously
2. **Reliability**: Ensure messages are not lost (persistence, acknowledgments)
3. **Scalability**: Support multiple workers and horizontal scaling
4. **Observability**: Track message flow and processing
5. **Error Handling**: Dead letter queues and retry mechanisms

## Architecture Overview

```
┌─────────────────┐
│   API Service   │
│                 │
│  (Publishers)   │
└────────┬────────┘
         │
         │ Publishes events/executions
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                     RabbitMQ                            │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   Events     │  │  Executions  │  │Notifications │ │
│  │   Exchange   │  │   Exchange   │  │   Exchange   │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                 │                  │         │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌──────▼───────┐ │
│  │ event_queue  │  │ exec_queue   │  │ notif_queue  │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│                                                         │
└─────────────────────────────────────────────────────────┘
         │                 │                  │
         │                 │                  │
    ┌────▼────┐       ┌────▼────┐       ┌────▼────┐
    │ Sensor  │       │Executor │       │Notifier │
    │ Service │       │ Service │       │ Service │
    │         │       │         │       │         │
    │(Consumer)│      │(Consumer)│      │(Consumer)│
    └─────────┘       └─────────┘       └─────────┘
```

## Technology Choice: RabbitMQ vs Redis

### Decision: **RabbitMQ (lapin)**

**Reasons:**
- ✅ Purpose-built for message queuing
- ✅ Built-in acknowledgments and persistence
- ✅ Dead letter queues and retry mechanisms
- ✅ Complex routing with exchanges and bindings
- ✅ Better message guarantees
- ✅ Already in workspace dependencies

**Redis Pub/Sub Alternative:**
- ❌ No message persistence by default
- ❌ No built-in acknowledgments
- ❌ Simpler routing capabilities
- ✅ Could use for real-time notifications (Phase 7)

## Implementation Phases

### Phase 3.1: Message Queue Setup (Foundation)

**Goal:** Create core RabbitMQ connection and management infrastructure

**Files to Create:**
```
crates/common/src/mq/
├── mod.rs              - Module exports and common types
├── connection.rs       - RabbitMQ connection pool management
├── config.rs           - Message queue configuration
├── error.rs            - MQ-specific error types
└── health.rs           - Health check for MQ connection
```

**Tasks:**
1. Create `mq` module structure
2. Implement connection management with pooling
3. Add configuration support (host, port, credentials, etc.)
4. Implement graceful connection handling and reconnection
5. Add health checks for monitoring

**Estimated Time:** 2-3 days

---

### Phase 3.2: Message Type Definitions

**Goal:** Define all message schemas for inter-service communication

**Files to Create:**
```
crates/common/src/mq/
├── messages/
│   ├── mod.rs          - Message trait and common utilities
│   ├── event.rs        - Event-related messages
│   ├── execution.rs    - Execution-related messages
│   ├── inquiry.rs      - Inquiry-related messages
│   └── notification.rs - Notification messages
```

**Message Types to Define:**

#### Event Messages
- `EventCreated` - New event detected by sensor
  - Fields: event_id, trigger_id, sensor_id, payload, timestamp

#### Execution Messages
- `ExecutionRequested` - New execution requested
  - Fields: execution_id, action_id, enforcement_id, parameters
- `ExecutionStatusChanged` - Execution status update
  - Fields: execution_id, old_status, new_status, timestamp
- `ExecutionCompleted` - Execution finished (success/failure)
  - Fields: execution_id, status, result, error

#### Inquiry Messages
- `InquiryCreated` - New inquiry needs human response
  - Fields: inquiry_id, execution_id, prompt, timeout
- `InquiryResponded` - User responded to inquiry
  - Fields: inquiry_id, execution_id, response, user_id

#### Notification Messages
- `NotificationCreated` - System notification
  - Fields: type, target, payload, timestamp

**Design Principles:**
- All messages implement `Message` trait
- Serializable to JSON for wire format
- Include correlation IDs for tracing
- Versioned for backwards compatibility
- Include timestamp and metadata

**Estimated Time:** 2-3 days

---

### Phase 3.3: Publisher Implementation

**Goal:** Enable services to publish messages to queues

**Files to Create:**
```
crates/common/src/mq/
├── publisher.rs        - Message publishing interface
└── exchanges.rs        - Exchange declarations
```

**Features:**
- Async message publishing
- Automatic routing based on message type
- Confirmation of delivery
- Error handling and retries
- Batch publishing support (future)

**Publisher Interface:**
```rust
pub struct Publisher {
    channel: Channel,
    config: PublisherConfig,
}

impl Publisher {
    pub async fn publish<M: Message>(&self, message: &M) -> Result<()>;
    pub async fn publish_with_routing_key<M: Message>(
        &self, 
        message: &M, 
        routing_key: &str
    ) -> Result<()>;
}
```

**Exchange Configuration:**
- `attune.events` - Topic exchange for events
- `attune.executions` - Direct exchange for executions
- `attune.notifications` - Fanout exchange for notifications

**Estimated Time:** 2 days

---

### Phase 3.4: Consumer Implementation

**Goal:** Enable services to consume messages from queues

**Files to Create:**
```
crates/common/src/mq/
├── consumer.rs         - Message consumption interface
└── queues.rs           - Queue declarations
```

**Features:**
- Async message consumption
- Automatic acknowledgment (configurable)
- Manual acknowledgment for at-least-once delivery
- Prefetch limits for backpressure
- Consumer cancellation and cleanup
- Message deserialization with error handling

**Consumer Interface:**
```rust
pub struct Consumer {
    channel: Channel,
    queue: String,
    config: ConsumerConfig,
}

impl Consumer {
    pub async fn consume<M, F>(&mut self, handler: F) -> Result<()>
    where
        M: Message,
        F: Fn(M) -> Future<Output = Result<()>>;
        
    pub async fn start(&mut self) -> Result<ConsumerStream>;
}
```

**Queue Configuration:**
- `attune.events.queue` - Event processing queue
- `attune.executions.queue` - Execution request queue
- `attune.notifications.queue` - Notification delivery queue

**Queue Features:**
- Durable queues (survive broker restart)
- Message TTL for stale messages
- Max priority for urgent messages
- Dead letter exchange binding

**Estimated Time:** 3 days

---

### Phase 3.5: Dead Letter Queues & Error Handling

**Goal:** Handle failed message processing gracefully

**Files to Create:**
```
crates/common/src/mq/
├── dlq.rs              - Dead letter queue management
└── retry.rs            - Retry logic and policies
```

**Features:**
- Automatic DLQ creation for each main queue
- Failed message routing to DLQ
- Retry count tracking in message headers
- Exponential backoff for retries
- Max retry limits
- DLQ monitoring and alerting

**DLQ Strategy:**
```
Main Queue → [Processing Fails] → DLQ
                                   ↓
                          [Manual Review / Replay]
```

**Retry Policy:**
- Max retries: 3
- Backoff: 1s, 5s, 30s
- After max retries → move to DLQ
- Track retry count in message headers

**Estimated Time:** 2 days

---

### Phase 3.6: Testing & Validation

**Goal:** Comprehensive testing of MQ infrastructure

**Files to Create:**
```
crates/common/tests/
├── mq_integration_tests.rs
└── mq_fixtures.rs
```

**Test Categories:**

#### Unit Tests
- Message serialization/deserialization
- Configuration parsing
- Error handling

#### Integration Tests
- Connection establishment and pooling
- Message publishing and consumption
- Acknowledgment behavior
- Dead letter queue routing
- Reconnection on failure

#### Performance Tests
- Throughput (messages/second)
- Latency (publish to consume)
- Consumer scalability
- Memory usage under load

**Test Infrastructure:**
- Docker Compose for RabbitMQ test instance
- Test fixtures for common scenarios
- Mock consumers and publishers

**Estimated Time:** 3-4 days

---

## Configuration Schema

### RabbitMQ Configuration (config.yaml)

```yaml
message_queue:
  enabled: true
  type: "rabbitmq"  # or "redis" for future
  
  rabbitmq:
    # Connection
    host: "localhost"
    port: 5672
    username: "attune"
    password: "attune_secret"
    vhost: "/"
    
    # Connection pool
    pool_size: 10
    connection_timeout: 30s
    heartbeat: 60s
    
    # Reconnection
    reconnect_delay: 5s
    max_reconnect_attempts: 10
    
    # Publishing
    confirm_publish: true
    publish_timeout: 5s
    
    # Consuming
    prefetch_count: 10
    consumer_timeout: 300s
    
    # Queues
    queues:
      events:
        name: "attune.events.queue"
        durable: true
        exclusive: false
        auto_delete: false
        
      executions:
        name: "attune.executions.queue"
        durable: true
        exclusive: false
        auto_delete: false
        
      notifications:
        name: "attune.notifications.queue"
        durable: true
        exclusive: false
        auto_delete: false
    
    # Exchanges
    exchanges:
      events:
        name: "attune.events"
        type: "topic"
        durable: true
        
      executions:
        name: "attune.executions"
        type: "direct"
        durable: true
        
      notifications:
        name: "attune.notifications"
        type: "fanout"
        durable: true
    
    # Dead Letter Queues
    dead_letter:
      enabled: true
      exchange: "attune.dlx"
      ttl: 86400000  # 24 hours in ms
```

---

## Message Format Standard

### Envelope Structure

```json
{
  "message_id": "uuid-v4",
  "correlation_id": "uuid-v4",
  "message_type": "ExecutionRequested",
  "version": "1.0",
  "timestamp": "2024-01-13T10:30:00Z",
  "headers": {
    "retry_count": 0,
    "source_service": "api",
    "trace_id": "uuid-v4"
  },
  "payload": {
    // Message-specific data
  }
}
```

### Example Messages

#### EventCreated

```json
{
  "message_type": "EventCreated",
  "payload": {
    "event_id": 123,
    "trigger_id": 5,
    "sensor_id": 10,
    "trigger_ref": "aws.ec2.instance_state_change",
    "sensor_ref": "aws.ec2.monitor_instances",
    "data": {
      "instance_id": "i-1234567890abcdef0",
      "previous_state": "running",
      "current_state": "stopped"
    }
  }
}
```

#### ExecutionRequested

```json
{
  "message_type": "ExecutionRequested",
  "payload": {
    "execution_id": 456,
    "enforcement_id": 789,
    "action_id": 20,
    "action_ref": "slack.send_message",
    "parameters": {
      "channel": "#alerts",
      "message": "EC2 instance stopped"
    },
    "context": {
      "event_id": 123,
      "rule_id": 15
    }
  }
}
```

---

## Integration Points

### API Service (Publisher)
- Publishes `EventCreated` when sensor detects events
- Publishes `ExecutionRequested` when rule triggers
- Publishes `NotificationCreated` for system alerts

### Executor Service (Consumer + Publisher)
- Consumes `ExecutionRequested` from queue
- Publishes `ExecutionStatusChanged` during processing
- Publishes `ExecutionCompleted` when done
- Publishes `InquiryCreated` when human input needed

### Sensor Service (Consumer + Publisher)
- Consumes sensor configuration changes
- Publishes `EventCreated` when events detected

### Worker Service (Consumer + Publisher)
- Consumes execution tasks from Executor
- Publishes status updates back to Executor

### Notifier Service (Consumer)
- Consumes `NotificationCreated` messages
- Delivers notifications to users via WebSocket/SSE

---

## Deployment Considerations

### Development
- Docker Compose with RabbitMQ container
- Management UI enabled (port 15672)
- Default credentials for local dev

### Production
- RabbitMQ cluster (3+ nodes) for HA
- SSL/TLS for connections
- Authentication with proper credentials
- Monitoring with Prometheus exporter
- Persistent storage for messages
- Resource limits and quotas

---

## Success Criteria

- [ ] RabbitMQ connection management working
- [ ] All message types defined and tested
- [ ] Publisher can send messages to all exchanges
- [ ] Consumer can receive messages from all queues
- [ ] Dead letter queues working correctly
- [ ] Retry logic functioning as expected
- [ ] Integration tests passing (95%+ coverage)
- [ ] Performance tests show acceptable throughput
- [ ] Documentation complete with examples
- [ ] Configuration working across environments

---

## Timeline

| Phase | Task | Duration | Dependencies |
|-------|------|----------|--------------|
| 3.1 | Message Queue Setup | 2-3 days | None |
| 3.2 | Message Types | 2-3 days | 3.1 |
| 3.3 | Publisher | 2 days | 3.1, 3.2 |
| 3.4 | Consumer | 3 days | 3.1, 3.2 |
| 3.5 | DLQ & Error Handling | 2 days | 3.3, 3.4 |
| 3.6 | Testing | 3-4 days | All above |

**Total Estimated Time:** 2-3 weeks

---

## Next Steps After Phase 3

Once Phase 3 is complete, the foundation is ready for:

1. **Phase 4: Executor Service** - Consume execution requests, orchestrate workflows
2. **Phase 5: Worker Service** - Execute actions, publish results
3. **Phase 6: Sensor Service** - Detect events, publish to queue
4. **Phase 7: Notifier Service** - Consume notifications, push to clients

---

## Risk Assessment

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Connection instability | Medium | High | Implement reconnection logic |
| Message loss | Low | Critical | Use acknowledgments + persistence |
| Performance bottleneck | Low | Medium | Load testing, proper prefetch |
| Queue buildup | Medium | Medium | Monitoring, backpressure handling |

### Operational Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| RabbitMQ downtime | Low | High | Cluster setup, HA configuration |
| Disk space exhaustion | Medium | High | Message TTL, monitoring, alerts |
| Memory overflow | Low | Medium | Resource limits, monitoring |

---

## Resources & References

### Documentation
- [RabbitMQ Documentation](https://www.rabbitmq.com/documentation.html)
- [Lapin (RabbitMQ Rust Client)](https://github.com/amqp-rs/lapin)
- [AMQP 0-9-1 Protocol](https://www.rabbitmq.com/amqp-0-9-1-reference.html)

### Best Practices
- [RabbitMQ Best Practices](https://www.rabbitmq.com/best-practices.html)
- [Message Queue Patterns](https://www.enterpriseintegrationpatterns.com/patterns/messaging/)

---

## Appendix: Alternative Approaches

### Why Not Redis Pub/Sub?

**Pros:**
- Simpler setup
- Lower latency
- Already using Redis for caching (potentially)

**Cons:**
- No message persistence by default
- No acknowledgments
- Fire-and-forget delivery
- No dead letter queues
- Limited routing capabilities

**Conclusion:** RabbitMQ is better suited for reliable, persistent message queuing needed for automation workflows.

### Why Not Kafka?

**Pros:**
- High throughput
- Log-based storage
- Great for event streaming

**Cons:**
- Heavyweight for our use case
- More complex to operate
- Overkill for message volumes
- Higher resource requirements

**Conclusion:** RabbitMQ provides the right balance for Attune's needs.

---

**Status:** Ready to begin implementation! 🚀

**First Task:** Create MQ module structure and connection management (Phase 3.1)