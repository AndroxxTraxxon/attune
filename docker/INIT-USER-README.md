# Automatic User Initialization

This document explains how Attune automatically creates a default test user when running in Docker.

## Overview

When you start Attune with Docker Compose, a default test user is **automatically created** if it doesn't already exist. This eliminates the need for manual user setup during development and testing.

## Default Credentials

- **Login**: `test@attune.local`
- **Password**: `TestPass123!`
- **Display Name**: `Test User`

## How It Works

### Docker Compose Service Flow

```
1. postgres         → Database starts
2. migrations       → SQLx migrations run (creates schema and tables)
3. init-user        → Creates default test user (if not exists)
4. api/workers/etc  → Application services start
```

All application services depend on `init-user`, ensuring the test user exists before services start.

### Init Script: `init-user.sh`

The initialization script:

1. **Waits** for PostgreSQL to be ready
2. **Checks** if user `test@attune.local` already exists
3. **Creates** user if it doesn't exist (using pre-computed Argon2id hash)
4. **Skips** creation if user already exists (idempotent)

**Key Features:**
- ✅ Idempotent - safe to run multiple times
- ✅ Fast - uses pre-computed password hash
- ✅ Configurable via environment variables
- ✅ Automatic - no manual intervention needed

## Using the Default User

### Test Login via API

```bash
curl -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"test@attune.local","password":"TestPass123!"}'
```

**Successful Response:**
```json
{
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "token_type": "Bearer",
    "expires_in": 86400,
    "user": {
      "id": 1,
      "login": "test@attune.local",
      "display_name": "Test User"
    }
  }
}
```

### Use Token for API Requests

```bash
# Get token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"test@attune.local","password":"TestPass123!"}' \
  | jq -r '.data.access_token')

# Use token for authenticated request
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs
```

## Customization

### Environment Variables

You can customize the default user by setting environment variables in `docker-compose.yaml`:

```yaml
init-user:
  environment:
    TEST_LOGIN: admin@company.com
    TEST_PASSWORD: SuperSecure123!
    TEST_DISPLAY_NAME: Administrator
```

### Custom Password Hash

For production or custom passwords, generate an Argon2id hash:

```bash
# Using Rust (requires project built)
cargo run --example hash_password "YourPasswordHere"

# Output: $argon2id$v=19$m=19456,t=2,p=1$...
```

Then update `init-user.sh` with your custom hash.

## Security Considerations

### Development vs Production

⚠️ **IMPORTANT SECURITY WARNINGS:**

1. **Default credentials are for development/testing ONLY**
   - Never use `test@attune.local` / `TestPass123!` in production
   - Disable or remove the `init-user` service in production deployments

2. **Change credentials before production**
   - Set strong, unique passwords
   - Use environment variables or secrets management
   - Never commit credentials to version control

3. **Disable init-user in production**
   ```yaml
   # In production docker-compose.override.yml
   services:
     init-user:
       profiles: ["dev"]  # Only runs with --profile dev
   ```

### Production User Creation

In production, create users via:

1. **Initial admin migration** - One-time database migration for bootstrap admin
2. **API registration endpoint** - If public registration is enabled
3. **Admin interface** - Web UI user management
4. **CLI tool** - `attune auth register` with proper authentication

## Troubleshooting

### User Creation Failed

**Symptom**: `init-user` container exits with error

**Check logs:**
```bash
docker-compose logs init-user
```

**Common issues:**
- Database not ready → Increase wait time or check database health
- Migration not complete → Verify `migrations` service completed successfully
- Schema mismatch → Ensure `DB_SCHEMA` matches your database configuration

### User Already Exists Error

This is **normal** and **expected** on subsequent runs. The script detects existing users and skips creation.

### Cannot Login with Default Credentials

**Verify user exists:**
```bash
docker-compose exec postgres psql -U attune -d attune \
  -c "SELECT id, login, display_name FROM attune.identity WHERE login = 'test@attune.local';"
```

**Expected output:**
```
 id |       login       | display_name
----+-------------------+--------------
  1 | test@attune.local | Test User
```

**If user doesn't exist:**
```bash
# Recreate user by restarting init-user service
docker-compose up -d init-user
docker-compose logs -f init-user
```

### Wrong Password

If you customized `TEST_PASSWORD` but the login fails, you may need to regenerate the password hash. The default hash only works for `TestPass123!`.

## Files

- **`docker/init-user.sh`** - Initialization script
- **`docker-compose.yaml`** - Service definition for `init-user`
- **`docs/testing/test-user-setup.md`** - Detailed user setup guide

## Related Documentation

- [Test User Setup Guide](../docs/testing/test-user-setup.md) - Manual user creation
- [Docker README](./README.md) - Docker configuration overview
- [Production Deployment](../docs/deployment/production-deployment.md) - Production setup

## Quick Commands

```bash
# View init-user logs
docker-compose logs init-user

# Recreate default user
docker-compose restart init-user

# Check if user exists
docker-compose exec postgres psql -U attune -d attune \
  -c "SELECT * FROM attune.identity WHERE login = 'test@attune.local';"

# Test login
curl -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"test@attune.local","password":"TestPass123!"}'
```

## Summary

- ✅ **Automatic**: User created on first startup
- ✅ **Idempotent**: Safe to run multiple times
- ✅ **Fast**: Uses pre-computed password hash
- ✅ **Configurable**: Customize via environment variables
- ✅ **Documented**: Clear credentials in comments and logs
- ⚠️ **Development only**: Not for production use

The automatic user initialization makes it easy to get started with Attune in Docker without manual setup steps!