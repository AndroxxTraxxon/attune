# Running Tests - Quick Reference

This guide provides quick commands for running all tests across the Attune project.

**Note:** Attune uses a **schema-per-test architecture** for true test isolation and parallel execution. See [Schema-Per-Test Architecture](./schema-per-test.md) for details.

---

## Quick Start

```bash
# Run all tests (from project root)
make test

# Or run individually by component
make test-common
make test-api
make test-executor
make test-worker
make test-sensor
make test-cli
make test-core-pack
```

---

## By Component

### 1. Common Library

```bash
cd crates/common
cargo test
```

**Coverage**: 539 tests
- Repository tests (all 15 repositories)
- Model validation
- Configuration parsing
- Error handling

**Note**: Tests run in parallel with isolated schemas (no `#[serial]` constraints)

---

### 2. API Service

```bash
cd crates/api
cargo test
```

**Coverage**: 82 tests
- Unit tests (41)
- Integration tests (41)
- Authentication flows
- CRUD operations

**Performance**: ~4-5 seconds (parallel execution with schema isolation)

---

### 3. Executor Service

```bash
cd crates/executor
cargo test
```

**Coverage**: 63 tests
- Unit tests (55)
- Integration tests (8)
- Queue management
- Workflow orchestration

---

### 4. Worker Service

```bash
cd crates/worker
cargo test
```

**Coverage**: 50 tests
- Unit tests (44)
- Security tests (6)
- Action execution
- Dependency isolation

---

### 5. Sensor Service

```bash
cd crates/sensor
cargo test
```

**Coverage**: 27 tests
- Timer sensors
- Interval timers
- Cron timers
- Event generation

---

### 6. CLI Tool

```bash
cd crates/cli
cargo test
```

**Coverage**: 60+ integration tests
- Pack management
- Action execution
- Configuration
- User workflows

---

### 7. Core Pack

```bash
# Bash test runner (fast)
cd packs/core/tests
./run_tests.sh

# Python test suite (comprehensive)
cd packs/core/tests
python3 test_actions.py

# With pytest (recommended)
cd packs/core/tests
pytest test_actions.py -v
```

**Coverage**: 76 tests
- core.echo (7 tests)
- core.noop (8 tests)
- core.sleep (8 tests)
- core.http_request (10 tests)
- File permissions (4 tests)
- YAML validation (optional)

---

## Running Specific Tests

### Rust Tests

```bash
# Run specific test by name
cargo test test_name

# Run tests matching pattern
cargo test pattern

# Run tests in specific module
cargo test module_name::

# Show test output
cargo test -- --nocapture

# Run tests serially (not parallel) - rarely needed with schema-per-test
cargo test -- --test-threads=1

# See verbose output from specific test
cargo test test_name -- --nocapture
```

### Python Tests (Core Pack)

```bash
# Run specific test class
pytest test_actions.py::TestEchoAction -v

# Run specific test method
pytest test_actions.py::TestEchoAction::test_basic_echo -v

# Show output
pytest test_actions.py -v -s
```

---

## Test Requirements

### Rust Tests

**Required**:
- Rust 1.70+
- PostgreSQL (for integration tests)
- RabbitMQ (for integration tests)

**Setup**:
```bash
# Start test dependencies (PostgreSQL)
docker-compose -f docker-compose.test.yaml up -d

# Create test database
createdb -U postgres attune_test

# Run tests (migrations run automatically per test)
cargo test

# Cleanup orphaned test schemas (optional)
./scripts/cleanup-test-schemas.sh
```

**Note**: Each test creates its own isolated schema (`test_<uuid>`), runs migrations, and cleans up automatically.

### Core Pack Tests

**Required**:
- bash
- python3

**Optional**:
- `pytest` - Better test output: `pip install pytest`
- `PyYAML` - YAML validation: `pip install pyyaml`
- `requests` - HTTP tests: `pip install requests>=2.28.0`

---

## Continuous Integration

### GitHub Actions

Tests run automatically on:
- Push to main
- Pull requests
- Manual workflow dispatch

View results: `.github/workflows/test.yml`

---

## Test Coverage

### Current Coverage by Component

| Component | Tests | Status | Coverage |
|-----------|-------|--------|----------|
| Common | 539 | ✅ Passing | ~90% |
| API | 82 | ✅ Passing | ~70% |
| Executor | 63 | ✅ Passing | ~85% |
| Worker | 50 | ✅ Passing | ~80% |
| Sensor | 27 | ✅ Passing | ~75% |
| CLI | 60+ | ✅ Passing | ~70% |
| Core Pack | 76 | ✅ Passing | 100% |
| **Total** | **732+** | **✅ 731+ Passing** | **~40%** |

---

## Troubleshooting

### Tests Fail Due to Database

```bash
# Ensure PostgreSQL is running
docker ps | grep postgres

# Check connection
psql -U postgres -h localhost -c "SELECT 1"

# Cleanup orphaned test schemas
./scripts/cleanup-test-schemas.sh --force

# Check for accumulated schemas
psql postgresql://postgres:postgres@localhost:5432/attune_test -c \
  "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'test_%';"

# If needed, recreate test database
dropdb attune_test
createdb attune_test
```

**Tip**: The schema-per-test approach means you don't need to reset the database between test runs. Each test gets its own isolated schema.

### Tests Fail Due to RabbitMQ

```bash
# Ensure RabbitMQ is running
docker ps | grep rabbitmq

# Check status
rabbitmqctl status

# Reset queues
rabbitmqadmin purge queue name=executor.enforcement
```

### Core Pack Tests Fail

```bash
# Check file permissions
ls -la packs/core/actions/

# Make scripts executable
chmod +x packs/core/actions/*.sh
chmod +x packs/core/actions/*.py

# Install Python dependencies
pip install requests>=2.28.0
```

### Slow Tests

```bash
# Run only fast unit tests (skip integration)
cargo test --lib

# Run specific test suite
cargo test --test integration_test

# Parallel execution (default, recommended with schema-per-test)
cargo test

# Limit parallelism if needed
cargo test -- --test-threads=4

# Serial execution (rarely needed with schema isolation)
cargo test -- --test-threads=1

# Cleanup accumulated schemas if performance degrades
./scripts/cleanup-test-schemas.sh --force
```

**Note**: With schema-per-test isolation, parallel execution is safe and ~4-8x faster than serial execution.

---

## Best Practices

### Before Committing

```bash
# 1. Run all tests
cargo test --all

# 2. Run core pack tests
cd packs/core/tests && ./run_tests.sh

# 3. Check formatting
cargo fmt --check

# 4. Run clippy
cargo clippy -- -D warnings
```

### Writing New Tests

1. **Unit tests**: In the same file as the code
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_something() {
           // Test code
       }
   }
   ```

2. **Integration tests**: In `tests/` directory
   ```rust
   // tests/integration_test.rs
   use crate::helpers::TestContext;
   
   #[tokio::test]
   async fn test_integration() {
       // Each test gets isolated schema automatically
       let ctx = TestContext::new().await;
       
       // Test code using ctx.pool, ctx.app, etc.
   }
   ```
   
   **Important**: No need for `#[serial]` attribute - schema-per-test provides isolation!

3. **Core Pack tests**: Add to both test runners
   - `packs/core/tests/run_tests.sh` for quick tests
   - `packs/core/tests/test_actions.py` for comprehensive tests

---

## Performance Benchmarks

Expected test execution times:

| Component | Time | Notes |
|-----------|------|-------|
| Common | ~0.5s | Parallel execution |
| API | ~4-5s | **75% faster** with schema-per-test |
| Executor | ~6s | Parallel with isolation |
| Worker | ~5s | Parallel execution |
| Sensor | ~3s | Parallel timer tests |
| CLI | ~12s | Integration tests |
| Core Pack (bash) | ~20s | Includes HTTP tests |
| Core Pack (python) | ~12s | Unittest suite |
| **Total** | **~60s** | **4-8x speedup** with parallel execution |

**Performance Improvement**: Schema-per-test architecture enables true parallel execution without `#[serial]` constraints, resulting in 75% faster test runs for integration tests.

---

## Resources

- [Schema-Per-Test Architecture](./schema-per-test.md) - **NEW**: Detailed explanation of test isolation
- [Testing Status](testing-status.md) - Detailed coverage analysis
- [Core Pack Tests](../packs/core/tests/README.md) - Core pack testing guide
- [Production Deployment](./production-deployment.md) - Production schema configuration
- [Contributing](../CONTRIBUTING.md) - Development guidelines

## Maintenance

### Cleanup Orphaned Test Schemas

If tests are interrupted (Ctrl+C, crash), schemas may accumulate:

```bash
# Manual cleanup
./scripts/cleanup-test-schemas.sh

# Force cleanup (no confirmation)
./scripts/cleanup-test-schemas.sh --force

# Check schema count
psql postgresql://postgres:postgres@localhost:5432/attune_test -c \
  "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'test_%';"
```

Run cleanup periodically or if you notice performance degradation.

---

**Last Updated**: 2026-01-28  
**Maintainer**: Attune Team