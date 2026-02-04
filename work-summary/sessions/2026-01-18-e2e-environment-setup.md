# E2E Environment Setup Complete

**Date:** 2026-01-18  
**Status:** ✅ Complete  
**Phase:** Integration Testing Infrastructure

---

## Summary

Successfully set up the complete E2E (end-to-end) testing environment for Attune. All 5 microservices can now be started together with an isolated test database and configuration. The system is ready for integration testing scenarios.

---

## What Was Accomplished

### 1. E2E Database Setup ✅

**Script:** `scripts/setup-e2e-db.sh`

- Creates isolated `attune_e2e` database
- Runs all 5 consolidated migrations (22 tables + 3 views)
- Generates test user with proper Argon2id password hashing
- Stores credentials correctly in `attributes` JSON field
- Validates schema creation
- Idempotent: can be run multiple times safely

**Test User:**
- Login: `e2e_test_user`
- Password: `test_password_123`
- Hash stored in: `password_hash` column (not attributes JSON)

### 2. E2E Configuration ✅

**File:** `config.e2e.yaml`

**Key Changes:**
- Environment set to `e2e` (prevents `config.test.yaml` override)
- Isolated ports: API (18080), WebSocket (18081)
- Test database: `attune_e2e`
- Faster polling intervals for quicker test execution
- Worker type: `Local` (proper enum value)
- Test-specific directories: `./tests/logs`, `./tests/artifacts`, `./tests/venvs`

### 3. Service Management Scripts ✅

**Start Script:** `scripts/start-e2e-services.sh`
- Checks PostgreSQL and RabbitMQ connectivity
- Builds all service binaries
- Starts services in correct order (API → Executor → Worker → Sensor → Notifier)
- Health checks with proper endpoint (`/api/v1/health`)
- Sets environment: `ATTUNE__ENVIRONMENT=e2e`
- Creates PID files for process management
- Comprehensive logging and status output

**Stop Script:** `scripts/stop-e2e-services.sh`
- Graceful shutdown with SIGTERM
- Force kill if process doesn't stop within 5 seconds
- Stops in reverse dependency order
- Cleans up PID files

### 4. All Services Verified ✅

**Running Services:**
1. **attune-api** (PID: 447551) - Port 18080
2. **attune-executor** (PID: 447567)
3. **attune-worker** (PID: 447592)
4. **attune-sensor** (PID: 447623)
5. **attune-notifier** (PID: 447648) - WebSocket Port 18081

**Service Logs:**
- `tests/logs/api.log`
- `tests/logs/executor.log`
- `tests/logs/worker.log`
- `tests/logs/sensor.log`
- `tests/logs/notifier.log`

### 5. Authentication Verified ✅

**Health Check:**
```bash
curl http://127.0.0.1:18080/api/v1/health
# → {"status":"ok"}
```

**Login Test:**
```bash
curl -X POST http://127.0.0.1:18080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"e2e_test_user","password":"test_password_123"}'
# → Returns access_token, refresh_token, user info
```

---

## Technical Issues Resolved

### Issue 1: Configuration Not Loading
**Problem:** Services were using default config instead of `config.e2e.yaml`

**Root Cause:** Config loader loads base config, then `config.{environment}.yaml`, which was overriding settings

**Solution:** Changed environment from `test` to `e2e` to avoid `config.test.yaml` override

### Issue 2: Health Check Failing
**Problem:** Health check was hitting `/health` which returned 404

**Root Cause:** Health endpoint is at `/api/v1/health`, not `/health`

**Solution:** Updated start script to use correct endpoint path

### Issue 3: Worker Type Enum Mismatch
**Problem:** Config had `worker_type: "general"` which is not a valid enum value

**Root Cause:** `WorkerType` enum has variants: `Local`, `Remote`, `Container`

**Solution:** Changed to `worker_type: "Local"` with proper PascalCase

### Issue 4: Authentication Failing
**Problem:** Login was failing even with correct password

**Root Cause:** API expects `password_hash` in `attributes` JSON field, not in `password_hash` column

**Solution:** Updated setup script to store hash in `attributes->>'password_hash'`

---

## Files Created/Modified

**New Files:**
- `scripts/setup-e2e-db.sh` - E2E database setup
- `scripts/start-e2e-services.sh` - Service startup orchestration
- `scripts/stop-e2e-services.sh` - Service shutdown
- `crates/common/examples/hash_password.rs` - Password hashing utility

**Modified Files:**
- `config.e2e.yaml` - Environment and worker type fixes
- `crates/api/src/main.rs` - Added debug logging for server config

---

## Usage

### Setup E2E Environment
```bash
# 1. Create E2E database
./scripts/setup-e2e-db.sh

# 2. Start all services
./scripts/start-e2e-services.sh

# 3. Verify services are running
curl http://127.0.0.1:18080/api/v1/health

# 4. Test authentication
curl -X POST http://127.0.0.1:18080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"e2e_test_user","password":"test_password_123"}'
```

### Monitor Services
```bash
# View all service logs
tail -f ./tests/logs/*.log

# View specific service
tail -f ./tests/logs/api.log

# Check running processes
ps aux | grep attune-
```

### Stop Services
```bash
./scripts/stop-e2e-services.sh
```

---

## Service Endpoints

| Service | Endpoint | Purpose |
|---------|----------|---------|
| API | http://127.0.0.1:18080 | REST API gateway |
| Health | http://127.0.0.1:18080/api/v1/health | Health check |
| Docs | http://127.0.0.1:18080/docs | OpenAPI documentation |
| WebSocket | ws://127.0.0.1:18081 | Real-time notifications |

---

## Database Details

- **Name:** `attune_e2e`
- **Host:** localhost:5432
- **User:** postgres
- **Tables:** 25 tables in `attune` schema
- **Views:** 3 workflow views
- **URL:** `postgresql://postgres:postgres@localhost:5432/attune_e2e`

---

## Next Steps

### Phase 2: Integration Test Implementation

With the E2E environment fully operational, we can now proceed with:

1. **Create Test Helper Utilities**
   - API client wrapper for authenticated requests
   - Service manager for test lifecycle
   - Database fixture helpers
   - Message queue test utilities

2. **Implement Basic Integration Tests**
   - Test 1: Timer automation (Sensor → Event → Rule → Execution)
   - Test 2: Workflow execution (Multi-task orchestration)
   - Test 3: FIFO queue ordering (Concurrency limits)

3. **Advanced Integration Tests**
   - Test 4: Secret management and encryption
   - Test 5: Human-in-the-loop inquiries
   - Test 6: Error handling and retries
   - Test 7: Real-time notifications via WebSocket
   - Test 8: Dependency isolation (Python venvs)

4. **CI/CD Integration**
   - Docker Compose configuration for CI
   - GitHub Actions workflow
   - Test coverage reporting

---

## Validation Checklist

- ✅ PostgreSQL connectivity verified
- ✅ RabbitMQ connectivity verified
- ✅ All 5 services build successfully
- ✅ All 5 services start without errors
- ✅ API health check returns 200 OK
- ✅ Authentication working with test user
- ✅ JWT tokens generated successfully
- ✅ Database migrations applied (25 tables)
- ✅ Service logs being written
- ✅ Graceful shutdown working
- ✅ Scripts are idempotent

---

## Notes

- **Password Hashing:** Using Argon2id with default params (m=19456, t=2, p=1)
- **Environment Isolation:** E2E environment is completely isolated from development
- **Port Conflicts:** E2E uses non-standard ports to avoid conflicts
- **Test Data:** Database is recreated fresh on each setup run
- **Log Retention:** Logs are not rotated; clean manually as needed
- **Process Management:** Uses PID files in `./tests/pids/`

---

## Troubleshooting

### Services Won't Start
```bash
# Check if ports are in use
lsof -i :18080
lsof -i :18081

# Check service logs
tail -f ./tests/logs/*.log

# Verify dependencies
./scripts/start-e2e-services.sh  # Shows dependency checks
```

### Authentication Fails
```bash
# Regenerate test user
./scripts/setup-e2e-db.sh

# Verify user exists
psql -d attune_e2e -c "SELECT id, login, attributes ? 'password_hash' FROM attune.identity WHERE login='e2e_test_user';"
```

### Database Issues
```bash
# Recreate database
./scripts/setup-e2e-db.sh  # Drops and recreates

# Verify migrations
psql -d attune_e2e -c "SELECT COUNT(*) FROM attune.pack;"
```

---

**Status:** E2E environment is production-ready for integration testing. All services verified and operational.

---

## Additional Fix: Password Hash Column Usage

After initial setup, the authentication system was corrected to use the dedicated `password_hash` column instead of storing hashes in the `attributes` JSON field. See `2026-01-18-password-hash-column-fix.md` for full details.

**Impact:**
- ✅ Better performance (direct column access vs JSON extraction)
- ✅ Proper data model integrity
- ✅ Type safety in Rust code
- ✅ All authentication tests passing
