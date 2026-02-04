# End-to-End Integration Testing Setup

**Date**: 2026-01-17 (Session 6)  
**Phase**: Production Readiness - Integration Testing  
**Status**: 🔄 IN PROGRESS  
**Priority**: P0 - BLOCKING

---

## Overview

Set up comprehensive end-to-end integration testing infrastructure to verify all 5 Attune services work correctly together. This is a critical milestone before production deployment.

---

## What Was Accomplished

### 1. Test Planning & Documentation

**Created**: `tests/README.md` (564 lines)

Comprehensive test plan covering:
- **8 Test Scenarios**: Timer automation, workflows, FIFO queues, secrets, inquiries, error handling, notifications, dependency isolation
- **Test Infrastructure**: Prerequisites, service configuration, running tests
- **Debugging Guide**: Service logs, database queries, message queue inspection
- **Success Criteria**: Clear checklist for passing tests

**Test Scenarios Defined:**

1. **Basic Timer Automation** (~30s)
   - Timer → Event → Rule → Enforcement → Execution → Completion
   - Verifies core automation chain

2. **Workflow Execution** (~45s)
   - 3-task sequential workflow
   - Verifies task ordering and variable propagation

3. **FIFO Queue Ordering** (~20s)
   - 5 executions with concurrency limit
   - Verifies execution ordering preserved

4. **Secret Management** (~15s)
   - Action uses secrets via stdin
   - Verifies secrets not in environment

5. **Human-in-the-Loop (Inquiry)** (~30s)
   - Execution pauses for user input
   - Verifies pause/resume flow

6. **Error Handling & Recovery** (~25s)
   - Action fails with retries
   - Verifies retry logic

7. **Real-Time Notifications** (~20s)
   - WebSocket updates on execution changes
   - Verifies notification delivery

8. **Dependency Isolation** (~40s)
   - Two packs with conflicting dependencies
   - Verifies per-pack virtual environments

---

### 2. E2E Test Configuration

**Created**: `config.e2e.yaml` (204 lines)

Test-specific configuration:
- Separate test database: `attune_e2e`
- Different ports: API=18080, WebSocket=18081
- Faster polling intervals for quicker tests
- Lower bcrypt cost for faster auth tests
- Test-specific directories for artifacts/logs/venvs
- Minimal logging (info level)
- All features enabled

**Key Settings:**
```yaml
environment: test
database.url: postgresql://postgres:postgres@localhost:5432/attune_e2e
server.port: 18080
executor.enforcement_poll_interval: 1  # Faster for tests
sensor.poll_interval_seconds: 2  # Faster for tests
worker.max_concurrent_executions: 10
```

---

### 3. Test Fixtures

**Created Test Pack**: `tests/fixtures/packs/test_pack/`

**Pack Metadata** (`pack.yaml`):
- Pack ref: `test_pack`
- Version: 1.0.0
- Python dependency: requests>=2.28.0
- Runtime: python3

**Echo Action** (`actions/echo.yaml` + `echo.py`):
- Simple action that echoes a message
- Supports delay parameter for timing tests
- Supports fail parameter for error testing
- Returns timestamp and execution time
- 87 lines of Python implementation

**Features:**
- JSON input/output via stdin/stdout
- Parameter validation
- Configurable delay (0-30 seconds)
- Intentional failure mode for testing
- Error handling and logging

**Simple Workflow** (`workflows/simple_workflow.yaml`):
- 3-task sequential workflow
- Tests task ordering
- Tests variable passing
- Tests workflow completion
- Input parameters: workflow_message, workflow_delay

**Task Flow:**
1. `task_start` - Echo start message, publish start_time
2. `task_wait` - Delay for specified seconds
3. `task_complete` - Echo completion message

---

### 4. Test Infrastructure Setup

**Created Directory Structure:**
```
tests/
├── README.md                    # Test documentation (564 lines)
├── fixtures/                    # Test data
│   └── packs/                   # Test packs
│       └── test_pack/          # E2E test pack
│           ├── pack.yaml       # Pack metadata
│           ├── actions/        # Action definitions
│           │   ├── echo.yaml   # Echo action spec
│           │   └── echo.py     # Echo action implementation
│           ├── workflows/      # Workflow definitions
│           │   └── simple_workflow.yaml
│           └── sensors/        # Sensor definitions (empty)
```

---

### 5. Documentation Updates

**Updated**: `work-summary/TODO.md`

- Marked API authentication as complete
- Reorganized priorities with E2E testing as Priority 1
- Updated success criteria checklist
- Added E2E testing to critical path

---

## Test Infrastructure Components

### Services Required

1. **PostgreSQL** - Database (port 5432)
2. **RabbitMQ** - Message queue (ports 5672, 15672)
3. **Redis** - Cache (optional, port 6379)

### Attune Services

1. **API** - Port 18080
2. **Executor** - Background service
3. **Worker** - Background service
4. **Sensor** - Background service
5. **Notifier** - Port 18081 (WebSocket)

### Database Setup

```bash
# Create E2E test database
createdb attune_e2e

# Run migrations
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune_e2e"
sqlx migrate run
```

---

## Running Tests (Manual)

### Start All Services

**Terminal 1 - API:**
```bash
cd crates/api
ATTUNE__CONFIG_FILE=../../config.e2e.yaml cargo run
```

**Terminal 2 - Executor:**
```bash
cd crates/executor
ATTUNE__CONFIG_FILE=../../config.e2e.yaml cargo run
```

**Terminal 3 - Worker:**
```bash
cd crates/worker
ATTUNE__CONFIG_FILE=../../config.e2e.yaml cargo run
```

**Terminal 4 - Sensor:**
```bash
cd crates/sensor
ATTUNE__CONFIG_FILE=../../config.e2e.yaml cargo run
```

**Terminal 5 - Notifier:**
```bash
cd crates/notifier
ATTUNE__CONFIG_FILE=../../config.e2e.yaml cargo run
```

---

## Next Steps

### Phase 1: Infrastructure (Current - 80% Complete)
- [x] Document test plan
- [x] Create config.e2e.yaml
- [x] Create test fixtures
- [x] Set up directory structure
- [ ] Create test database and seed data
- [ ] Verify all services start with E2E config
- [ ] Create test helper utilities

### Phase 2: Basic Tests (Next)
- [ ] Implement helper modules (api_client, service_manager)
- [ ] Write timer automation test
- [ ] Write workflow execution test
- [ ] Write FIFO ordering test
- [ ] Verify all basic scenarios pass

### Phase 3: Advanced Tests
- [ ] Write secret management test
- [ ] Write inquiry flow test
- [ ] Write error handling test
- [ ] Write notification test
- [ ] Write dependency isolation test

### Phase 4: Automation
- [ ] Create service start/stop scripts
- [ ] Create automated test runner
- [ ] Add CI/CD integration
- [ ] Add performance benchmarks

---

## Technical Decisions

### Why Separate E2E Config?

1. **Isolation**: Separate database prevents test pollution
2. **Different Ports**: Avoid conflicts with dev services
3. **Faster Polling**: Reduce test duration
4. **Lower Security**: Faster tests (bcrypt_cost=4)
5. **Minimal Logging**: Cleaner test output

### Why Test Fixtures?

1. **Consistency**: Same pack used across all tests
2. **Simplicity**: Echo action is trivial to verify
3. **Flexibility**: Supports delay and failure modes
4. **Realistic**: Real pack structure, not mocks

### Why Manual Service Start First?

1. **Debugging**: Easier to see service output
2. **Iteration**: Faster test development cycle
3. **Validation**: Verify config works before automation
4. **Later**: Automate once tests are stable

---

## Files Created

1. `tests/README.md` - Test documentation (564 lines)
2. `config.e2e.yaml` - E2E test configuration (204 lines)
3. `tests/fixtures/packs/test_pack/pack.yaml` - Pack metadata (51 lines)
4. `tests/fixtures/packs/test_pack/actions/echo.yaml` - Action spec (43 lines)
5. `tests/fixtures/packs/test_pack/actions/echo.py` - Action implementation (87 lines)
6. `tests/fixtures/packs/test_pack/workflows/simple_workflow.yaml` - Workflow (56 lines)
7. `work-summary/2026-01-17-e2e-test-setup.md` - This document

**Total**: 7 files, ~1,000 lines of test infrastructure

---

## Files Modified

1. `work-summary/TODO.md` - Updated priorities and status
2. `work-summary/TODO.OLD.md` - Moved old TODO for archival

---

## Challenges & Solutions

### Challenge: Multiple Services to Coordinate
**Solution**: Created clear documentation with step-by-step service startup instructions

### Challenge: Test Isolation
**Solution**: Separate database, different ports, dedicated config file

### Challenge: Fast Test Execution
**Solution**: Faster polling intervals, lower bcrypt cost, minimal logging

### Challenge: Realistic Test Data
**Solution**: Created actual pack with real action structure (not mocks)

---

## Success Criteria

For E2E tests to be considered complete:

- [x] Test plan documented (8 scenarios)
- [x] Test infrastructure created
- [x] Test fixtures created (pack + action + workflow)
- [x] E2E configuration file created
- [ ] All services start with E2E config
- [ ] Database seeded with test data
- [ ] Basic timer test passing
- [ ] Workflow test passing
- [ ] All 8 test scenarios passing
- [ ] No errors in service logs
- [ ] Clean shutdown of all services

---

## Estimated Timeline

**Phase 1: Setup** (Current)
- Infrastructure setup: ✅ Complete
- Database setup: ⏳ 1 hour
- Service verification: ⏳ 2 hours
- Helper utilities: ⏳ 2 hours
- **Total**: ~5 hours remaining

**Phase 2: Basic Tests**
- Timer automation test: 2-3 hours
- Workflow execution test: 2-3 hours
- FIFO ordering test: 2-3 hours
- **Total**: 6-9 hours

**Phase 3: Advanced Tests**
- Secret management: 2-3 hours
- Inquiry flow: 2-3 hours
- Error handling: 2-3 hours
- Notifications: 2-3 hours
- Dependency isolation: 2-3 hours
- **Total**: 10-15 hours

**Phase 4: Automation**
- Scripts: 2-3 hours
- CI/CD: 3-4 hours
- **Total**: 5-7 hours

**Grand Total**: 26-36 hours (3-5 days)

---

## Benefits Achieved

1. **Clear Test Strategy**: 8 well-defined scenarios
2. **Comprehensive Documentation**: 564 lines of test guide
3. **Realistic Fixtures**: Actual pack structure for testing
4. **Isolated Environment**: Won't interfere with development
5. **Fast Iteration**: Faster polling for quicker test runs
6. **Production-Like**: Tests full service integration

---

## Lessons Learned

1. **Document First**: Writing test plan revealed edge cases
2. **Realistic Fixtures**: Better than mocks for integration tests
3. **Separate Config**: Essential for test isolation
4. **Manual First**: Easier to debug before automation

---

## Next Session Goals

1. **Create E2E Database**
   - Create attune_e2e database
   - Run migrations
   - Seed with test pack and user

2. **Verify Service Startup**
   - Start all 5 services with E2E config
   - Verify database connections
   - Verify message queue connections
   - Check for any configuration issues

3. **Implement First Test**
   - Create test helper utilities
   - Write timer automation test
   - Get first green test passing

---

**Status**: 🔄 IN PROGRESS (Phase 1: ~80% complete)  
**Priority**: P0 - BLOCKING  
**Confidence**: HIGH - Clear path forward  
**Next Milestone**: All services running with E2E config