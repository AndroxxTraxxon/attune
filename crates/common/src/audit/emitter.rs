//! `AuditEmitter` — clone-able non-blocking handle used by services to record
//! audit events.

use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

use super::PendingAuditEvent;

/// Clone-able handle. Sending is non-blocking and lock-free.
///
/// If the writer task has been dropped (e.g. during shutdown) the send is
/// silently logged and discarded — audit emission must never break the
/// request path.
#[derive(Debug, Clone)]
pub struct AuditEmitter {
    tx: Option<UnboundedSender<PendingAuditEvent>>,
}

impl AuditEmitter {
    /// Construct an emitter that pushes onto the given channel.
    pub fn new(tx: UnboundedSender<PendingAuditEvent>) -> Self {
        Self { tx: Some(tx) }
    }

    /// Construct a no-op emitter. Useful in tests, or where audit logging is
    /// disabled by configuration.
    pub fn noop() -> Self {
        Self { tx: None }
    }

    /// Returns true if this emitter is configured to actually send events.
    pub fn is_active(&self) -> bool {
        self.tx.is_some()
    }

    /// Emit an event. Returns immediately. Failures are logged at WARN level
    /// and dropped.
    pub fn emit(&self, event: PendingAuditEvent) {
        let Some(tx) = &self.tx else {
            return;
        };
        if let Err(err) = tx.send(event) {
            warn!(error = %err, "audit emitter: writer task dropped, audit event lost");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditCategory, AuditEventBuilder, AuditOutcome};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn emit_via_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let emitter = AuditEmitter::new(tx);
        emitter.emit(
            AuditEventBuilder::new(AuditCategory::Api, "api.request", AuditOutcome::Success)
                .build(),
        );
        let received = rx.recv().await.expect("event received");
        assert_eq!(received.event_type, "api.request");
    }

    #[tokio::test]
    async fn noop_emitter_does_nothing() {
        let emitter = AuditEmitter::noop();
        assert!(!emitter.is_active());
        emitter.emit(
            AuditEventBuilder::new(AuditCategory::Api, "api.request", AuditOutcome::Success)
                .build(),
        );
    }

    #[tokio::test]
    async fn dropped_receiver_is_logged_not_panicked() {
        let (tx, rx) = mpsc::unbounded_channel();
        drop(rx);
        let emitter = AuditEmitter::new(tx);
        emitter.emit(
            AuditEventBuilder::new(AuditCategory::Api, "api.request", AuditOutcome::Success)
                .build(),
        );
    }
}
