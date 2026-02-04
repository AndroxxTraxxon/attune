# Sensor Service Setup Guide

## Prerequisites

Before running the Sensor Service, you need to:

1. **PostgreSQL Database** - Running instance with Attune schema
2. **RabbitMQ** - Message queue for inter-service communication
3. **SQLx Query Cache** - Prepared query metadata for compilation

## SQLx Query Cache Preparation

The Sensor Service uses SQLx compile-time query verification. This requires either:

### Option 1: Online Mode (Recommended for Development)

Set `DATABASE_URL` environment variable and SQLx will verify queries against the live database during compilation:

```bash
# Export database URL
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"

# Build the sensor service
cargo build --package attune-sensor
```

### Option 2: Offline Mode (Recommended for CI/CD)

Prepare the query cache once, then build without database:

```bash
# 1. Start your PostgreSQL database
docker-compose up -d postgres

# 2. Run migrations to create schema
cd migrations
sqlx migrate run --database-url postgresql://postgres:postgres@localhost:5432/attune

# 3. Set DATABASE_URL
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"

# 4. Prepare SQLx cache for the entire workspace
cargo sqlx prepare --workspace

# 5. Now you can build offline
SQLX_OFFLINE=true cargo build --package attune-sensor
```

The `cargo sqlx prepare` command creates a `.sqlx/` directory in the workspace root containing query metadata. This allows compilation without a database connection.

## Current Status

**As of 2024-01-17:**

The Sensor Service code is complete but requires SQLx cache preparation before it can compile. The queries are valid and tested in other services (API, Executor), but the sensor service is new and doesn't have cached metadata yet.

### Queries Used by Sensor Service

1. **event_generator.rs:**
   - `INSERT INTO attune.event` (2 variants)
   - `SELECT FROM attune.event WHERE id = $1`
   - `SELECT FROM attune.event WHERE trigger_ref = $1`

2. **rule_matcher.rs:**
   - `SELECT FROM attune.rule WHERE trigger_ref = $1`
   - `INSERT INTO attune.enforcement`

3. **sensor_manager.rs:**
   - `SELECT FROM attune.sensor WHERE enabled = true`
   - `SELECT FROM attune.trigger WHERE id = $1`

All queries follow the same patterns used successfully in the API and Executor services.

## Running the Sensor Service

Once SQLx cache is prepared:

```bash
# Development
cargo run --bin attune-sensor -- --config config.development.yaml

# Production
cargo run --release --bin attune-sensor -- --config config.production.yaml

# With custom log level
cargo run --bin attune-sensor -- --log-level debug
```

## Configuration

The Sensor Service requires these configuration sections:

```yaml
# config.yaml
database:
  url: postgresql://user:pass@localhost:5432/attune
  max_connections: 10

message_queue:
  enabled: true
  url: amqp://guest:guest@localhost:5672

# Optional sensor-specific settings (future)
sensor:
  enabled: true
  poll_interval: 30              # Default poll interval (seconds)
  max_concurrent_sensors: 100    # Max sensors running concurrently
  sensor_timeout: 300            # Sensor execution timeout (seconds)
  restart_on_error: true         # Restart sensors on error
  max_restart_attempts: 3        # Max restart attempts
```

## Troubleshooting

### Error: "set `DATABASE_URL` to use query macros online"

**Solution:** Export DATABASE_URL before building:
```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
cargo build --package attune-sensor
```

### Error: "SQLX_OFFLINE=true but there is no cached data"

**Solution:** Prepare the query cache first:
```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
cargo sqlx prepare --workspace
```

### Error: "failed to connect to database"

**Solution:** Ensure PostgreSQL is running and accessible:
```bash
# Test connection
psql postgresql://postgres:postgres@localhost:5432/attune -c "SELECT 1"

# Or use docker-compose
docker-compose up -d postgres
```

### Error: "relation 'attune.sensor' does not exist"

**Solution:** Run migrations to create the schema:
```bash
cd migrations
sqlx migrate run --database-url postgresql://postgres:postgres@localhost:5432/attune
```

## Testing

### Unit Tests

Unit tests don't require a database:

```bash
cargo test --package attune-sensor --lib
```

### Integration Tests

Integration tests require a running database:

```bash
# Start test database
docker-compose -f docker-compose.test.yaml up -d

# Run migrations
export DATABASE_URL="postgresql://postgres:postgres@localhost:5433/attune_test"
sqlx migrate run

# Run tests
cargo test --package attune-sensor
```

## Next Steps

1. **Prepare SQLx Cache** - Run `cargo sqlx prepare` with database running
2. **Implement Sensor Runtime Execution** - Integrate with Worker's runtime infrastructure
3. **Create Example Sensors** - Build sample sensors for testing
4. **End-to-End Testing** - Test full sensor → event → enforcement flow
5. **Configuration Updates** - Add sensor-specific settings to config.yaml

## See Also

- [Sensor Service Documentation](sensor-service.md) - Architecture and design
- [Sensor Service Implementation](../work-summary/sensor-service-implementation.md) - Implementation details
- [SQLx Documentation](https://github.com/launchbadge/sqlx) - SQLx query checking