# Dependency Management Guide

This guide covers how to manage dependencies in the Attune project.

---

## Current Dependency Versions (as of 2026-01-17)

### Core Dependencies

| Category | Package | Version | Purpose |
|----------|---------|---------|---------|
| **Async Runtime** | tokio | 1.49.0 | Async runtime and I/O |
| | tokio-util | 0.7 | Tokio utilities |
| | async-trait | 0.1 | Async trait support |
| | futures | 0.3 | Futures utilities |
| **Web Framework** | axum | 0.7.9 | HTTP server framework |
| | tower | 0.5.3 | Service abstraction |
| | tower-http | 0.6 | HTTP middleware |
| **Database** | sqlx | 0.8.6 | PostgreSQL async driver |
| | sea-query | 0.31 | Query builder |
| | sea-query-postgres | 0.5 | PostgreSQL dialect |
| **Message Queue** | lapin | 2.5.5 | RabbitMQ client |
| | redis | 0.27.6 | Redis client |
| **Serialization** | serde | 1.0 | Serialization framework |
| | serde_json | 1.0 | JSON support |
| **Security** | argon2 | 0.5 | Password hashing |
| | ring | 0.17 | Cryptography |
| | aes-gcm | 0.10 | AES encryption |
| | sha2 | 0.10 | SHA hashing |
| | base64 | 0.22 | Base64 encoding |

---

## Workspace Dependency Management

Attune uses Cargo's workspace feature to manage dependencies centrally.

### Structure

```
attune/
├── Cargo.toml              # Workspace root - defines all dependencies
└── crates/
    ├── common/
    │   └── Cargo.toml      # References workspace dependencies
    ├── api/
    │   └── Cargo.toml      # References workspace dependencies
    └── ...
```

### Adding a New Dependency

#### 1. Add to Workspace Root

Edit `Cargo.toml` in the workspace root:

```toml
[workspace.dependencies]
# Add your dependency here
my-new-crate = "1.0"
```

#### 2. Reference in Crate

Edit the specific crate's `Cargo.toml`:

```toml
[dependencies]
my-new-crate = { workspace = true }
```

### Benefits of Workspace Dependencies

- ✅ Single version across all crates (no version conflicts)
- ✅ Centralized version management
- ✅ Easier to update (change once, applies everywhere)
- ✅ Consistent feature flags across workspace

---

## Updating Dependencies

### Check for Updates

```bash
# Install cargo-outdated
cargo install cargo-outdated

# Check for outdated dependencies
cargo outdated
```

### Update Specific Dependency

```bash
# Update a specific dependency to latest compatible version
cargo update -p tokio

# Update to a specific version
cargo update -p tokio --precise 1.49.0
```

### Update All Dependencies

```bash
# Update all to latest compatible versions
cargo update

# Update and allow breaking changes (requires Cargo.toml edits)
# 1. Edit version in Cargo.toml
# 2. Run:
cargo update
```

### Test After Updates

```bash
# Build and test
cargo build
cargo test --workspace

# Check for unused dependencies
cargo install cargo-udeps
cargo +nightly udeps
```

---

## Security Auditing

### Cargo Audit

Install and run security audit:

```bash
# Install
cargo install cargo-audit

# Audit dependencies for known vulnerabilities
cargo audit

# Fix vulnerabilities where possible
cargo audit fix
```

### GitHub Dependabot

Attune uses GitHub Dependabot to automatically:
- Monitor dependencies for security vulnerabilities
- Create pull requests for security updates
- Track outdated dependencies

**Configuration:** `.github/dependabot.yml`

---

## SQLx Offline Compilation

SQLx requires special handling for query macros.

### Generate Query Cache

```bash
# Set database URL
export DATABASE_URL="postgresql://user:pass@localhost:5432/attune"

# Generate query metadata cache
cargo sqlx prepare --workspace

# Commit the cache
git add .sqlx/
git commit -m "Update SQLx query cache"
```

### Compile Without Database

Once query cache is generated:

```bash
# Compile offline using cached query metadata
SQLX_OFFLINE=true cargo build
```

### When to Update Query Cache

Update the cache when:
- Database schema changes (after running migrations)
- SQL queries are added or modified
- SQLx version is upgraded

---

## Dependency Update Strategy

### Security Updates (Immediate)

Apply security patches as soon as available:

```bash
cargo audit
cargo update -p <vulnerable-package>
cargo test
```

### Minor Updates (Monthly)

Update minor versions monthly:

```bash
cargo outdated
cargo update
cargo build
cargo test --workspace
```

### Major Updates (Quarterly)

Plan for major version upgrades:

1. **Review changelog** for breaking changes
2. **Test in development** environment
3. **Update incrementally** (one major dependency at a time)
4. **Run full test suite**
5. **Deploy to staging** for integration testing
6. **Monitor production** after deployment

---

## Common Issues

### Version Conflicts

**Problem:** Different crates require incompatible versions.

**Solution:**
```bash
# Check dependency tree
cargo tree -d

# Identify conflicting versions
cargo tree -i <package-name>

# Update to compatible versions in Cargo.toml
```

### Build Cache Issues

**Problem:** Stale build artifacts causing errors.

**Solution:**
```bash
# Clean build cache
cargo clean

# Rebuild
cargo build
```

### SQLx Type Errors

**Problem:** `error[E0282]: type annotations needed`

**Solution:**
```bash
# Set DATABASE_URL
export DATABASE_URL="postgresql://user:pass@localhost:5432/attune"

# Or generate query cache
cargo sqlx prepare --workspace
```

### Outdated Lockfile

**Problem:** `Cargo.lock` out of sync with `Cargo.toml`

**Solution:**
```bash
# Regenerate lockfile
cargo update

# Or delete and regenerate
rm Cargo.lock
cargo build
```

---

## Best Practices

### Version Pinning

- ✅ **DO** use semantic versioning in `Cargo.toml`
- ✅ **DO** commit `Cargo.lock` to version control
- ❌ **DON'T** use wildcard versions (`*`) in production
- ❌ **DON'T** use git dependencies in production

### Testing Updates

Before merging dependency updates:

1. ✅ Run full test suite
2. ✅ Check for deprecation warnings
3. ✅ Review dependency changelog
4. ✅ Test integration points (database, message queue, HTTP)
5. ✅ Verify no new vulnerabilities (`cargo audit`)

### Documentation

When adding dependencies:

- Document why the dependency is needed
- Note any special configuration requirements
- Add to this guide if it's a core dependency

---

## Dependency Review Checklist

Before adding a new dependency:

- [ ] Is there an existing dependency that provides this functionality?
- [ ] Is the crate actively maintained? (check last commit date)
- [ ] Does it have a reasonable number of downloads?
- [ ] Are there known security issues? (`cargo audit`)
- [ ] Is the license compatible? (MIT/Apache 2.0 preferred)
- [ ] Does it have minimal transitive dependencies?
- [ ] Is the API stable? (version 1.0+)
- [ ] Does it support async/await if needed?
- [ ] Is it well-documented?

---

## Useful Commands

```bash
# Show dependency tree
cargo tree

# Show dependencies for a specific crate
cargo tree -p attune-common

# Show why a dependency is included
cargo tree -i <package-name>

# Check for duplicate dependencies
cargo tree -d

# Check for outdated dependencies
cargo outdated

# Audit for security vulnerabilities
cargo audit

# Find unused dependencies
cargo +nightly udeps

# Update all dependencies
cargo update

# Update specific dependency
cargo update -p <package-name>

# Clean build artifacts
cargo clean

# Generate SQLx query cache
cargo sqlx prepare --workspace
```

---

## References

- [Cargo Book - Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
- [Cargo Book - Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [SQLx Documentation](https://github.com/launchbadge/sqlx)
- [Cargo Audit](https://github.com/rustsec/rustsec)
- [Semantic Versioning](https://semver.org/)

---

## Maintenance Schedule

### Weekly
- Monitor GitHub Dependabot alerts
- Review and merge security updates

### Monthly
- Run `cargo outdated`
- Update minor versions
- Run full test suite
- Update query cache if needed

### Quarterly
- Review major version upgrades
- Plan breaking change migrations
- Update this documentation
- Audit unused dependencies

---

**Last Updated:** 2026-01-17  
**Last Dependency Update:** 2026-01-17  
**Next Scheduled Review:** 2026-02-17