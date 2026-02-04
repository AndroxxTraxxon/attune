# Schema-Per-Test Architecture

**Status:** Implemented  
**Version:** 1.0  
**Last Updated:** 2026-01-28  

## Overview

Attune uses a **schema-per-test architecture** to achieve true test isolation and enable parallel test execution. Each test runs in its own dedicated PostgreSQL schema, eliminating shared state and data contamination between tests.

This approach provides:

- ✅ **True Isolation**: Each test has its own complete database schema with independent data
- ✅ **Parallel Execution**: Tests can run concurrently without interference (4-8x faster)
- ✅ **Simple Cleanup**: Just drop the schema instead of complex deletion logic
- ✅ **No Serial Constraints**: No need for `#[serial]` or manual locking
- ✅ **Better Reliability**: Foreign key constraints never conflict between tests

## How It Works

### 1. Schema Creation

When a test starts, a unique schema is created:

```rust
// Test helper creates unique schema per test
let schema = format!("test_{}", uuid::Uuid::new_v4().simple());

// Create schema in database
sqlx::query(&format!("CREATE SCHEMA {}", schema))
    .execute(&pool)
    .await?;

// Set search_path for all connections
sqlx::query(&format!("SET search_path TO {}", schema))
    .execute(&pool)
    .await?;
```

Schema names follow the pattern: `test_<uuid>` (e.g., `test_a1b2c3d4e5f6...`)

### 2. Migration Execution

Each test schema gets its own complete set of tables:

```rust
// Run migrations in the test schema
// Migrations are schema-agnostic (no hardcoded "attune." prefixes)
for migration in migrations {
    sqlx::query(&migration.sql)
        .execute(&pool)
        .await?;
}
```

All 17 Attune tables are created:
- `pack`, `action`, `trigger`, `sensor`, `rule`, `event`, `enforcement`
- `execution`, `inquiry`, `identity`, `key`, `workflow_definition`
- `workflow_execution`, `notification`, `artifact`, `queue_stats`, etc.

### 3. Search Path Mechanism

PostgreSQL's `search_path` determines which schema to use for unqualified table names:

```sql
-- Set once per connection
SET search_path TO test_a1b2c3d4;

-- Now all queries use the test schema automatically
SELECT * FROM pack;           -- Resolves to test_a1b2c3d4.pack
INSERT INTO action (...);     -- Resolves to test_a1b2c3d4.action
```

This is set via the `after_connect` hook in `Database::new()`:

```rust
.after_connect(move |conn, _meta| {
    let schema = schema_for_hook.clone();
    Box::pin(async move {
        let search_path = if schema.starts_with("test_") {
            format!("SET search_path TO {}", schema)
        } else {
            format!("SET search_path TO {}, public", schema)
        };
        sqlx::query(&search_path).execute(&mut *conn).await?;
        Ok(())
    })
})
```

### 4. Test Execution

Tests run with isolated data:

```rust
#[tokio::test]
async fn test_create_pack() {
    // Each test gets its own TestContext with unique schema
    let ctx = TestContext::new().await;
    
    // Create pack in this test's schema only
    let pack = create_test_pack(&ctx.pool).await;
    
    // Other tests running in parallel don't see this data
    assert_eq!(pack.name, "test-pack");
    
    // Cleanup happens automatically when TestContext drops
}
```

### 5. Automatic Cleanup

**Schema is automatically dropped when the test completes** via Rust's `Drop` trait:

```rust
impl Drop for TestContext {
    fn drop(&mut self) {
        // Cleanup happens synchronously to ensure it completes before test exits
        let schema = self.schema.clone();
        
        // Block on async cleanup using the current tokio runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async move {
                if let Err(e) = cleanup_test_schema(&schema).await {
                    eprintln!("Failed to cleanup test schema {}: {}", schema, e);
                } else {
                    tracing::info!("Test context cleanup completed for schema: {}", schema);
                }
            });
        }
        
        // Also cleanup test packs directory
        std::fs::remove_dir_all(&self.test_packs_dir).ok();
    }
}

async fn cleanup_test_schema(schema_name: &str) -> Result<()> {
    // Drop entire schema with CASCADE
    // This removes all tables, data, functions, types, etc.
    let base_pool = create_base_pool().await?;
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema_name))
        .execute(&base_pool)
        .await?;
    Ok(())
}
```

**Key Points:**
- Cleanup is **synchronous** (blocks until complete) to ensure schema is dropped before test exits
- Uses `tokio::runtime::Handle::block_on()` to run async cleanup in the current runtime
- Drops the entire schema with `CASCADE`, removing all objects in one operation
- Also cleans up the test-specific packs directory
- Logs success/failure for debugging

This means **you don't need to manually cleanup** - just let `TestContext` go out of scope:

```rust
#[tokio::test]
async fn test_something() {
    let ctx = TestContext::new().await;
    // ... run your test ...
    // Schema automatically dropped here when ctx goes out of scope
}
```

## Production vs. Test Configuration

### Production Configuration

Production always uses the `attune` schema:

```yaml
# config.production.yaml
database:
  schema: "attune"  # REQUIRED: Do not change
```

The database layer validates and logs schema usage:

```rust
if schema != "attune" {
    tracing::warn!("Using non-standard schema: '{}'. Production should use 'attune'", schema);
} else {
    tracing::info!("Using production schema: {}", schema);
}
```

### Test Configuration

Tests use dynamic schemas:

```yaml
# config.test.yaml
database:
  schema: null  # Will be set per-test in TestContext
```

Each test creates its own unique schema at runtime.

## Code Structure

### Test Helper (`crates/api/tests/helpers.rs`)

```rust
pub struct TestContext {
    pub pool: PgPool,
    pub app: Router,
    pub token: Option<String>,
    pub user: Option<Identity>,
    pub schema: String,  // Unique per test
}

impl TestContext {
    pub async fn new() -> Self {
        // 1. Connect to base database
        let base_pool = create_base_pool().await;
        
        // 2. Create unique test schema
        let schema = format!("test_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {}", schema))
            .execute(&base_pool)
            .await
            .expect("Failed to create test schema");
        
        // 3. Create schema-specific pool with search_path set
        let pool = create_schema_pool(&schema).await;
        
        // 4. Run migrations in test schema
        run_test_migrations(&pool, &schema).await;
        
        // 5. Build test app
        let app = build_test_app(pool.clone());
        
        Self {
            pool,
            app,
            token: None,
            user: None,
            schema,
        }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Cleanup happens here
    }
}
```

### Database Layer (`crates/common/src/db.rs`)

```rust
impl Database {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let schema = config.schema.clone().unwrap_or_else(|| "attune".to_string());
        
        // Validate schema name (security)
        Self::validate_schema_name(&schema)?;
        
        // Log schema usage
        if schema != "attune" {
            warn!("Using non-standard schema: '{}'", schema);
        } else {
            info!("Using production schema: {}", schema);
        }
        
        // Create pool with search_path hook
        let pool = PgPoolOptions::new()
            .after_connect(move |conn, _meta| {
                let schema = schema_for_hook.clone();
                Box::pin(async move {
                    let search_path = if schema.starts_with("test_") {
                        format!("SET search_path TO {}", schema)
                    } else {
                        format!("SET search_path TO {}, public", schema)
                    };
                    sqlx::query(&search_path).execute(&mut *conn).await?;
                    Ok(())
                })
            })
            .connect(&config.url)
            .await?;
        
        Ok(Self { pool, schema })
    }
}
```

### Repository Queries (Schema-Agnostic)

All repository queries use unqualified table names:

```rust
// ✅ CORRECT: Schema-agnostic
sqlx::query_as::<_, Pack>("SELECT * FROM pack WHERE id = $1")
    .bind(id)
    .fetch_one(pool)
    .await

// ❌ WRONG: Hardcoded schema
sqlx::query_as::<_, Pack>("SELECT * FROM attune.pack WHERE id = $1")
    .bind(id)
    .fetch_one(pool)
    .await
```

The `search_path` automatically resolves `pack` to the correct schema:
- Production: `attune.pack`
- Test: `test_a1b2c3d4.pack`

### Migration Files (Schema-Agnostic)

Migrations don't specify schema prefixes:

```sql
-- ✅ CORRECT: Schema-agnostic
CREATE TABLE pack (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    ...
);

-- ❌ WRONG: Hardcoded schema
CREATE TABLE attune.pack (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    ...
);
```

## Running Tests

### Run All Tests (Parallel)

```bash
cargo test
# Tests run in parallel across multiple threads
```

### Run Specific Test File

```bash
cargo test --test api_packs_test
```

### Run Single Test

```bash
cargo test test_create_pack
```

### Verbose Output

```bash
cargo test -- --nocapture --test-threads=1
```

### Using Makefile

```bash
make test                 # Run all tests
make test-integration     # Run integration tests only
```

## Maintenance

### Cleanup Orphaned Schemas

**Normal test execution:** Schemas are automatically cleaned up via the `Drop` implementation in `TestContext`.

**However, if tests are interrupted** (Ctrl+C, crash, panic before Drop runs, etc.), schemas may accumulate:

```bash
# Manual cleanup
./scripts/cleanup-test-schemas.sh

# With custom database
DATABASE_URL="postgresql://user:pass@host/db" ./scripts/cleanup-test-schemas.sh

# Force mode (no confirmation)
./scripts/cleanup-test-schemas.sh --force
```

The cleanup script:
- Finds all schemas matching `test_%` pattern
- Drops them with CASCADE (removes all objects)
- Processes in batches to avoid shared memory issues
- Provides progress reporting and verification

### Automated Cleanup

Add to CI/CD:

```yaml
# .github/workflows/test.yml
jobs:
  test:
    steps:
      - name: Run tests
        run: cargo test
      
      - name: Cleanup test schemas
        if: always()
        run: ./scripts/cleanup-test-schemas.sh --force
```

Or use a cron job:

```bash
# Cleanup every night at 3am
0 3 * * * /path/to/attune/scripts/cleanup-test-schemas.sh --force
```

### Monitoring Schema Count

Check for schema accumulation:

```bash
# Count test schemas
psql $DATABASE_URL -c "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'test_%';"

# List all test schemas
psql $DATABASE_URL -c "SELECT nspname FROM pg_namespace WHERE nspname LIKE 'test_%' ORDER BY nspname;"
```

If the count grows over time, tests are not cleaning up properly. Run the cleanup script.

## Troubleshooting

### Tests Fail: "Schema does not exist"

**Cause:** Test schema creation failed or was prematurely dropped

**Solution:**
1. Check database connection: `psql $DATABASE_URL`
2. Verify user has CREATE privilege: `GRANT CREATE ON DATABASE attune_test TO postgres;`
3. Check disk space and PostgreSQL limits
4. Review test output for error messages
5. Check if `TestContext` is being dropped too early (ensure it lives for entire test duration)

### Tests Fail: "Too many connections"

**Cause:** Connection pool exhaustion from many parallel tests

**Solution:**
1. Reduce `max_connections` in `config.test.yaml`
2. Increase PostgreSQL's `max_connections` setting
3. Run tests with fewer threads: `cargo test -- --test-threads=4`

### Cleanup Script Fails: "Out of shared memory"

**Cause:** Too many schemas to drop at once (this shouldn't happen with automatic cleanup, but can occur if many tests were killed)

**Solution:** The script now handles this automatically by processing in batches of 50. If you still see this error, reduce the `BATCH_SIZE` in the script.

**Prevention:** The automatic cleanup in `TestContext::Drop` prevents schema accumulation under normal circumstances.

### Performance Degradation

**Cause:** Too many accumulated schemas (usually from interrupted tests)

**Note:** With automatic cleanup via `Drop`, schemas should not accumulate during normal test execution.

**Solution:**
```bash
# Check schema count
psql $DATABASE_URL -c "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'test_%';"

# If count is high (>100), cleanup - likely from interrupted tests
./scripts/cleanup-test-schemas.sh --force
```

**Prevention:** Avoid killing tests with SIGKILL; use Ctrl+C instead to allow Drop to run.

### SQLx Compile-Time Checks Fail

**Cause:** SQLx macros need schema in search_path during compilation

**Solution:** Use offline mode (already configured):
```bash
# Generate query metadata
cargo sqlx prepare

# Compile using offline mode
cargo build
# or
cargo test
```

See `.sqlx/` directory for cached query metadata.

## Benefits Summary

### Before Schema-Per-Test

- ❌ Serial execution with `#[serial]` attribute
- ❌ Complex cleanup logic with careful deletion order
- ❌ Foreign key constraint conflicts between tests
- ❌ Data contamination if cleanup fails
- ❌ Slow test suite (~20 seconds per test file)

### After Schema-Per-Test

- ✅ Parallel execution (no serial constraints)
- ✅ Simple cleanup (drop schema)
- ✅ No foreign key conflicts
- ✅ Complete isolation between tests
- ✅ Fast test suite (~4-5 seconds per test file, 4-8x speedup)
- ✅ Better reliability and developer experience

## Migration History

This architecture was implemented in phases:

1. **Phase 1**: Updated all migrations to remove schema prefixes
2. **Phase 2**: Updated all repositories to be schema-agnostic
3. **Phase 3**: Enhanced database layer with dynamic schema configuration
4. **Phase 4**: Overhauled test infrastructure to create/destroy schemas
5. **Phase 5**: Removed all serial test constraints
6. **Phase 6**: Enabled SQLx offline mode for compile-time checks
7. **Phase 7**: Added production safety measures and validation
8. **Phase 8**: Created cleanup utility script
9. **Phase 9**: Updated documentation

See `docs/plans/schema-per-test-refactor.md` for complete implementation details.

## References

- [PostgreSQL search_path Documentation](https://www.postgresql.org/docs/current/ddl-schemas.html#DDL-SCHEMAS-PATH)
- [SQLx Compile-Time Verification](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query)
- [Running Tests Guide](./running-tests.md)
- [Production Deployment Guide](./production-deployment.md)
- [Schema-Per-Test Refactor Plan](./plans/schema-per-test-refactor.md)

## See Also

- [Testing Status](./testing-status.md)
- [Running Tests](./running-tests.md)
- [Database Architecture](./queue-architecture.md)
- [Configuration Guide](./configuration.md)