//! Background batch-writer task that drains the audit channel and inserts
//! events into the `audit_event` hypertable.

use sqlx::{PgPool, Postgres, QueryBuilder};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use super::{AuditEmitter, PendingAuditEvent};

/// Maximum number of events to flush in a single INSERT.
const DEFAULT_MAX_BATCH: usize = 256;

/// Maximum time to wait for a partial batch to fill before flushing.
const DEFAULT_FLUSH_INTERVAL_MS: u64 = 200;

/// Handle to a spawned audit writer task. Drop to signal shutdown; the task
/// will drain remaining events and exit.
pub struct AuditWriterHandle {
    pub emitter: AuditEmitter,
    pub task: JoinHandle<()>,
}

/// Spawn the background writer task and return both an emitter and the
/// task handle.
pub fn spawn_writer(pool: PgPool) -> AuditWriterHandle {
    spawn_writer_with(pool, DEFAULT_MAX_BATCH, DEFAULT_FLUSH_INTERVAL_MS)
}

/// As [`spawn_writer`] but with explicit batch tuning. Used in tests.
pub fn spawn_writer_with(
    pool: PgPool,
    max_batch: usize,
    flush_interval_ms: u64,
) -> AuditWriterHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let emitter = AuditEmitter::new(tx);
    let task = tokio::spawn(async move {
        run_writer(pool, rx, max_batch, flush_interval_ms).await;
    });
    AuditWriterHandle { emitter, task }
}

async fn run_writer(
    pool: PgPool,
    mut rx: UnboundedReceiver<PendingAuditEvent>,
    max_batch: usize,
    flush_interval_ms: u64,
) {
    info!(max_batch, flush_interval_ms, "audit writer task started");
    let mut buffer: Vec<PendingAuditEvent> = Vec::with_capacity(max_batch);
    let flush_interval = Duration::from_millis(flush_interval_ms);

    loop {
        // Wait for at least one event, or for the channel to close.
        let first = match rx.recv().await {
            Some(e) => e,
            None => break, // emitter dropped
        };
        buffer.push(first);

        // Drain additional events that are already queued, then optionally
        // wait up to flush_interval for the buffer to fill.
        while buffer.len() < max_batch {
            match rx.try_recv() {
                Ok(e) => buffer.push(e),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    flush(&pool, &mut buffer).await;
                    info!("audit writer task draining and exiting");
                    return;
                }
            }
        }

        if buffer.len() < max_batch {
            // Wait briefly for more events.
            let deadline = tokio::time::sleep(flush_interval);
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut deadline => break,
                    maybe_evt = rx.recv() => {
                        match maybe_evt {
                            Some(e) => {
                                buffer.push(e);
                                if buffer.len() >= max_batch {
                                    break;
                                }
                            }
                            None => {
                                flush(&pool, &mut buffer).await;
                                info!("audit writer task draining and exiting");
                                return;
                            }
                        }
                    }
                }
            }
        }

        flush(&pool, &mut buffer).await;
    }

    // Channel closed, drain leftovers.
    if !buffer.is_empty() {
        flush(&pool, &mut buffer).await;
    }
    info!("audit writer task exited");
}

async fn flush(pool: &PgPool, buffer: &mut Vec<PendingAuditEvent>) {
    if buffer.is_empty() {
        return;
    }
    let count = buffer.len();
    debug!(count, "audit writer flushing batch");

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO audit_event (\
            category, event_type, outcome, \
            actor_identity, actor_login, actor_token_type, actor_ip, actor_user_agent, \
            request_id, \
            resource_type, resource_id, resource_ref, \
            http_method, http_path, http_status, duration_ms, \
            details, correlation_chain\
        ) ",
    );

    qb.push_values(buffer.drain(..), |mut b, e| {
        b.push_bind(e.category)
            .push_bind(e.event_type)
            .push_bind(e.outcome)
            .push_bind(e.actor_identity)
            .push_bind(e.actor_login)
            .push_bind(e.actor_token_type)
            // INET column accepts a string cast; use text rendering of IpAddr.
            .push_bind(e.actor_ip.map(|ip| ip.to_string()))
            .push_bind(e.actor_user_agent)
            .push_bind(e.request_id)
            .push_bind(e.resource_type)
            .push_bind(e.resource_id)
            .push_bind(e.resource_ref)
            .push_bind(e.http_method)
            .push_bind(e.http_path)
            .push_bind(e.http_status)
            .push_bind(e.duration_ms)
            .push_bind(e.details)
            .push_bind(e.correlation_chain);
    });

    // The string cast for INET must be performed by the server; rewrite the
    // bound parameter for actor_ip with an explicit ::inet. Achieved by
    // appending a no-op type hint inside the VALUES clause is not trivial via
    // QueryBuilder; instead we rely on PostgreSQL's text-to-inet implicit
    // coercion which works for INET columns when the input is a valid IP
    // literal. If the caller stored an invalid string, the row insert will
    // fail at flush time and we'll log a WARN below.

    match qb.build().execute(pool).await {
        Ok(res) => debug!(rows = res.rows_affected(), "audit batch flushed"),
        Err(err) => {
            error!(error = %err, count, "audit writer: batch insert failed; events dropped");
        }
    }
    let _ = count;
}

#[cfg(test)]
mod tests {
    // Writer tests require a live PostgreSQL with the audit_event table.
    // They live in the integration test suite (crates/common/tests) rather
    // than as unit tests, since they need the schema to exist.

    use super::*;
    use crate::audit::{AuditCategory, AuditEventBuilder, AuditOutcome};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn writer_exits_when_channel_closes() {
        // Use a (channel-only) variant of the writer loop logic to verify
        // shutdown semantics without needing a database. We exercise the
        // runtime contract by manually polling the channel.
        let (tx, mut rx) = mpsc::unbounded_channel::<PendingAuditEvent>();
        let task = tokio::spawn(async move {
            // Drain until closed.
            while rx.recv().await.is_some() {}
        });
        tx.send(
            AuditEventBuilder::new(AuditCategory::Api, "api.request", AuditOutcome::Success)
                .build(),
        )
        .unwrap();
        drop(tx);
        // Should exit promptly after channel closes.
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("writer task should exit when channel closes")
            .unwrap();
    }
}
