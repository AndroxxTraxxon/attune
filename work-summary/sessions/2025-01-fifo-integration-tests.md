# FIFO Ordering Integration Tests - Execution Plan

**Date**: 2025-01-27  
**Status**: Ready for Execution  
**Priority**: P0 - Critical  

## Overview

Comprehensive integration and stress tests have been created to validate the FIFO policy execution ordering system. These tests verify end-to-end correctness, performance under load, and edge case handling.

## Test Suite Location

**File**: `crates/executor/tests/fifo_ordering_integration_test.rs`

## Test Coverage

### 1. Basic Integration Test: `test_fifo_ordering_with_database`
**Purpose**: Verify FIFO ordering with database persistence  
**Scenario**: 10 executions, concurrency=1  
**Validates**:
- Database execution creation
- Queue manager integration
- Queue stats persistence to database
- Strict FIFO ordering
- Completion notification flow

**Expected Runtime**: ~2-3 seconds

---

### 2. High Concurrency Stress Test: `test_high_concurrency_stress`
**Purpose**: Validate ordering under heavy load  
**Scenario**: 1000 executions, concurrency=5  
**Validates**:
- FIFO ordering maintained at scale
- Queue stats accuracy under load
- Performance characteristics (throughput)
- Memory efficiency
- Database write performance

**Expected Runtime**: ~30-60 seconds  
**Expected Throughput**: >100 exec/sec

---

### 3. Multiple Workers Simulation: `test_multiple_workers_simulation`
**Purpose**: Simulate real-world worker behavior  
**Scenario**: 30 executions, 3 workers with varying speeds  
**Validates**:
- FIFO ordering independent of worker speed
- Worker completion rate tracking
- Load distribution across workers
- Async notification correctness

**Expected Runtime**: ~5-10 seconds

---

### 4. Cross-Action Independence: `test_cross_action_independence`
**Purpose**: Ensure actions don't interfere with each other  
**Scenario**: 3 actions × 50 executions each (150 total)  
**Validates**:
- Separate queues per action
- Concurrent action execution
- Independent queue statistics
- Interleaved completion handling

**Expected Runtime**: ~10-15 seconds

---

### 5. Cancellation Test: `test_cancellation_during_queue`
**Purpose**: Verify cancellation removes from queue correctly  
**Scenario**: 10 queued executions, cancel 3 specific positions  
**Validates**:
- Cancellation removes correct execution
- Queue length decreases appropriately
- Remaining executions proceed in order
- Cancelled tasks return errors

**Expected Runtime**: ~2-3 seconds

---

### 6. Queue Stats Persistence: `test_queue_stats_persistence`
**Purpose**: Verify database sync during execution  
**Scenario**: 50 executions with periodic stats checks  
**Validates**:
- Memory and database stats match
- Real-time stats updates
- Final stats accuracy
- Database write consistency

**Expected Runtime**: ~5-10 seconds

---

### 7. Queue Full Rejection: `test_queue_full_rejection`
**Purpose**: Test queue limit enforcement  
**Scenario**: Fill queue to max_queue_length (10), attempt overflow  
**Validates**:
- Queue full detection
- Rejection with clear error message
- Stats accuracy at capacity
- No memory overflow

**Expected Runtime**: ~2 seconds

---

### 8. Extreme Stress Test: `test_extreme_stress_10k_executions`
**Purpose**: Validate system under extreme load  
**Scenario**: 10,000 executions, concurrency=10  
**Validates**:
- FIFO ordering at extreme scale
- Memory stability
- Database performance
- No resource leaks
- Throughput metrics

**Expected Runtime**: ~5-10 minutes  
**Note**: This is marked `#[ignore]` and should be run separately

---

## Execution Instructions

### Prerequisites

1. **Database Setup**:
   ```bash
   # Ensure PostgreSQL is running
   sudo systemctl start postgresql
   
   # Apply latest migrations
   cd attune
   sqlx migrate run
   ```

2. **Environment Configuration**:
   ```bash
   # Ensure config.development.yaml has correct database URL
   # Or set environment variable:
   export ATTUNE__DATABASE__URL="postgresql://attune:attune@localhost/attune"
   ```

### Running Tests

#### Run All Tests (Except Extreme Stress)
```bash
cd attune/crates/executor
cargo test --test fifo_ordering_integration_test -- --ignored --test-threads=1
```

**Important**: Use `--test-threads=1` to avoid database contention between tests.

#### Run Individual Test
```bash
# Basic integration test
cargo test --test fifo_ordering_integration_test test_fifo_ordering_with_database -- --ignored --nocapture

# High concurrency stress test
cargo test --test fifo_ordering_integration_test test_high_concurrency_stress -- --ignored --nocapture

# Multiple workers simulation
cargo test --test fifo_ordering_integration_test test_multiple_workers_simulation -- --ignored --nocapture
```

#### Run Extreme Stress Test (Separately)
```bash
# Run with verbose output to see progress
cargo test --test fifo_ordering_integration_test test_extreme_stress_10k_executions -- --ignored --nocapture --test-threads=1
```

### Expected Output

All tests should:
- ✅ Pass with `test result: ok`
- ✅ Show progress messages during execution
- ✅ Report throughput metrics (stress tests)
- ✅ Complete within expected runtime
- ✅ Clean up test data automatically

---

## Performance Benchmarks

### Target Metrics

| Metric | Target | Acceptable |
|--------|--------|------------|
| Throughput (1000 exec) | >200/sec | >100/sec |
| Throughput (10k exec) | >500/sec | >300/sec |
| Memory per queued exec | <1 KB | <5 KB |
| Queue stats DB write | <5ms | <10ms |
| Cancellation latency | <1ms | <5ms |

### Measuring Performance

```bash
# Run with timing
time cargo test --test fifo_ordering_integration_test test_high_concurrency_stress -- --ignored --nocapture

# Monitor memory usage
/usr/bin/time -v cargo test --test fifo_ordering_integration_test test_extreme_stress_10k_executions -- --ignored --nocapture
```

---

## Troubleshooting

### Test Failures

#### Database Connection Issues
```
Error: Failed to connect to database
```
**Solution**: Ensure PostgreSQL is running and connection URL is correct.

#### Queue Full Errors During Tests
```
Error: Queue full (max length: 10000)
```
**Solution**: Increase `max_queue_length` in test config or reduce concurrent spawns.

#### Timeout Errors
```
Error: Execution timed out waiting for queue slot
```
**Solution**: Increase `queue_timeout_seconds` or check for completion notification issues.

#### FIFO Order Violations
```
assertion failed: order == expected
```
**Solution**: This is a CRITICAL bug. Check:
- Notify mechanism is working
- Queue entry/exit logic
- Lock ordering
- Race conditions in queue manager

### Database Cleanup

If tests crash and leave test data:
```sql
-- Clean up test data
DELETE FROM attune.queue_stats WHERE action_id IN (
    SELECT id FROM attune.action WHERE pack IN (
        SELECT id FROM attune.pack WHERE ref LIKE 'fifo_test_pack_%'
    )
);

DELETE FROM attune.execution WHERE action IN (
    SELECT id FROM attune.action WHERE pack IN (
        SELECT id FROM attune.pack WHERE ref LIKE 'fifo_test_pack_%'
    )
);

DELETE FROM attune.action WHERE pack IN (
    SELECT id FROM attune.pack WHERE ref LIKE 'fifo_test_pack_%'
);

DELETE FROM attune.pack WHERE ref LIKE 'fifo_test_pack_%';
```

---

## Test Maintenance

### Adding New Tests

When adding new test scenarios:

1. **Follow naming convention**: `test_<scenario_description>`
2. **Mark as `#[ignore]`**: All tests require database
3. **Use unique suffix**: Include timestamp to avoid conflicts
4. **Clean up data**: Always call `cleanup_test_data()` at end
5. **Document expectations**: Add comments explaining what's validated

### Updating Tests

When modifying queue manager behavior:

1. Review all tests for assumptions
2. Update expected values if behavior changes
3. Add new tests for new features
4. Run full suite before committing

---

## Integration with CI/CD

### GitHub Actions Workflow (Future)

```yaml
name: FIFO Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_DB: attune
          POSTGRES_USER: attune
          POSTGRES_PASSWORD: attune
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v3
      - name: Run migrations
        run: sqlx migrate run
      - name: Run integration tests
        run: |
          cargo test --test fifo_ordering_integration_test -- --ignored --test-threads=1
      - name: Run stress test
        if: github.ref == 'refs/heads/main'
        run: |
          cargo test --test fifo_ordering_integration_test test_high_concurrency_stress -- --ignored --nocapture
```

---

## Success Criteria

All tests must:
- ✅ Pass consistently (3+ consecutive runs)
- ✅ Maintain strict FIFO ordering
- ✅ Complete within 2x expected runtime
- ✅ Show no memory leaks (valgrind/miri)
- ✅ Clean up all test data
- ✅ Work with concurrent test runs

---

## Next Steps

1. **Run Test Suite**: Execute all tests and verify they pass
2. **Document Results**: Record actual performance metrics
3. **Fix Any Issues**: Address test failures before production
4. **Update TODO**: Mark Step 7 as complete
5. **Proceed to Step 8**: Documentation and production readiness

---

## Related Documents

- `work-summary/2025-01-policy-ordering-plan.md` - Implementation plan
- `work-summary/FIFO-ORDERING-STATUS.md` - Overall status
- `crates/executor/src/queue_manager.rs` - Queue manager implementation
- `crates/executor/tests/fifo_ordering_integration_test.rs` - Test suite