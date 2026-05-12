# Attune Sensor Service

## Overview

The **Sensor Service** is responsible for monitoring trigger conditions and generating events in the Attune automation platform. It bridges the gap between external systems and the rule-based automation engine.

## Architecture

### Core Responsibilities

1. **Sensor Lifecycle Management**: Load, start, stop, and restart sensors
2. **Event Monitoring**: Execute sensors to detect trigger conditions
3. **Event Generation**: Create event records when triggers fire
4. **Rule Matching**: Find matching rules and create enforcements
5. **Event Publishing**: Publish events to the message queue for processing
6. **Operational Visibility**: Register sensor-worker health, expose rotating sensor logs, and honor sensor placement constraints

### Service Components

```
sensor/src/
├── main.rs                 # Service entry point
├── service.rs              # Main service orchestrator
├── sensor_manager.rs       # Manage sensor instances
├── event_generator.rs      # Generate events from sensor data
├── rule_matcher.rs         # Match events to rules
├── monitors/               # Different trigger monitor types
│   ├── mod.rs
│   ├── custom.rs           # Execute custom sensor code
│   ├── timer.rs            # Cron/interval triggers
│   ├── webhook.rs          # HTTP webhook triggers
│   └── file.rs             # File watch triggers
└── runtime/                # Sensor runtime execution
    ├── mod.rs
    └── sensor_executor.rs  # Execute sensor code in runtime
```

## Event Flow

```
Sensor Poll → Condition Met → Generate Event → Match Rules → Create Enforcements
     ↓              ↓              ↓               ↓              ↓
  Database    Sensor Code    attune.event    Rule Query    attune.enforcement
                                 ↓                              ↓
                          EventCreated Msg            EnforcementCreated Msg
                                 ↓                              ↓
                           (to Notifier)                 (to Executor)
```

## Database Schema

### Trigger Table

```sql
CREATE TABLE attune.trigger (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,           -- Format: pack.name
    pack BIGINT REFERENCES attune.pack(id),
    pack_ref TEXT,
    label TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    param_schema JSONB,                 -- Configuration schema
    out_schema JSONB,                   -- Output payload schema
    created TIMESTAMPTZ NOT NULL,
    updated TIMESTAMPTZ NOT NULL
);
```

### Sensor Table

```sql
CREATE TABLE attune.sensor (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,           -- Format: pack.name
    pack BIGINT REFERENCES attune.pack(id),
    pack_ref TEXT,
    label TEXT NOT NULL,
    description TEXT NOT NULL,
    entrypoint TEXT NOT NULL,           -- Code entry point
    runtime BIGINT NOT NULL REFERENCES attune.runtime(id),
    runtime_ref TEXT NOT NULL,          -- e.g., "core.sensor.python3"
    trigger BIGINT NOT NULL REFERENCES attune.trigger(id),
    trigger_ref TEXT NOT NULL,          -- e.g., "core.webhook"
    enabled BOOLEAN NOT NULL,
    param_schema JSONB,                 -- Sensor configuration schema
    created TIMESTAMPTZ NOT NULL,
    updated TIMESTAMPTZ NOT NULL
);
```

### Event Table

```sql
CREATE TABLE attune.event (
    id BIGSERIAL PRIMARY KEY,
    trigger BIGINT REFERENCES attune.trigger(id),
    trigger_ref TEXT NOT NULL,          -- Preserved even if trigger deleted
    config JSONB,                       -- Snapshot of trigger/sensor config
    payload JSONB,                      -- Event data
    source BIGINT REFERENCES attune.sensor(id),
    source_ref TEXT,                    -- Sensor that generated event
    created TIMESTAMPTZ NOT NULL,
    updated TIMESTAMPTZ NOT NULL
);
```

## Sensor Types

### 1. Custom Sensors

Custom sensors execute user-defined code that polls for conditions:

```python
# Example: GitHub webhook sensor
def poll():
    # Check for new GitHub events
    events = check_github_api()
    
    for event in events:
        # Return event payload
        yield {
            "event_type": event.type,
            "repository": event.repo,
            "author": event.author,
            "data": event.data
        }
```

**Features**:
- Support multiple runtimes (Python, Node.js)
- Poll on configurable intervals
- Handle failures and retries
- Restart on errors

### 2. Timer Triggers (Built-in)

Execute actions on a schedule:

```yaml
trigger:
  ref: core.timer
  type: cron
  schedule: "0 0 * * *"  # Daily at midnight
```

**Features**:
- Cron expressions
- Interval-based (every N seconds/minutes/hours)
- Timezone support

### 3. Webhook Triggers (Built-in)

HTTP endpoints for external systems:

```yaml
trigger:
  ref: core.webhook
  path: /webhook/github
  method: POST
  auth: bearer_token
```

**Features**:
- Dynamic webhook URL generation
- Authentication (API key, bearer token, HMAC)
- Payload validation
- Path parameters

### 4. File Watch Triggers (Future)

Monitor filesystem changes:

```yaml
trigger:
  ref: core.file_watch
  path: /var/log/app.log
  patterns: ["ERROR", "FATAL"]
```

## Configuration

### Service Configuration

```yaml
sensor:
  enabled: true
  poll_interval: 30              # Default poll interval (seconds)
  max_concurrent_sensors: 100    # Max sensors running concurrently
  sensor_timeout: 300            # Sensor execution timeout (seconds)
  restart_on_error: true         # Restart sensors on error
  max_restart_attempts: 3        # Max restart attempts before disabling
  labels: {}                     # Sensor-worker labels used by sensor placement
  taints: []                     # Sensor-worker taints used by sensor placement
  
  # Webhook server (if enabled)
  webhook:
    enabled: false
    host: 0.0.0.0
    port: 8083
    base_path: /webhooks
    
  # Timer triggers (if enabled)
  timer:
    enabled: false
    tick_interval: 1             # Check timers every N seconds
```

### Environment Variables

```bash
# Override service settings
ATTUNE__SENSOR__ENABLED=true
ATTUNE__SENSOR__POLL_INTERVAL=30
ATTUNE__SENSOR__MAX_CONCURRENT_SENSORS=100
ATTUNE__SENSOR__WEBHOOK__ENABLED=true
ATTUNE__SENSOR__WEBHOOK__PORT=8083
```

## Message Queue Integration

### Consumes From

No messages consumed initially (standalone operation).

### Publishes To

#### EventCreated Message

Published to `attune.events` exchange with routing key `event.created`:

```json
{
  "message_id": "uuid",
  "correlation_id": "uuid",
  "message_type": "EventCreated",
  "timestamp": "2024-01-15T10:30:00Z",
  "payload": {
    "event_id": 123,
    "trigger_ref": "github.webhook",
    "trigger_id": 45,
    "sensor_ref": "github.listener",
    "sensor_id": 67,
    "payload": {
      "event_type": "push",
      "repository": "user/repo",
      "author": "johndoe"
    },
    "config": {
      "repo_url": "https://github.com/user/repo"
    }
  }
}
```

#### EnforcementCreated Message

Published to `attune.events` exchange with routing key `enforcement.created`:

```json
{
  "message_id": "uuid",
  "correlation_id": "uuid",
  "message_type": "EnforcementCreated",
  "timestamp": "2024-01-15T10:30:00Z",
  "payload": {
    "enforcement_id": 456,
    "rule_id": 78,
    "rule_ref": "github.deploy_on_push",
    "event_id": 123,
    "trigger_ref": "github.webhook",
    "payload": {
      "event_type": "push",
      "repository": "user/repo",
      "branch": "main"
    }
  }
}
```

## Sensor Execution

### Sensor Manager

The `SensorManager` component:

1. **Loads Sensors**: Query database for enabled sensors
2. **Starts Sensors**: Spawn async tasks for each sensor whose placement matches this sensor worker
3. **Monitors Health**: Track sensor status and restarts
4. **Handles Errors**: Retry logic and failure tracking

```rust
pub struct SensorManager {
    sensors: Arc<RwLock<HashMap<i64, SensorInstance>>>,
    config: Arc<SensorConfig>,
    db: PgPool,
}

impl SensorManager {
    pub async fn start(&self) -> Result<()> {
        let sensors = self.load_enabled_sensors().await?;
        
        for sensor in sensors {
            self.start_sensor(sensor).await?;
        }
        
        Ok(())
    }
    
    async fn start_sensor(&self, sensor: Sensor) -> Result<()> {
        let instance = SensorInstance::new(sensor, self.config.clone());
        instance.start().await?;
        
        self.sensors.write().await.insert(sensor.id, instance);
        
        Ok(())
    }
}
```

### Sensor Instance

Each sensor runs in its own async task:

```rust
pub struct SensorInstance {
    sensor: Sensor,
    runtime: RuntimeConfig,
    poll_interval: Duration,
    status: Arc<RwLock<SensorStatus>>,
}

impl SensorInstance {
    pub async fn start(&self) -> Result<()> {
        tokio::spawn(self.run_loop());
        Ok(())
    }
    
    async fn run_loop(&self) {
        loop {
            match self.poll().await {
                Ok(events) => {
                    for event_data in events {
                        self.generate_event(event_data).await;
                    }
                }
                Err(e) => {
                    self.handle_error(e).await;
                }
            }
            
            tokio::time::sleep(self.poll_interval).await;
        }
    }
    
    async fn poll(&self) -> Result<Vec<JsonValue>> {
        // Execute sensor code in runtime
        // Similar to Worker's ActionExecutor
        todo!()
    }
}
```

### Sensor placement and logs

Pack sensors can declare `worker_selector`, `worker_tolerations`, and `worker_affinity`, using the same placement vocabulary as actions. Sensor workers register configured `sensor.labels` and `sensor.taints` in `worker.capabilities`; `SensorManager` evaluates those capabilities before starting or restarting a sensor process.

Sensor stdout and stderr are written to rotating files under `{artifacts_dir}/sensors/{sensor_ref}/`. The API exposes stream metadata and tail reads through `GET /api/v1/sensors/{sensor_ref}/logs` and `GET /api/v1/sensors/{sensor_ref}/logs/{stream}?tail=N`, and the sensor detail page can follow stdout/stderr by polling the tail endpoint.

Managed sensor process live state is persisted in `sensor_process` and changes are mirrored to `sensor_process_history`. The manager records process starts/stops, detects unexpected child exits with non-blocking `try_wait`, captures stderr excerpts, marks failed processes as `backoff`, restarts them with capped exponential backoff while active rules still depend on the sensor, and emits `core.alert` after repeated failures.

### Runtime Execution

Sensors execute in runtimes (Python, Node.js) similar to actions:

```rust
pub struct SensorExecutor {
    runtime_manager: RuntimeManager,
}

impl SensorExecutor {
    pub async fn execute(&self, sensor: &Sensor, config: JsonValue) -> Result<Vec<JsonValue>> {
        // 1. Prepare execution environment
        // 2. Inject sensor code and configuration
        // 3. Execute sensor
        // 4. Collect yielded events
        // 5. Return event data
        
        let runtime = self.runtime_manager.get_runtime(&sensor.runtime_ref)?;
        let result = runtime.execute_sensor(sensor, config).await?;
        
        Ok(result)
    }
}
```

## Event Generation

When a sensor detects a trigger condition:

1. **Create Event Record**: Insert into `attune.event` table
2. **Snapshot Configuration**: Capture trigger/sensor config at event time
3. **Store Payload**: Save event data from sensor
4. **Publish Message**: Send `EventCreated` to message queue

```rust
pub struct EventGenerator {
    db: PgPool,
    mq: MessageQueue,
}

impl EventGenerator {
    pub async fn generate_event(
        &self,
        sensor: &Sensor,
        trigger: &Trigger,
        payload: JsonValue,
    ) -> Result<i64> {
        // Create event record
        let event_id = sqlx::query_scalar!(
            r#"
            INSERT INTO attune.event 
                (trigger, trigger_ref, config, payload, source, source_ref)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            Some(trigger.id),
            &trigger.r#ref,
            self.build_config_snapshot(trigger, sensor),
            &payload,
            Some(sensor.id),
            Some(&sensor.r#ref)
        )
        .fetch_one(&self.db)
        .await?;
        
        // Publish EventCreated message
        self.publish_event_created(event_id, trigger, sensor, &payload).await?;
        
        Ok(event_id)
    }
}
```

## Rule Matching

After generating an event, find matching rules:

1. **Query Rules**: Find rules for the trigger
2. **Evaluate Conditions**: Check if event matches rule conditions
3. **Create Enforcements**: Insert enforcement records
4. **Publish Messages**: Send `EnforcementCreated` to executor

```rust
pub struct RuleMatcher {
    db: PgPool,
    mq: MessageQueue,
}

impl RuleMatcher {
    pub async fn match_rules(&self, event: &Event) -> Result<Vec<i64>> {
        // Find enabled rules for this trigger
        let rules = sqlx::query_as!(
            Rule,
            r#"
            SELECT * FROM attune.rule
            WHERE trigger_ref = $1 AND enabled = true
            "#,
            &event.trigger_ref
        )
        .fetch_all(&self.db)
        .await?;
        
        let mut enforcement_ids = Vec::new();
        
        for rule in rules {
            // Evaluate rule conditions
            if self.evaluate_conditions(&rule, event).await? {
                let enforcement_id = self.create_enforcement(&rule, event).await?;
                enforcement_ids.push(enforcement_id);
            }
        }
        
        Ok(enforcement_ids)
    }
    
    async fn evaluate_conditions(&self, rule: &Rule, event: &Event) -> Result<bool> {
        // Evaluate JSON conditions against event payload
        // Simple implementation: check if all conditions match
        todo!()
    }
    
    async fn create_enforcement(&self, rule: &Rule, event: &Event) -> Result<i64> {
        let enforcement_id = sqlx::query_scalar!(
            r#"
            INSERT INTO attune.enforcement 
                (rule, rule_ref, trigger_ref, event, status, payload, condition, conditions)
            VALUES ($1, $2, $3, $4, 'created', $5, $6, $7)
            RETURNING id
            "#,
            Some(rule.id),
            &rule.r#ref,
            &rule.trigger_ref,
            Some(event.id),
            event.payload.clone().unwrap_or_default(),
            rule.condition,
            &rule.conditions
        )
        .fetch_one(&self.db)
        .await?;
        
        // Publish EnforcementCreated message
        self.publish_enforcement_created(enforcement_id, rule, event).await?;
        
        Ok(enforcement_id)
    }
}
```

## Condition Evaluation

Rule conditions are evaluated against event payloads:

### Condition Format

```json
{
  "conditions": [
    {
      "field": "payload.branch",
      "operator": "equals",
      "value": "main"
    },
    {
      "field": "payload.author",
      "operator": "not_equals",
      "value": "bot"
    }
  ],
  "condition": "all"  // or "any"
}
```

### Supported Operators

- `equals`: Exact match
- `not_equals`: Not equal
- `contains`: String contains
- `starts_with`: String prefix
- `ends_with`: String suffix
- `matches`: Regex match
- `greater_than`: Numeric comparison
- `less_than`: Numeric comparison
- `in`: Value in array
- `not_in`: Value not in array

## Error Handling

### Sensor Failures

When a sensor fails:

1. Log error with context
2. Increment failure count
3. Restart sensor (if configured)
4. Disable sensor after max retries
5. Create notification

```rust
async fn handle_sensor_error(&self, sensor_id: i64, error: Error) {
    error!("Sensor {} failed: {}", sensor_id, error);
    
    let failure_count = self.increment_failure_count(sensor_id).await;
    
    if failure_count >= self.config.max_restart_attempts {
        warn!("Sensor {} exceeded max restart attempts, disabling", sensor_id);
        self.disable_sensor(sensor_id).await;
    } else {
        info!("Restarting sensor {} (attempt {})", sensor_id, failure_count);
        self.restart_sensor(sensor_id).await;
    }
}
```

### Event Generation Failures

If event generation fails:

1. Log error
2. Retry with backoff
3. Create alert notification
4. Continue sensor operation

## Monitoring

### Metrics

- `sensors_active`: Number of active sensors
- `sensors_failed`: Number of failed sensors
- `events_generated_total`: Total events generated
- `events_generated_rate`: Events per second
- `enforcements_created_total`: Total enforcements created
- `sensor_poll_duration`: Time to poll sensor
- `event_generation_duration`: Time to generate event
- `rule_matching_duration`: Time to match rules

### Health Checks

```rust
pub struct HealthCheck {
    sensor_manager: Arc<SensorManager>,
}

impl HealthCheck {
    pub async fn check(&self) -> HealthStatus {
        let active_sensors = self.sensor_manager.active_count().await;
        let failed_sensors = self.sensor_manager.failed_count().await;
        
        if active_sensors == 0 {
            HealthStatus::Unhealthy("No active sensors".to_string())
        } else if failed_sensors > 10 {
            HealthStatus::Degraded(format!("{} sensors failed", failed_sensors))
        } else {
            HealthStatus::Healthy
        }
    }
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_event_generation() {
        let generator = EventGenerator::new(db_pool(), mq_client());
        
        let event_id = generator.generate_event(
            &test_sensor(),
            &test_trigger(),
            json!({"test": "data"}),
        ).await.unwrap();
        
        assert!(event_id > 0);
    }
    
    #[tokio::test]
    async fn test_rule_matching() {
        let matcher = RuleMatcher::new(db_pool(), mq_client());
        
        let event = test_event_with_payload(json!({
            "branch": "main",
            "author": "alice"
        }));
        
        let enforcements = matcher.match_rules(&event).await.unwrap();
        assert_eq!(enforcements.len(), 1);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_sensor_to_enforcement_flow() {
    // 1. Create sensor
    let sensor = create_test_sensor().await;
    
    // 2. Create trigger
    let trigger = create_test_trigger().await;
    
    // 3. Create rule
    let rule = create_test_rule(trigger.id, action.id).await;
    
    // 4. Start sensor
    sensor_manager.start_sensor(sensor).await;
    
    // 5. Wait for event
    let event = wait_for_event(trigger.id).await;
    
    // 6. Verify enforcement created
    let enforcement = wait_for_enforcement(rule.id).await;
    
    assert_eq!(enforcement.event, Some(event.id));
}
```

## Deployment

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin attune-sensor

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    python3 \
    nodejs \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/attune-sensor /usr/local/bin/
CMD ["attune-sensor"]
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: attune-sensor
spec:
  replicas: 2
  selector:
    matchLabels:
      app: attune-sensor
  template:
    metadata:
      labels:
        app: attune-sensor
    spec:
      containers:
      - name: sensor
        image: attune/sensor:latest
        env:
        - name: ATTUNE__DATABASE__URL
          valueFrom:
            secretKeyRef:
              name: attune-db
              key: url
        - name: ATTUNE__MESSAGE_QUEUE__URL
          valueFrom:
            secretKeyRef:
              name: attune-mq
              key: url
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
```

## Security Considerations

1. **Sensor Code Isolation**: Execute sensor code in sandboxed environments
2. **Secret Management**: Use secrets for sensor authentication (API keys, tokens)
3. **Rate Limiting**: Limit sensor poll frequency to prevent abuse
4. **Input Validation**: Validate event payloads before storage
5. **Access Control**: Restrict sensor management to authorized users
6. **Audit Logging**: Log all sensor operations and events

## Future Enhancements

1. **Distributed Sensors**: Run sensors across multiple nodes
2. **Sensor Clustering**: Group related sensors for coordination
3. **Event Deduplication**: Prevent duplicate events
4. **Event Filtering**: Pre-filter events before rule matching
5. **Sensor Hot Reload**: Update sensor code without restart
6. **Advanced Scheduling**: Complex polling schedules
7. **Webhook Security**: HMAC validation, IP whitelisting
8. **Sensor Metrics Dashboard**: Real-time sensor monitoring UI
