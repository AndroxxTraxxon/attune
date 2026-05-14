# Attune Supervisor

`attune-supervisor` is Attune's platform maintenance service. It runs outside the API, executor, worker, sensor, and notifier hot paths and owns cross-cutting cleanup, retention, monitoring, and guarded remediation work.

## Where it fits

The supervisor is a single-purpose operations service. It should normally run as one replica, but every maintenance cycle is protected by a PostgreSQL advisory lock so accidental multiple replicas skip work instead of racing each other.

The executor still owns normal execution scheduling, workflow advancement, worker timeout reconciliation, and queue dispatch. The supervisor acts as the maintenance and safety-net layer for data that can otherwise grow without bound or remain stale after an abnormal shutdown.

The service connects to:

- PostgreSQL, required for all retention and maintenance work.
- RabbitMQ, optional but recommended. When configured, supervisor corrective actions publish normal execution lifecycle messages so workflows and queues wake up after remediation.
- The artifact filesystem, required when artifact cleanup deletes expired file-backed artifact versions.

## What it does

### Runtime database retention

The supervisor purges runtime metadata according to database-backed retention settings. The effective retention settings are stored in `runtime_retention_config` and `runtime_retention_target_config`, loaded at the start of every cycle, and can be changed without restarting the service.

Use the web UI **Runtime Retention** page (`/retention`) or the API:

- `GET /api/v1/retention-config` requires `retention:read`
- `PUT /api/v1/retention-config` requires `retention:update`

Retention changes are audited as maintenance/admin audit events.

The default database seed enables all targets, runs every 3600 seconds, deletes up to 1000 regular-table rows per target per cycle, and uses dry-run mode `false`.

| Target key | Default max age | Purge behavior |
| --- | ---: | --- |
| `events` | 30 days | Drops old `event` hypertable chunks. |
| `enforcements` | 30 days | Deletes only non-`created` rows older than the cutoff. |
| `executions` | 30 days | Deletes only terminal executions (`completed`, `failed`, `cancelled`, `timeout`, `abandoned`) by `updated`. |
| `execution_history` | 30 days | Drops old `execution_history` hypertable chunks. |
| `worker_history` | 30 days | Drops old `worker_history` hypertable chunks. |
| `sensor_process_history` | 30 days | Drops old `sensor_process_history` hypertable chunks. |
| `audit_events` | 90 days | Drops old `audit_event` hypertable chunks. |
| `continuous_aggregates` | 30 days | Drops old continuous-aggregate materialization chunks. |
| `notifications` | 30 days | Deletes rows older than the cutoff. |
| `webhook_event_logs` | 30 days | Deletes rows older than the cutoff. |
| `inquiries` | 30 days | Deletes only terminal inquiries (`responded`, `timeout`, `cancelled`) by `updated`. |
| `work_queue_items` | 30 days | Deletes only terminal queue items (`completed`, `failed`, `skipped`, `cancelled`) by `updated`. |
| `work_queue_dispatches` | 30 days | Deletes only terminal dispatches (`completed`, `failed`, `released`, `cancelled`) by `updated`. |
| `pack_test_executions` | 30 days | Deletes old pack test execution rows by `execution_time`. |
| `execution_admission` | 30 days | Removes stale execution admission state/entries. |
| `workers` | 30 days | Deletes only stale `inactive`/`error` workers that are not cordoned and do not own active sensor processes. |
| `sensor_processes` | 30 days | Deletes only `stopped`/`failed` processes with `active_rule_count = 0`. |

Set a target's `enabled` field to `false` to skip it. Set `max_age_seconds` to `null` to keep that target forever while still leaving it visible in configuration.

### Artifact cleanup

Artifact version-count retention still happens when artifact versions are inserted. The supervisor handles the complementary cleanup path for artifacts using time-based policies (`days`, `hours`, or `minutes`):

1. Find expired artifact versions.
2. Delete the file-backed bytes when a version has a `file_path`.
3. Delete the `artifact_version` row.
4. Refresh artifact metadata or delete empty artifact metadata rows when no versions/data remain.

This is controlled by `maintenance.artifact_cleanup_enabled` and `maintenance.artifact_cleanup_batch_size`.

### Monitoring and alerts

When `maintenance.monitoring_enabled` is true, the supervisor emits deduplicated `core.alert` events for:

- non-terminal executions that have remained stale beyond `stuck_execution_seconds`
- leased queue items and leased/dispatched queue dispatches stale beyond `stuck_queue_seconds`
- retention lag, where eligible rows remain older than a target's max age plus `retention_lag_alert_seconds`

Alerts include a correlation id and are suppressed for `alert_cooldown_seconds` to avoid alert storms. Each cycle emits at most `alert_limit_per_cycle` monitoring alerts.

### Corrective actions

When `maintenance.corrective_actions_enabled` is true, the supervisor applies guarded remediation for stale runtime state:

- stale `canceling` executions become `cancelled`
- stale `requested`, `scheduling`, `scheduled`, and unavailable-worker `running` executions become `abandoned`
- stale work queue dispatches and leased items are released, retried, failed, or cancelled according to queue state and retry limits
- execution admission entries tied to terminal/stale executions are removed, and queued entries may be promoted when capacity opens
- stale workflow rows are synchronized from terminal parent executions, or failed when all children are terminal and at least one child failed/cancelled/timed out/was abandoned

Corrective mutations emit `core.alert` events and `maintenance.corrective_action.applied` audit events. If RabbitMQ is configured, the supervisor publishes `ExecutionCompleted` or `ExecutionRequested` messages for corrected/promoted executions so downstream workflow and queue handlers observe the change.

## Configuration

### Runtime retention configuration

Retention is database-backed and hot-reloaded each supervisor cycle. The YAML `retention` block documents defaults and provides fallback config shape, but the runtime source of truth is the database once migrations have seeded it.

Example API payload:

```json
{
  "enabled": true,
  "check_interval_seconds": 3600,
  "batch_size": 1000,
  "dry_run": false,
  "advisory_lock_key": 7821001,
  "targets": {
    "events": { "enabled": true, "max_age_seconds": 2592000 },
    "executions": { "enabled": true, "max_age_seconds": 2592000 },
    "audit_events": { "enabled": true, "max_age_seconds": 7776000 }
  }
}
```

| Field | Default | Description |
| --- | ---: | --- |
| `enabled` | `true` | Master switch for runtime retention. Maintenance jobs still use `maintenance.enabled`. |
| `check_interval_seconds` | `3600` | Delay between supervisor cycles. Must be greater than zero. |
| `batch_size` | `1000` | Maximum rows deleted per regular-table target per cycle. Hypertable targets drop chunks instead. |
| `dry_run` | `false` | Counts candidates and emits audit/log output without deleting rows or chunks. |
| `advisory_lock_key` | `7821001` | PostgreSQL advisory lock key used to make multiple supervisors safe. |
| `targets.<target>.enabled` | `true` | Whether a target is processed. |
| `targets.<target>.max_age_seconds` | target default | Maximum retained age. Use `null` to keep forever. Must not be `0`. |

### Maintenance configuration

Maintenance settings are loaded from the normal Attune configuration file and environment variables at supervisor startup. Restart the supervisor after changing these values.

```yaml
maintenance:
  enabled: true
  artifact_cleanup_enabled: true
  artifact_cleanup_batch_size: 100
  monitoring_enabled: true
  corrective_actions_enabled: true
  stuck_execution_seconds: 3600
  execution_remediation_seconds: 7200
  stuck_queue_seconds: 900
  queue_remediation_seconds: 1800
  admission_remediation_seconds: 1800
  retention_lag_alert_seconds: 86400
  alert_limit_per_cycle: 25
  alert_cooldown_seconds: 3600
```

| Field | Default | Description |
| --- | ---: | --- |
| `enabled` | `true` | Master switch for non-retention maintenance jobs. |
| `artifact_cleanup_enabled` | `true` | Enables cleanup of expired time-policy artifact versions. |
| `artifact_cleanup_batch_size` | `100` | Maximum expired artifact versions cleaned per cycle. |
| `monitoring_enabled` | `true` | Enables stuck-state and retention-lag alerting. |
| `corrective_actions_enabled` | `true` | Enables guarded DB remediation for stale executions, queues, workflow rows, and admission entries. |
| `stuck_execution_seconds` | `3600` | Alert threshold for stale non-terminal executions. |
| `execution_remediation_seconds` | `7200` | Remediation threshold for stale executions and workflow state. |
| `stuck_queue_seconds` | `900` | Alert threshold for stale queue leases and dispatches. |
| `queue_remediation_seconds` | `1800` | Remediation threshold for stale queue leases and dispatches. |
| `admission_remediation_seconds` | `1800` | Remediation threshold for stale execution admission entries. |
| `retention_lag_alert_seconds` | `86400` | Grace period beyond a target's retention window before alerting on remaining eligible rows. |
| `alert_limit_per_cycle` | `25` | Maximum monitoring/remediation alerts emitted per cycle. |
| `alert_cooldown_seconds` | `3600` | Duplicate-alert suppression window for the same correlation id. |

### Environment overrides

All YAML fields can be overridden with `ATTUNE__` environment variables. Common supervisor-related examples:

```bash
ATTUNE_CONFIG=/etc/attune/attune.yaml
ATTUNE__DATABASE__URL=postgresql://attune:attune@localhost:5432/attune
ATTUNE__RABBITMQ__URL=amqp://attune:attune@localhost:5672/attune
ATTUNE__MAINTENANCE__CORRECTIVE_ACTIONS_ENABLED=false
ATTUNE__MAINTENANCE__ALERT_COOLDOWN_SECONDS=7200
RUST_LOG=info
```

Use the retention API or UI for runtime retention changes instead of relying on environment variables after the database has been initialized.

## Running the supervisor

### Docker Compose

`docker-compose.yaml` includes a `supervisor` service using `attune-supervisor`. It mounts the same Docker config and artifact volume as the rest of the stack:

```bash
docker compose up -d supervisor
docker compose logs -f supervisor
```

### Local development

```bash
make run-supervisor
```

Or directly:

```bash
cargo run --bin attune-supervisor -- --config config.development.yaml
```

### Linux packages

Package installs include a systemd unit for service packages:

```bash
sudo systemctl enable --now attune-supervisor
sudo journalctl -u attune-supervisor -f
```

Set required secrets and service URLs in `/etc/attune/environment` and `/etc/attune/attune.yaml`.

### Kubernetes

The Helm chart exposes:

```yaml
supervisor:
  replicaCount: 1
  resources: {}
```

Keep `replicaCount: 1` unless you intentionally want advisory-lock-protected standby replicas.

## Operational guidance

- Start with `dry_run: true` when lowering retention windows in an existing environment. Review logs/audit entries, then switch to `false`.
- Keep audit-event retention longer than other runtime targets unless compliance requirements say otherwise.
- Prefer disabling a target or setting `max_age_seconds: null` over setting a very large value when you want to retain data indefinitely.
- Do not run aggressive retention windows in test suites unless the tests are isolated from workflow/retry/concurrency assertions.
- If corrective actions are too aggressive for an environment, set `maintenance.corrective_actions_enabled: false`; monitoring alerts can remain enabled.
- If RabbitMQ is unavailable, the supervisor can still mutate database state, but workflow/queue wakeups from corrective actions will not be published until another component observes the state.
