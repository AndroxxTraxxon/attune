# Test User Setup Guide

This guide explains how to create and manage test users in Attune for development and testing.

## Quick Setup

### Create Default Test User

Run the script to create a test user with default credentials:

```bash
./scripts/create-test-user.sh
```

**Default Credentials:**
- **Login**: `test@attune.local`
- **Password**: `TestPass123!`
- **Display Name**: `Test User`

### Test Login

Once created, test the login via API:

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
      "id": 2,
      "login": "test@attune.local",
      "display_name": "Test User"
    }
  }
}
```

## Custom User Credentials

### Using Environment Variables

Create a user with custom credentials:

```bash
TEST_LOGIN="myuser@example.com" \
TEST_PASSWORD="MySecurePass123!" \
TEST_DISPLAY_NAME="My Custom User" \
./scripts/create-test-user.sh
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TEST_LOGIN` | User login/email | `test@attune.local` |
| `TEST_PASSWORD` | User password | `TestPass123!` |
| `TEST_DISPLAY_NAME` | User display name | `Test User` |
| `ATTUNE_DB_NAME` | Database name | `attune` |
| `ATTUNE_DB_HOST` | Database host | `localhost` |
| `ATTUNE_DB_PORT` | Database port | `5432` |
| `ATTUNE_DB_USER` | Database user | `postgres` |
| `ATTUNE_DB_PASSWORD` | Database password | `postgres` |

## Password Hashing

Attune uses **Argon2id** for password hashing, which is a secure, modern password hashing algorithm.

### Generate Password Hash Manually

To generate a password hash for manual database insertion:

```bash
cargo run --example hash_password "YourPasswordHere"
```

**Example Output:**
```
$argon2id$v=19$m=19456,t=2,p=1$F0UlGNd21LBXF7TWmpD93w$F65DKRjPU6japrzYv3ZcddnMFCtjVIBDWIkiLbkqt2I
```

### Manual Database Insertion

Insert a user directly via SQL:

```sql
INSERT INTO identity (login, display_name, password_hash, attributes)
VALUES (
    'user@example.com',
    'User Name',
    '$argon2id$v=19$m=19456,t=2,p=1$...',  -- Hash from above
    '{}'::jsonb
);
```

## Updating Existing Users

### Update Password via Script

If the user already exists, the script will prompt to update:

```bash
./scripts/create-test-user.sh
# Outputs: "User 'test@attune.local' already exists"
# Prompts: "Do you want to update the password? (y/N):"
```

**Auto-confirm update:**
```bash
echo "y" | ./scripts/create-test-user.sh
```

### Update Password via SQL

Directly update a user's password in the database:

```sql
-- Generate hash first: cargo run --example hash_password "NewPassword"

UPDATE identity
SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$...',
    updated = NOW()
WHERE login = 'test@attune.local';
```

## Multiple Database Support

Create users in different databases/schemas:

### Development Database (public schema)

```bash
ATTUNE_DB_NAME=attune \
./scripts/create-test-user.sh
```

### E2E Test Database

```bash
ATTUNE_DB_NAME=attune_e2e \
./scripts/create-test-user.sh
```

### Custom Database

```bash
ATTUNE_DB_NAME=my_custom_db \
ATTUNE_DB_HOST=db.example.com \
ATTUNE_DB_PASSWORD=secretpass \
./scripts/create-test-user.sh
```

## Verification

### Check User Exists

Query the database to verify user creation:

```bash
psql postgresql://postgres:postgres@localhost:5432/attune \
  -c "SELECT id, login, display_name FROM identity WHERE login = 'test@attune.local';"
```

**Expected Output:**
```
 id |       login       | display_name
----+-------------------+--------------
  2 | test@attune.local | Test User
```

### Test Authentication

Use curl to test login:

```bash
# Login
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"test@attune.local","password":"TestPass123!"}' \
  | jq -r '.data.access_token')

# Use token for authenticated request
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs
```

## Security Considerations

### Development vs Production

⚠️ **Important Security Notes:**

1. **Never use test credentials in production**
   - Default test user (`test@attune.local`) is for development only
   - Change or remove before production deployment

2. **Password Strength**
   - Default password (`TestPass123!`) is intentionally simple for testing
   - Production passwords should be stronger and unique

3. **Database Access**
   - Test user creation requires direct database access
   - In production, create users via API with proper authentication

4. **Password Storage**
   - Never store passwords in plain text
   - Always use the Argon2id hashing mechanism
   - Never commit password hashes to version control

### Production User Creation

In production, users should be created through:

1. **API Registration Endpoint** (if enabled)
2. **Admin Interface** (web UI)
3. **CLI Tool** with proper authentication
4. **Database Migration** for initial admin user only

## Troubleshooting

### Script Fails to Connect

**Error:** `Cannot connect to database attune`

**Solutions:**
- Verify PostgreSQL is running: `pg_isready -h localhost -p 5432`
- Check database exists: `psql -l | grep attune`
- Verify credentials are correct
- Create database: `./scripts/setup-db.sh`

### Password Hash Generation Fails

**Error:** `Failed to generate password hash`

**Solutions:**
- Ensure Rust toolchain is installed: `cargo --version`
- Build the project first: `cargo build`
- Use pre-generated hash for default password (already in script)

### Login Fails After User Creation

**Possible Causes:**

1. **Wrong Database Schema**
   - Verify API service uses same schema as user creation
   - Check config: `grep schema config.development.yaml`

2. **Password Mismatch**
   - Ensure password hash matches the password
   - Regenerate hash: `cargo run --example hash_password "YourPassword"`

3. **User Not Found**
   - Verify user exists in database
   - Check correct database is being queried

### API Returns 401 Unauthorized

**Check:**
- User exists in database
- Password hash is correct
- API service is running and connected to correct database
- JWT secret is configured properly

## Related Documentation

- [Configuration Guide](configuration.md) - Database and security settings
- [API Authentication](api-authentication.md) - JWT tokens and authentication flow
- [Running Tests](running-tests.md) - E2E testing with test users
- [Database Setup](../scripts/setup-db.sh) - Initial database configuration

## Script Location

The test user creation script is located at:
```
scripts/create-test-user.sh
```

**Source Code:**
- Password hashing: `crates/api/src/auth/password.rs`
- Hash example: `crates/common/examples/hash_password.rs`
- Identity model: `crates/common/src/models.rs`

## Examples

### Create Admin User

```bash
TEST_LOGIN="admin@company.com" \
TEST_PASSWORD="SuperSecure123!" \
TEST_DISPLAY_NAME="System Administrator" \
./scripts/create-test-user.sh
```

### Create Multiple Test Users

```bash
# User 1
TEST_LOGIN="alice@test.com" \
TEST_DISPLAY_NAME="Alice Test" \
./scripts/create-test-user.sh

# User 2
TEST_LOGIN="bob@test.com" \
TEST_DISPLAY_NAME="Bob Test" \
./scripts/create-test-user.sh

# User 3
TEST_LOGIN="charlie@test.com" \
TEST_DISPLAY_NAME="Charlie Test" \
./scripts/create-test-user.sh
```

### Create E2E Test User

```bash
ATTUNE_DB_NAME=attune_e2e \
TEST_LOGIN="e2e@test.local" \
TEST_PASSWORD="E2ETest123!" \
./scripts/create-test-user.sh
```

## Summary

- ✅ **Simple**: One command to create test users
- ✅ **Flexible**: Customizable via environment variables
- ✅ **Secure**: Uses Argon2id password hashing
- ✅ **Safe**: Prompts before overwriting existing users
- ✅ **Verified**: Includes login test instructions

For production deployments, use the API or admin interface to create users with proper authentication and authorization checks.