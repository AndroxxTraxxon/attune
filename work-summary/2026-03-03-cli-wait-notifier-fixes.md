# CLI `--wait` and Notifier WebSocket Fixes

**Date**: 2026-03-03  
**Session type**: Bug investigation and fix

## Summary

Investigated and fixed a long-standing hang in `attune action execute --wait` and the underlying root-cause bugs in the notifier service. The `--wait` flag now works reliably, returning within milliseconds of execution completion via WebSocket notifications.

## Problems Found and Fixed

### Bug 1: PostgreSQL `PgListener` broken after sequential `listen()` calls (Notifier)

**File**: `crates/notifier/src/postgres_listener.rs`

**Symptom**: The notifier service never received any PostgreSQL LISTEN/NOTIFY messages after startup. Direct `pg_notify()` calls from psql also went undelivered.

**Root cause**: The notifier called `listener.listen(channel)` in a loop — once per channel — totalling 9 separate calls. In sqlx 0.8, each `listen()` call sends a `LISTEN` command and reads a `ReadyForQuery` response. The repeated calls left the connection in an unexpected state where subsequent `recv()` calls would never fire, even though the PostgreSQL backend showed the connection as actively `LISTEN`-ing.

**Fix**: Replaced the loop with a single `listener.listen_all(NOTIFICATION_CHANNELS.iter().copied()).await` call, which issues all 9 LISTEN commands in one round-trip. Extracted a `create_listener()` helper so the same single-call pattern is used on reconnect.

```crates/notifier/src/postgres_listener.rs#L93-135
async fn create_listener(&self) -> Result<PgListener> {
    let mut listener = PgListener::connect(&self.database_url)
        .await
        .context("Failed to connect PostgreSQL listener")?;

    // Use listen_all for a single round-trip instead of N separate commands
    listener
        .listen_all(NOTIFICATION_CHANNELS.iter().copied())
        .await
        .context("Failed to LISTEN on notification channels")?;

    Ok(listener)
}
```

Also added:
- A 60-second heartbeat log (`INFO: PostgreSQL listener heartbeat`) so it's easy to confirm the task is alive during idle periods
- `tokio::time::timeout` wrapper on `recv()` so the heartbeat fires even when no notifications arrive
- Improved reconnect logging

### Bug 2: Notifications serialized without the `"type"` field (Notifier → CLI)

**File**: `crates/notifier/src/websocket_server.rs`

**Symptom**: Even after fixing Bug 1, the CLI's WebSocket loop received messages but `serde_json::from_str::<ServerMsg>(&txt)` always failed with `missing field 'type'`, silently falling through the `Err(_)` catch-all arm.

**Root cause**: The outgoing notification task serialized the raw `Notification` struct directly:
```rust
match serde_json::to_string(&notification) { ... }
```

The `Notification` struct has no `type` field. The CLI's `ServerMsg` enum uses `#[serde(tag = "type")]`, so it expects `{"type":"notification",...}`. The bare struct produces `{"notification_type":"...","entity_type":"...",...}` — no `"type"` key.

**Fix**: Wrap the notification in the `ClientMessage` tagged enum before serializing:
```rust
let envelope = ClientMessage::Notification(notification);
match serde_json::to_string(&envelope) { ... }
```

This produces the correct `{"type":"notification","notification_type":"...","entity_type":"...","entity_id":...,"payload":{...}}` format.

### Bug 3: Polling fallback used exhausted deadline (CLI)

**File**: `crates/cli/src/wait.rs`

**Symptom**: When `--wait` fell back to polling (e.g. when WS notifications weren't delivered), the polling would immediately time out even though the execution had long since completed.

**Root cause**: Both the WebSocket path and the polling fallback shared a single `deadline = Instant::now() + timeout_secs`. The WS path ran until the deadline, leaving 0 time for polling.

**Fix**: Reserve a minimum polling budget (`MIN_POLL_BUDGET = 10s`) so the WS path exits early enough to leave polling headroom:
```rust
const MIN_POLL_BUDGET: Duration = Duration::from_secs(10);
let ws_deadline = if overall_deadline > Instant::now() + MIN_POLL_BUDGET {
    overall_deadline - MIN_POLL_BUDGET
} else {
    overall_deadline  // very short timeout — skip WS, go straight to polling
};
```

Polling always uses `overall_deadline` directly (the full user-specified timeout), so at minimum `MIN_POLL_BUDGET` of polling time is guaranteed.

### Additional CLI improvement: poll-first in polling loop

The polling fallback now checks the execution status **immediately** on entry (before sleeping) rather than sleeping first. This catches the common case where the execution already completed while the WS path was running.

Also improved error handling in the poll loop: REST failures are logged and retried rather than propagating as fatal errors.

## End-to-End Verification

```
$ attune --profile docker action execute core.echo --param message="Hello!" --wait
ℹ Executing action: core.echo
ℹ Waiting for execution 51 to complete...
  [notifier] connected to ws://localhost:8081/ws
  [notifier] session id client_2
  [notifier] subscribed to entity:execution:51
  [notifier] execution_status_changed for execution 51 — status=Some("scheduled")
  [notifier] execution_status_changed for execution 51 — status=Some("running")
  [notifier] execution_status_changed for execution 51 — status=Some("completed")
✓ Execution 51 completed
```

Three consecutive runs all returned via WebSocket within milliseconds, no polling fallback triggered.

## Files Changed

| File | Change |
|------|--------|
| `crates/notifier/src/postgres_listener.rs` | Replace sequential `listen()` loop with `listen_all()`; add `create_listener()` helper; add heartbeat logging with timeout-wrapped recv |
| `crates/notifier/src/websocket_server.rs` | Wrap `Notification` in `ClientMessage::Notification(...)` before serializing for outgoing WS messages |
| `crates/notifier/src/service.rs` | Handle `RecvError::Lagged` and `RecvError::Closed` in broadcaster; add `debug` import |
| `crates/notifier/src/subscriber_manager.rs` | Scale broadcast result logging back to `debug` level |
| `crates/cli/src/wait.rs` | Fix shared-deadline bug with `MIN_POLL_BUDGET`; poll immediately on entry; improve error handling and verbose logging |
| `AGENTS.md` | Document notifier WebSocket protocol and the `listen_all` requirement |

## Key Protocol Facts (for future reference)

**Notifier WebSocket — server → client message format**:
```json
{"type":"notification","notification_type":"execution_status_changed","entity_type":"execution","entity_id":42,"user_id":null,"payload":{...execution row...},"timestamp":"..."}
```

**Notifier WebSocket — client → server subscribe format**:
```json
{"type":"subscribe","filter":"entity:execution:42"}
```

Filter formats supported: `all`, `entity_type:<type>`, `entity:<type>:<id>`, `user:<id>`, `notification_type:<type>`

**Critical rule**: Always use `PgListener::listen_all()` for subscribing to multiple PostgreSQL channels. Individual `listen()` calls in a loop leave the listener in a broken state in sqlx 0.8.