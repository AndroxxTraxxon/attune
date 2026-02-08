# Worker Graceful Shutdown and Heartbeat Validation

**Date:** 2026-02-04  
**Status:** Complete  
**Services Modified:** `attune-worker`, `attune-executor`

## Overview

Implemented graceful shutdown handling for workers and added heartbeat validation in the executor to prevent scheduling executions to stale or unavailable workers.

## Problem Statement

Workers were not properly marking themselves as offline when shutting down, leading to:
- Executors attempting to schedule work to terminated workers
- Failed executions due to worker unavailability
- No validation of worker health before scheduling

## Changes Implemented

### 1. Worker Graceful Shutdown (`attune-worker`)

**File:** `crates/worker/src/main.rs`

- **Signal Handling:** Added proper handling for `SIGINT` and `SIGTERM` signals using tokio's Unix signal API
- **Shutdown Flow:** Workers now properly deregister (mark as inactive) before shutdown
- **Service Lifecycle:** Separated `start()` and `stop()` calls from signal handling logic

**Key Changes:**
```rust
// Setup signal handlers for graceful shutdown
let mut sigint = signal(SignalKind::interrupt())?;
let mut sigterm = signal(SignalKind::terminate())?;

tokio::select! {
    _ = sigint.recv() => {
        info!("Received SIGINT signal");
    }
    _ = sigterm.recv() => {
        info!("Received SIGTERM signal");
    }
}

// Stop the service and mark worker as inactive
service.stop().await?;
```

**File:** `crates/worker/src/service.rs`

- **Removed:** `run()` method that mixed signal handling with service logic
- **Rationale:** Signal handling is now cleanly separated in `main.rs`, making the service module more testable and focused

### 2. Executor Heartbeat Validation (`attune-executor`)

**File:** `crates/executor/src/scheduler.rs`

Added heartbeat freshness validation before scheduling executions to workers.

**Constants:**
```rust
const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;          // seconds
const HEARTBEAT_STALENESS_MULTIPLIER: u64 = 3;        // 3x interval
// Max age = 90 seconds (3 * 30s)
```

**New Function:** `is_worker_heartbeat_fresh()`
- Checks if worker's `last_heartbeat` timestamp exists
- Validates heartbeat is within `HEARTBEAT_INTERVAL * STALENESS_MULTIPLIER` (90 seconds)
- Logs warnings for stale workers
- Returns `false` if no heartbeat recorded

**Integration:** Added heartbeat filtering in `select_worker()` flow:
```rust
// Filter by heartbeat freshness (only workers with recent heartbeats)
let fresh_workers: Vec<_> = active_workers
    .into_iter()
    .filter(|w| Self::is_worker_heartbeat_fresh(w))
    .collect();

if fresh_workers.is_empty() {
    return Err(anyhow::anyhow!(
        "No workers with fresh heartbeats available"
    ));
}
```

**Worker Selection Order:**
1. Filter by runtime compatibility
2. Filter by active status
3. **NEW:** Filter by heartbeat freshness
4. Select best worker (currently first available)

### 3. Unit Tests

**File:** `crates/executor/src/scheduler.rs`

Added comprehensive unit tests for heartbeat validation:
- `test_heartbeat_freshness_with_recent_heartbeat` - 30s old (fresh)
- `test_heartbeat_freshness_with_stale_heartbeat` - 100s old (stale)
- `test_heartbeat_freshness_at_boundary` - 90s old (boundary case)
- `test_heartbeat_freshness_with_no_heartbeat` - no heartbeat (stale)
- `test_heartbeat_freshness_with_very_recent` - 5s old (fresh)

**Test Results:** All 6 tests pass ✅

## Technical Details

### Heartbeat Staleness Calculation

- **Default Heartbeat Interval:** 30 seconds (from `WorkerConfig::default_heartbeat_interval`)
- **Staleness Threshold:** 3x heartbeat interval = 90 seconds
- **Rationale:** Allows for up to 2 missed heartbeats plus buffer time before considering worker stale

### Shutdown Sequence

1. Worker receives SIGINT/SIGTERM signal
2. Signal handler triggers graceful shutdown
3. `service.stop()` is called:
   - Stops heartbeat manager
   - Waits 100ms for heartbeat to stop
   - Calls `registration.deregister()`
   - Updates worker status to `Inactive` in database
4. Worker exits cleanly

### Error Handling

- **Stale Workers:** Executor logs warning and excludes from scheduling
- **No Fresh Workers:** Execution scheduling fails with descriptive error message
- **Heartbeat Validation:** Runs on every execution scheduling attempt

## Benefits

1. **Improved Reliability:** Prevents scheduling to dead workers
2. **Faster Failure Detection:** Workers mark themselves offline immediately on shutdown
3. **Better Observability:** Clear logging when workers are stale or unavailable
4. **Graceful Degradation:** System continues operating with remaining healthy workers
5. **Production Ready:** Proper signal handling for containerized environments (Docker, Kubernetes)

## Docker Compatibility

The SIGTERM handling is especially important for containerized environments:
- Docker sends SIGTERM on `docker stop`
- Kubernetes sends SIGTERM during pod termination
- Workers now have 10s (default grace period) to mark themselves offline before forced SIGKILL

## Configuration

No new configuration required. Uses existing `WorkerConfig::heartbeat_interval` (default: 30s).

**Future Enhancement Opportunity:** Add configurable staleness multiplier:
```yaml
worker:
  heartbeat_interval: 30
  heartbeat_staleness_multiplier: 3  # Optional, defaults to 3
```

## Testing Recommendations

### Manual Testing

1. **Worker Graceful Shutdown:**
   ```bash
   # Start worker
   docker compose up worker-shell
   
   # Send SIGTERM
   docker compose stop worker-shell
   
   # Verify in logs: "Deregistering worker ID: X"
   # Verify in DB: worker status = 'inactive'
   ```

2. **Heartbeat Validation:**
   ```bash
   # Stop worker heartbeat (simulate crash)
   docker compose pause worker-shell
   
   # Wait 100 seconds
   
   # Attempt to schedule execution
   # Should fail with "No workers with fresh heartbeats available"
   ```

### Integration Testing

- Test execution scheduling with stale workers
- Test execution scheduling with no workers
- Test worker restart with existing registration
- Test multiple workers with varying heartbeat states

## Related Files

- `crates/worker/src/main.rs` - Signal handling
- `crates/worker/src/service.rs` - Service lifecycle
- `crates/worker/src/registration.rs` - Worker registration/deregistration
- `crates/worker/src/heartbeat.rs` - Heartbeat manager
- `crates/executor/src/scheduler.rs` - Execution scheduling with heartbeat validation
- `crates/common/src/config.rs` - Worker configuration (heartbeat_interval)
- `crates/common/src/models.rs` - Worker model (last_heartbeat field)

## Migration Notes

**No database migration required.** Uses existing `worker.last_heartbeat` column.

**No configuration changes required.** Uses existing heartbeat interval settings.

**Backward Compatible:** Works with existing workers; old workers without proper shutdown will be detected as stale after 90s.

## Future Enhancements

1. **Configurable Staleness Multiplier:** Allow tuning staleness threshold per environment
2. **Worker Health Checks:** Add active health probing beyond passive heartbeat monitoring
3. **Graceful Work Completion:** Allow in-progress executions to complete before shutdown (requires execution state tracking)
4. **Worker Reconnection:** Handle network partitions vs. actual worker failures
5. **Load-Based Selection:** Consider worker load alongside heartbeat freshness

## Conclusion

These changes significantly improve the robustness of the worker infrastructure by ensuring:
- Workers cleanly deregister on shutdown
- Executor only schedules to healthy, responsive workers
- System gracefully handles worker failures and restarts

All changes are backward compatible and require no configuration updates.