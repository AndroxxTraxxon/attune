# Compilation Notes

## Build Cache Issues

If you see compilation errors that appear to be already fixed in the source code, the build cache may be stale.

### Clear Build Cache

```bash
cargo clean -p <package-name>
# or clean everything
cargo clean
```

Then rebuild:
```bash
cargo build --package <package-name>
```

---

## SQLx Offline Compilation

SQLx macros (`query!`, `query_as!`, `query_scalar!`) perform compile-time verification of SQL queries against the database schema. This requires either:

1. **Online mode:** Database connection available at compile time
2. **Offline mode:** Pre-generated query metadata cache

### Error: Type Annotations Needed

If you see errors like:

```
error[E0282]: type annotations needed
   --> crates/sensor/src/rule_matcher.rs:406:13
    |
406 |         let result = sqlx::query!(
    |             ^^^^^^
```

This means SQLx cannot infer types because:
- No `DATABASE_URL` is set
- Query metadata cache is missing or outdated

### Solution 1: Compile with Database (Recommended)

```bash
export DATABASE_URL="postgresql://user:pass@localhost:5432/attune"
cargo build
```

### Solution 2: Update Query Cache

Generate/update the query metadata cache:

```bash
export DATABASE_URL="postgresql://user:pass@localhost:5432/attune"
cargo sqlx prepare --workspace
```

This creates `.sqlx/` directory with query metadata that allows offline compilation.

Commit the `.sqlx/` directory to version control so others can compile without a database.

### Solution 3: Skip SQLx Checks (Not Recommended)

Disable compile-time verification (queries will only be checked at runtime):

```bash
cargo build --features sqlx/offline
```

---

## Common Compilation Errors

### 1. Mismatched Types in Option Handling

**Error:**
```
error[E0308]: mismatched types
   --> src/file.rs:100:30
    |
100 |     let x = result.and_then(|row| row.field)
    |                              ^^^^^^^^^^^^^^ expected `Option<_>`, found `Value`
```

**Cause:** `and_then` expects a function that returns `Option<T>`, but `row.field` is already `Option<T>`.

**Solution:** Use `map().flatten()` for nested Options:
```rust
// Wrong
let x = result.and_then(|row| row.field);

// Right
let x = result.map(|row| row.field).flatten();
```

### 2. SQLx Query Type Inference

**Error:**
```
error[E0282]: type annotations needed
   --> src/file.rs:50:13
    |
50  |         let result = sqlx::query!(...);
    |             ^^^^^^ type must be known at this point
```

**Cause:** SQLx needs database connection to infer types.

**Solutions:**
- Set `DATABASE_URL` environment variable
- Run `cargo sqlx prepare` to generate cache
- See "SQLx Offline Compilation" section above

### 3. Missing Traits

**Error:**
```
error[E0599]: the method `from_row` exists for struct `X`, but its trait bounds were not satisfied
```

**Cause:** Missing `#[derive(FromRow)]` on model struct.

**Solution:** Add SQLx derive macro:
```rust
use sqlx::FromRow;

#[derive(FromRow)]
pub struct MyModel {
    pub id: i64,
    pub name: String,
}
```

---

## Development Workflow

### Recommended Setup

1. **Keep database running during development:**
   ```bash
   docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:14
   export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
   ```

2. **Apply migrations:**
   ```bash
   sqlx database create
   sqlx migrate run
   ```

3. **Generate query cache (for CI/CD):**
   ```bash
   cargo sqlx prepare --workspace
   git add .sqlx/
   git commit -m "Update SQLx query cache"
   ```

4. **Build normally:**
   ```bash
   cargo build
   ```

### CI/CD Pipeline

For continuous integration without database access:

1. **Commit `.sqlx/` directory** with prepared query metadata
2. **Enable offline mode** in CI:
   ```bash
   SQLX_OFFLINE=true cargo build --release
   ```

---

## Troubleshooting

### Build succeeds but tests fail

```bash
# Ensure database is running and migrations are applied
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune_test"
sqlx database create
sqlx migrate run

# Run tests
cargo test
```

### Query cache out of sync

```bash
# Delete old cache
rm -rf .sqlx/

# Regenerate
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
cargo sqlx prepare --workspace
```

### "prepared statement already exists"

This typically indicates multiple connections trying to prepare the same statement. Solutions:
- Use connection pooling (already implemented in `attune_common::db`)
- Ensure tests use separate database instances
- Clean up connections properly

---

## See Also

- [SQLx Documentation](https://github.com/launchbadge/sqlx)
- [SQLx Offline Mode](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode)
- [Cargo Build Cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)