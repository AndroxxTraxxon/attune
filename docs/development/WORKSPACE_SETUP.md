# Attune Rust Workspace Setup Summary

This document summarizes the Cargo workspace setup for the Attune automation platform.

## ✅ What Has Been Created

### 1. Workspace Structure

A complete Cargo workspace with the following structure:

```
attune/
├── Cargo.toml                    # Workspace root configuration
├── README.md                     # Project documentation
├── .gitignore                    # Git ignore rules
├── .env.example                  # Environment configuration template
├── WORKSPACE_SETUP.md           # This file
├── reference/
│   ├── models.py                # Python SQLAlchemy models (reference)
│   └── models.md                # Comprehensive model documentation
└── crates/
    ├── common/                  # Shared library
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs           # Library entry point
    │       ├── config.rs        # Configuration management
    │       ├── db.rs            # Database connection pooling
    │       ├── error.rs         # Unified error types
    │       ├── models.rs        # Data models (SQLx)
    │       ├── schema.rs        # Schema utilities and validation
    │       └── utils.rs         # Common utilities
    ├── api/                     # REST API Service
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── executor/                # Execution Management Service
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── worker/                  # Action Execution Service
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── sensor/                  # Event Monitoring Service
    │   ├── Cargo.toml
    │   └── src/main.rs
    └── notifier/                # Notification Service
        ├── Cargo.toml
        └── src/main.rs
```

### 2. Common Library (`attune-common`)

The shared library provides:

- **Configuration Management**: Full-featured config system supporting env vars and config files
- **Database Layer**: SQLx-based connection pooling with health checks and migrations support
- **Error Handling**: Comprehensive error types with helper methods
- **Data Models**: Complete SQLx models matching the Python reference models
- **Schema Utilities**: Validation for refs, JSON schemas, and database operations
- **Common Utilities**: Pagination, time formatting, string sanitization, etc.

### 3. Service Crates

Five specialized services, each with:
- Individual `Cargo.toml` with appropriate dependencies
- Stub `main.rs` with CLI argument parsing and configuration loading
- Ready for implementation of service-specific logic

#### Services Overview:

1. **attune-api**: REST API gateway for all client interactions
2. **attune-executor**: Manages action execution lifecycle and scheduling
3. **attune-worker**: Executes actions in various runtime environments
4. **attune-sensor**: Monitors for trigger conditions and generates events
5. **attune-notifier**: Handles real-time notifications and pub/sub

### 4. Dependencies

All services share a common set of workspace dependencies:

- **Async Runtime**: tokio (full-featured async runtime)
- **Web Framework**: axum + tower (for API service)
- **Database**: sqlx (async PostgreSQL with compile-time checked queries)
- **Serialization**: serde + serde_json
- **Logging**: tracing + tracing-subscriber
- **Message Queue**: lapin (RabbitMQ client)
- **Cache**: redis (optional, for caching)
- **Error Handling**: anyhow + thiserror
- **Configuration**: config crate with environment variable support
- **Validation**: validator + jsonschema
- **Encryption**: argon2 + ring
- **CLI**: clap (command-line argument parsing)

## 🚀 Quick Start

### Prerequisites

Install the required services:

```bash
# PostgreSQL
brew install postgresql@14  # macOS
# or
sudo apt install postgresql-14  # Ubuntu

# RabbitMQ
brew install rabbitmq  # macOS
# or
sudo apt install rabbitmq-server  # Ubuntu

# Redis (optional)
brew install redis  # macOS
# or
sudo apt install redis-server  # Ubuntu
```

### Setup Steps

1. **Copy environment configuration:**
   ```bash
   cp .env.example .env
   ```

2. **Edit `.env` and update:**
   - Database connection URL
   - JWT secret (generate a secure random string)
   - Encryption key (at least 32 characters)

3. **Create database:**
   ```bash
   createdb attune
   ```

4. **Build the workspace:**
   ```bash
   cargo build
   ```

5. **Run tests:**
   ```bash
   cargo test
   ```

6. **Start a service:**
   ```bash
   cargo run --bin attune-api
   ```

## 📝 Configuration

Configuration uses a hierarchical approach:

1. **Default values** (defined in `config.rs`)
2. **Configuration file** (if `ATTUNE_CONFIG` env var is set)
3. **Environment variables** (prefix: `ATTUNE__`, separator: `__`)

Example environment variable:
```bash
ATTUNE__DATABASE__URL=postgresql://localhost/attune
```

Maps to:
```rust
config.database.url
```

## 🏗️ Development Workflow

### Building

```bash
# Build all services
cargo build

# Build in release mode
cargo build --release

# Build specific service
cargo build -p attune-api

# Check without building
cargo check
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p attune-common

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run clippy with all features
cargo clippy --all-features -- -D warnings
```

## 📚 Key Files to Implement Next

### 1. Database Migrations

Create `migrations/` directory with SQLx migrations:

```bash
# Create migration
sqlx migrate add initial_schema

# Run migrations
sqlx migrate run
```

### 2. API Routes

In `crates/api/src/`:
- `routes/mod.rs` - Route definitions
- `handlers/mod.rs` - Request handlers
- `middleware/` - Authentication, logging, etc.

### 3. Service Logic

Each service needs:
- Message queue consumers/producers
- Business logic implementation
- Integration with database
- Error handling

### 4. Tests

Each crate should have:
- Unit tests in `tests/` directory
- Integration tests
- Mock implementations for testing

## 🔧 Common Tasks

### Adding a New Dependency

1. Add to workspace dependencies in root `Cargo.toml`:
   ```toml
   [workspace.dependencies]
   new-crate = "1.0"
   ```

2. Use in service `Cargo.toml`:
   ```toml
   [dependencies]
   new-crate = { workspace = true }
   ```

### Creating a New Service

1. Create directory: `crates/new-service/`
2. Add to workspace members in root `Cargo.toml`
3. Create `Cargo.toml` and `src/main.rs`
4. Add dependencies from workspace

### Database Queries

Using SQLx with compile-time checking:

```rust
// Query single row
let pack = sqlx::query_as!(
    Pack,
    r#"SELECT * FROM attune.pack WHERE ref = $1"#,
    pack_ref
)
.fetch_one(&pool)
.await?;

// Query multiple rows
let packs = sqlx::query_as!(
    Pack,
    r#"SELECT * FROM attune.pack ORDER BY created DESC"#
)
.fetch_all(&pool)
.await?;
```

## 🎯 Next Steps

1. **Implement Database Migrations**
   - Create migration files for all tables
   - Add indexes and constraints
   - Set up database triggers and functions

2. **Implement API Service**
   - CRUD endpoints for all models
   - Authentication middleware
   - OpenAPI/Swagger documentation
   - WebSocket support for notifications

3. **Implement Executor Service**
   - Execution queue management
   - Status tracking
   - Policy enforcement
   - Workflow orchestration

4. **Implement Worker Service**
   - Runtime environment setup
   - Action execution
   - Result reporting
   - Heartbeat mechanism

5. **Implement Sensor Service**
   - Trigger monitoring
   - Event generation
   - Sensor lifecycle management

6. **Implement Notifier Service**
   - PostgreSQL LISTEN/NOTIFY integration
   - WebSocket server
   - Notification routing

7. **Add Tests**
   - Unit tests for all modules
   - Integration tests for services
   - End-to-end workflow tests

8. **Documentation**
   - API documentation
   - Service architecture docs
   - Deployment guides
   - Example packs

## 📖 References

- **Models Documentation**: `reference/models.md` - Comprehensive documentation of all data models
- **Python Models**: `reference/models.py` - Reference SQLAlchemy implementation
- **README**: `README.md` - Full project documentation
- **Config Example**: `.env.example` - Configuration template with all options

## 🐛 Troubleshooting

### Compilation Errors

```bash
# Clean and rebuild
cargo clean
cargo build

# Update dependencies
cargo update
```

### Database Connection Issues

1. Check PostgreSQL is running
2. Verify connection URL in `.env`
3. Ensure database exists
4. Check firewall/network settings

### Missing Dependencies

```bash
# Install system dependencies (Ubuntu)
sudo apt install pkg-config libssl-dev

# Install system dependencies (macOS)
brew install openssl pkg-config
```

## 💡 Tips

- Use `cargo watch` for automatic rebuilds during development
- Run `cargo clippy` before committing to catch common issues
- Use `RUST_LOG=debug` for detailed logging
- Set `RUST_BACKTRACE=1` for better error messages
- Use `cargo-expand` to see macro expansions
- Use `cargo-tree` to view dependency tree

## ✨ Status

**Current Status**: ✅ Workspace Setup Complete

All foundational code is in place. The workspace compiles successfully and is ready for service implementation.

**Next Milestone**: Implement database migrations and basic API endpoints.