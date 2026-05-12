# Operational visibility foundation

Implemented the first operational visibility layer for workers, sensor workers, executions, alerts, work queues, and sensor logs.

## Completed

- Added worker cordon metadata and worker-health API fields so operator intent is distinct from observed worker status.
- Added worker cordon/uncordon API and web controls in the runtimes worker inventory.
- Excluded cordoned action workers from executor scheduling.
- Reconciled running executions on unavailable workers to `abandoned` and published normal completion messages.
- Added `core.alert` trigger metadata and a shared system-alert helper for unexpected worker loss and dead-worker execution reconciliation.
- Surfaced current action-worker and sensor-worker health on the dashboard.
- Added sensor-worker labels/taints and pack sensor placement fields (`worker_selector`, `worker_tolerations`, `worker_affinity`).
- Added durable sensor-process health with `sensor_process`, `sensor_process_history`, active child-exit supervision, restart backoff, stderr excerpts, and repeated-failure `core.alert` emission.
- Extended sensor log download with `tail` support and added a sensor-detail stdout/stderr tail/follow panel.
- Added `core.queue_started` and `core.queue_empty` lifecycle triggers for work queues, emitted by the executor when queues transition into processing and back to empty.
- Consolidated the new schema changes into the existing first-release migrations instead of adding standalone migration files.
- Fixed the operational e2e regressions by rebuilding application images in the e2e runner, granting `core.admin` `workers:manage`, accepting capitalized boolean query values for worker filters, persisting sensor placement through repository create/update, and decoding nullable execution `workflow_task` rows correctly.
- Documented the operational visibility model in `docs/deployment/operational-visibility.md` and updated architecture/project guidance.
- Validated the fixes with `cargo check --workspace --all-targets` and the full Docker e2e suite (`267 passed, 8 skipped`).

## Follow-up

The current UI exposes sensor log tail/follow controls but does not yet include a dedicated sensor-process health/history panel.
