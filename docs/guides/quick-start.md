# Attune API Quick Start Guide

Get the Attune API up and running in minutes!

## Prerequisites

- Rust 1.70+ installed
- PostgreSQL 13+ installed and running
- `sqlx-cli` (will be installed if needed)

## Step 1: Database Setup

The API needs a PostgreSQL database. Run the setup script:

```bash
cd attune
./scripts/setup-db.sh
```

This will:
- Create the `attune` database
- Run all migrations
- Set up the schema

**If the script doesn't work**, do it manually:

```bash
# Connect to PostgreSQL
psql -U postgres

# Create database
CREATE DATABASE attune;

# Exit psql
\q

# Run migrations
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
cargo install sqlx-cli --no-default-features --features postgres
sqlx migrate run
```

## Step 2: Configure the API

The API uses a YAML configuration file. Create your config from the example:

```bash
cp config.example.yaml config.yaml
```

**Edit the configuration file:**

```bash
nano config.yaml
```

Key settings to review:

```yaml
database:
  url: postgresql://postgres:postgres@localhost:5432/attune

security:
  jwt_secret: your-secret-key-change-this
  encryption_key: your-32-char-encryption-key-here

server:
  port: 8080
  cors_origins:
    - http://localhost:3000
```

**Generate secure secrets for production:**

```bash
# Generate JWT secret
openssl rand -base64 64

# Generate encryption key
openssl rand -base64 32
```

**If your database uses different credentials**, update the database URL in `config.yaml`:

```yaml
database:
  url: postgresql://YOUR_USER:YOUR_PASSWORD@localhost:5432/attune
```

## Step 3: Start the API

Simply run:

```bash
cargo run --bin attune-api
```

You should see:
```
INFO Starting Attune API Service
INFO Loaded configuration from config.yaml
INFO Configuration loaded successfully
INFO Environment: development
INFO Connecting to database...
INFO Database connection established
INFO JWT configuration initialized (access: 3600s, refresh: 604800s)
INFO Starting server on 0.0.0.0:8080
INFO Server listening on 0.0.0.0:8080
INFO Attune API Service is ready
```

## Step 4: Test It!

### Health Check

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{
  "status": "healthy"
}
```

### Register a User

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "login": "admin",
    "password": "admin123456",
    "display_name": "Administrator"
  }'
```

You'll get back access and refresh tokens:
```json
{
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### Use the Token

Save your access token and use it for authenticated requests:

```bash
# Replace YOUR_TOKEN with the actual access_token from above
curl http://localhost:8080/auth/me \
  -H "Authorization: Bearer YOUR_TOKEN"
```

Response:
```json
{
  "data": {
    "id": 1,
    "login": "admin",
    "display_name": "Administrator"
  }
}
```

## Step 5: Explore the API

### Available Endpoints

**Authentication:**
- `POST /auth/register` - Register new user
- `POST /auth/login` - Login
- `POST /auth/refresh` - Refresh token
- `GET /auth/me` - Get current user (protected)
- `POST /auth/change-password` - Change password (protected)

**Health:**
- `GET /health` - Basic health check
- `GET /health/detailed` - Detailed status with DB check
- `GET /health/ready` - Readiness probe
- `GET /health/live` - Liveness probe

**Packs:**
- `GET /api/v1/packs` - List all packs
- `POST /api/v1/packs` - Create pack
- `GET /api/v1/packs/:ref` - Get pack by reference
- `PUT /api/v1/packs/:ref` - Update pack
- `DELETE /api/v1/packs/:ref` - Delete pack

### Create a Pack

```bash
curl -X POST http://localhost:8080/api/v1/packs \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "core.basics",
    "name": "Basic Operations",
    "description": "Core automation pack",
    "version": "1.0.0",
    "author": "Admin"
  }'
```

## Configuration Options

The `.env` file supports many configuration options. See `.env.example` for all available settings.

### Common Customizations

**Change the port:**
```bash
ATTUNE__SERVER__PORT=3000
```
### Debug Logging

Edit `config.yaml`:

```yaml
log:
  level: debug
  format: pretty  # Human-readable output
```

Or use environment variables:

```bash
export ATTUNE__LOG__LEVEL=debug
export ATTUNE__LOG__FORMAT=pretty
cargo run --bin attune-api
```
### Longer Token Expiration (Development)

Edit `config.yaml`:

```yaml
security:
  jwt_access_expiration: 7200      # 2 hours
  jwt_refresh_expiration: 2592000  # 30 days
```
### Database Connection Pool

Edit `config.yaml`:

```yaml
database:
  max_connections: 100
  min_connections: 10
```

## Troubleshooting

### Database Connection Failed

```
Error: error communicating with database
```

**Solution:**
1. Verify PostgreSQL is running: `pg_isready`
2. Check credentials in `.env` file
3. Ensure database exists: `psql -U postgres -l | grep attune`

### Migration Errors

```
Error: migration version not found
```

**Solution:**
```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
sqlx migrate run
```

### Port Already in Use

```
Error: Address already in use
```

**Solution:** Change the port in `.env`:
```bash
ATTUNE__SERVER__PORT=8081
```

### JWT Secret Warning

```
WARN JWT_SECRET not set in config, using default (INSECURE for production!)
```

**Solution:** The default `.env` file has this set. Make sure:
1. The `.env` file exists in the `attune/` directory
2. The variable is set: `ATTUNE__SECURITY__JWT_SECRET=your-secret-here`

## Development Tips

### Auto-reload on Changes

Use `cargo-watch` for automatic rebuilds:

```bash
cargo install cargo-watch
cargo watch -x 'run --bin attune-api'
```

### Enable SQL Query Logging

In `.env`:
Edit `config.yaml`:

```yaml
database:
  log_statements: true
```

### Pretty Logs

For development, use pretty formatting:
Edit `config.yaml`:

```yaml
log:
  format: pretty
```

For production, use JSON:
Edit `config.yaml`:

```yaml
log:
  format: json
```

## Next Steps

- Read the [Authentication Guide](./authentication.md)
- Learn about [Testing](./testing-authentication.md)
- See [API Reference](./auth-quick-reference.md) for all endpoints
- Check out the [Architecture Documentation](./architecture.md)

## Production Deployment

Before deploying to production:

1. **Change JWT Secret:**
   ```bash
   ATTUNE__SECURITY__JWT_SECRET=$(openssl rand -base64 64)
   ```

2. **Use Environment Variables:**
   Don't commit `.env` to version control. Use your platform's secrets management.

3. **Enable HTTPS:**
   Configure TLS/SSL termination at your load balancer or reverse proxy.

3. **Use production configuration:**
   ```bash
   # Use production config file
   export ATTUNE_CONFIG=config.production.yaml
   
   # Or set environment
   export ATTUNE__ENVIRONMENT=production
   ```

4. **Adjust connection pool** in `config.production.yaml`:
   ```yaml
   database:
     max_connections: 100
     min_connections: 10
   ```

6. **Enable JSON Logging:**
   ```bash
   ATTUNE__LOG__FORMAT=json
   ```

## Getting Help

- Documentation: `docs/` directory
- Issues: GitHub Issues
- API Quick Reference: `docs/auth-quick-reference.md`

Happy automating! 🚀
**Configure CORS origins:**
```bash
# Default (empty) - allows localhost:3000, localhost:8080, 127.0.0.1:3000, 127.0.0.1:8080
ATTUNE__SERVER__CORS_ORIGINS=

# Custom origins (comma-separated)
ATTUNE__SERVER__CORS_ORIGINS=http://localhost:3000,https://app.example.com
```

**Note:** CORS origins must be specified when using authentication (credentials). The API cannot use wildcard origins (`*`) with credentials enabled.
