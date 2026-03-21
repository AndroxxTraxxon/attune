# Attune Project Rules

## Project Overview
Attune is an **event-driven automation and orchestration platform** built in Rust, similar to StackStorm. It enables building complex workflows triggered by events with multi-tenancy, RBAC, and human-in-the-loop capabilities.

## Development Status: Pre-Production

**This project is under active development with no users, deployments, or stable releases.**

### Breaking Changes Policy
- **Breaking changes are explicitly allowed and encouraged** when they improve the architecture, API design, or developer experience
- **No backward compatibility required** - there are no existing versions to support
- **Database migrations can be modified or consolidated** - no production data exists
- **API contracts can change freely** - no external integrations depend on them, only internal interfaces with other services and the web UI must be maintained.
- **Configuration formats can be redesigned** - no existing config files need migration
- **Service interfaces can be refactored** - no live deployments to worry about

When this project reaches v1.0 or gets its first production deployment, this section should be removed and replaced with appropriate stability guarantees and versioning policies.

## Languages & Core Technologies
- **Primary Language**: Rust 2021 edition
- **Database**: PostgreSQL 16+ with TimescaleDB 2.17+ (primary data store + LISTEN/NOTIFY pub/sub + time-series history)
- **Message Queue**: RabbitMQ 3.12+ (via lapin)
- **Cache**: Redis 7.0+ (optional)
- **Web UI**: TypeScript + React 19 + Vite
- **Async Runtime**: Tokio
- **Web Framework**: Axum 0.8
- **ORM**: SQLx (compile-time query checking)

## Project Structure (Cargo Workspace)

```
attune/
├── Cargo.toml                    # Workspace root
├── config.{development,test}.yaml # Environment configs
├── Makefile                      # Common dev tasks
├── crates/                       # Rust services
│   ├── common/                   # Shared library (models, db, repos, mq, config, error, template_resolver)
│   ├── api/                      # REST API service (8080)
│   ├── executor/                 # Execution orchestration service
│   ├── worker/                   # Action execution service (multi-runtime)
│   ├── sensor/                   # Event monitoring service
│   ├── notifier/                 # Real-time notification service
│   └── cli/                      # Command-line interface
├── migrations/                   # SQLx database migrations (19 tables)
├── web/                          # React web UI (Vite + TypeScript)
├── packs/                        # Pack bundles
│   └── core/                     # Core pack (timers, HTTP, etc.)
├── docs/                         # Technical documentation
├── scripts/                      # Helper scripts (DB setup, testing)
└── tests/                        # Integration tests
```

## Service Architecture (Distributed Microservices)

1. **attune-api**: REST API gateway, JWT auth, all client interactions
2. **attune-executor**: Manages execution lifecycle, scheduling, policy enforcement, workflow orchestration
3. **attune-worker**: Executes actions in multiple runtimes (Python/Node.js/containers)
4. **attune-agent**: Universal worker agent — statically-linked (musl) binary injected into any container to auto-detect runtimes and execute actions. Functionally identical to `attune-worker` but packaged for universal deployment. Lives in the same crate (`crates/worker`) as a second binary target (`src/agent_main.rs`). Uses runtime auto-detection (`src/runtime_detect.rs`) instead of `ATTUNE_WORKER_RUNTIMES` manual config. Supports `--detect-only` flag for probing container environments.
5. **attune-sensor**: Monitors triggers, generates events
6. **attune-notifier**: Real-time notifications via PostgreSQL LISTEN/NOTIFY + WebSocket (port 8081)
   - **PostgreSQL listener**: Uses `PgListener::listen_all()` (single batch command) to subscribe to all 11 channels. **Do NOT use individual `listen()` calls in a loop** — this leaves the listener in a broken state where it stops receiving after the last call.
   - **Artifact notifications**: `artifact_created` and `artifact_updated` channels. The `artifact_updated` trigger extracts a progress summary (`progress_percent`, `progress_message`, `progress_entries`) from the last entry in the `data` JSONB array for progress-type artifacts, enabling inline progress bars without extra API calls. The Web UI uses `useArtifactStream` hook to subscribe to `entity_type:artifact` notifications and invalidate React Query caches + push progress summaries to a `artifact_progress` cache key.
   - **WebSocket protocol** (client → server): `{"type":"subscribe","filter":"entity:execution:<id>"}` — filter formats: `all`, `entity_type:<type>`, `entity:<type>:<id>`, `user:<id>`, `notification_type:<type>`
   - **WebSocket protocol** (server → client): All messages use `#[serde(tag="type")]` — `{"type":"welcome","client_id":"...","message":"..."}` on connect; `{"type":"notification","notification_type":"...","entity_type":"...","entity_id":...,"payload":{...},"user_id":null,"timestamp":"..."}` for notifications; `{"type":"error","message":"..."}` for errors
   - **Key invariant**: The outgoing task in `websocket_server.rs` MUST wrap `Notification` in `ClientMessage::Notification(notification)` before serializing — bare `Notification` serialization omits the `"type"` field and breaks clients

**Communication**: Services communicate via RabbitMQ for async operations

## Docker Compose Orchestration

**All Attune services run via Docker Compose.**

- **Compose file**: `docker-compose.yaml` (root directory)
- **Configuration**: `config.docker.yaml` (Docker-specific settings, including `artifacts_dir: /opt/attune/artifacts`)
- **Default user**: `test@attune.local` / `TestPass123!` (auto-created)

**Services**:
- **Infrastructure**: postgres (TimescaleDB), rabbitmq, redis
- **Init** (run-once): migrations, init-user, init-packs, init-agent
- **Application**: api (8080), executor, worker-{shell,python,node,full}, sensor, notifier (8081), web (3000)

**Volumes** (named):
- `postgres_data`, `rabbitmq_data`, `redis_data` — infrastructure state
- `packs_data` — pack files (shared across all services)
- `runtime_envs` — isolated runtime environments (virtualenvs, node_modules)
- `artifacts_data` — file-backed artifact storage (shared between API rw, workers rw, executor ro)
- `agent_bin` — statically-linked `attune-agent` binary (populated by `init-agent`, mounted read-only by agent workers and API for binary download endpoint)
- `*_logs` — per-service log volumes

**Commands**:
```bash
docker compose up -d          # Start all services
docker compose down           # Stop all services
docker compose logs -f <svc>  # View logs
docker compose -f docker-compose.yaml -f docker-compose.agent.yaml up -d  # Start with agent workers
```

**Key environment overrides**: `JWT_SECRET`, `ENCRYPTION_KEY` (required for production)

### Docker Build Optimization
- **Optimized Dockerfiles**: `docker/Dockerfile.optimized`, `docker/Dockerfile.worker.optimized`, `docker/Dockerfile.sensor.optimized`, and `docker/Dockerfile.agent`
- **Agent Dockerfile** (`docker/Dockerfile.agent`): Builds a statically-linked `attune-agent` binary using musl (`x86_64-unknown-linux-musl`). Three stages: `builder` (cross-compile), `agent-binary` (scratch — just the binary), `agent-init` (busybox — for volume population via `cp`). The binary has zero runtime dependencies (no glibc, no libssl). Build with `make docker-build-agent`.
- **Strategy**: Selective crate copying - only copy crates needed for each service (not entire workspace)
- **Performance**: 90% faster incremental builds (~30 sec vs ~5 min for code changes)
- **BuildKit cache mounts**: Persist cargo registry and compilation artifacts between builds
  - **Cache strategy**: `sharing=shared` for registry/git (concurrent-safe), service-specific IDs for target caches
  - **Parallel builds**: 4x faster than old `sharing=locked` strategy - no serialization overhead
- **Rustc stack size**: All Rust Dockerfiles set `ENV RUST_MIN_STACK=67108864` (64 MiB) in the build stage to prevent `rustc` SIGSEGV crashes during release compilation. The `Makefile` also exports this variable for local builds.
- **Documentation**: See `docs/docker-layer-optimization.md`, `docs/QUICKREF-docker-optimization.md`, `docs/QUICKREF-buildkit-cache-strategy.md`

### Docker Runtime Standardization
- **Base image**: All worker and sensor runtime stages use `debian:bookworm-slim` (or `debian:bookworm` for worker-full)
- **Python**: Always installed via `apt-get install python3 python3-pip python3-venv` → binary at `/usr/bin/python3`
- **Node.js**: Always installed via NodeSource apt repo (`setup_${NODE_VERSION}.x`) → binary at `/usr/bin/node`
- **NEVER** use `python:` or `node:` Docker images as base — they install binaries at `/usr/local/bin/` which causes broken venv symlinks when multiple containers share the `runtime_envs` volume
- **UID**: All containers use UID 1000 for the `attune` user
- **Venv creation**: Uses `--copies` flag (`python3 -m venv --copies`) to avoid cross-container broken symlinks
- **Worker targets**: `worker-base` (shell), `worker-python` (shell+python), `worker-node` (shell+node), `worker-full` (all)
- **Sensor targets**: `sensor-base` (native only), `sensor-full` (native+python+node)

### Packs Volume Architecture
- **Key Principle**: Packs are NOT copied into Docker images - they are mounted as volumes
- **Volume Flow**: Host `./packs/` → `init-packs` service → `packs_data` volume → mounted in all services
- **Benefits**: Update packs with restart (~5 sec) instead of rebuild (~5 min)
- **Pack Binaries**: Built separately with `./scripts/build-pack-binaries.sh` (GLIBC compatibility)
- **Development**: Use `./packs.dev/` for instant testing (direct bind mount, no restart needed)
- **Documentation**: See `docs/QUICKREF-packs-volumes.md`

### Runtime Environments Volume
- **Key Principle**: Runtime environments (virtualenvs, node_modules) are stored OUTSIDE pack directories
- **Volume**: `runtime_envs` named volume mounted at `/opt/attune/runtime_envs` in worker, sensor, and API containers
- **Path Pattern**: `{runtime_envs_dir}/{pack_ref}/{runtime_name}` (e.g., `/opt/attune/runtime_envs/python_example/python`)
- **Creation**: Worker creates environments proactively at startup and via `pack.registered` MQ events; lightweight existence check at execution time
- **Broken venv auto-repair**: Worker detects broken interpreter symlinks (e.g., from mismatched container python paths) and automatically recreates the environment
- **API best-effort**: API attempts environment setup during pack registration but logs and defers to worker on failure (Docker API containers lack interpreters)
- **Pack directories remain read-only**: Packs mounted `:ro` in workers; all generated env files go to `runtime_envs` volume
- **Config**: `runtime_envs_dir` setting in config YAML (default: `/opt/attune/runtime_envs`)

## Domain Model & Event Flow

**Critical Event Flow**:
```
Sensor → Trigger fires → Event created → Rule evaluates →
Enforcement created → Execution scheduled → Worker executes Action

For workflows:
Execution requested → Scheduler detects workflow_def → Loads definition →
Creates workflow_execution record → Dispatches entry-point tasks as child executions →
Completion listener advances workflow → Schedules successor tasks → Completes workflow
```

**Key Entities** (all in `public` schema, IDs are `i64`):
- **Pack**: Bundle of automation components (actions, sensors, rules, triggers, runtimes)
- **Runtime**: Unified execution environment definition (Python, Shell, Node.js, etc.) — used by both actions and sensors. Configured via `execution_config` JSONB (interpreter, environment setup, dependency management, env_vars). No type distinction; whether a runtime is executable is determined by its `execution_config` content.
- **RuntimeVersion**: A specific version of a runtime (e.g., Python 3.12.1, Node.js 20.11.0). Each version has its own `execution_config` and `distributions` for version-specific interpreter paths, verification commands, and environment setup. Actions and sensors can declare an optional `runtime_version_constraint` (semver range) to select a compatible version at execution time.
- **Trigger**: Event type definition (e.g., "webhook_received")
- **Sensor**: Monitors for trigger conditions, creates events
- **Event**: Instance of a trigger firing with payload
- **Action**: Executable task with parameters
- **Rule**: Links triggers to actions with conditional logic
- **Enforcement**: Represents a rule activation
- **Execution**: Single action run; supports parent-child relationships for workflows
  - **Workflow Tasks**: Workflow-specific metadata stored in `execution.workflow_task` JSONB field
- **Inquiry**: Human-in-the-loop async interaction (approvals, inputs)
- **Identity**: User/service account with RBAC permissions
- **Key**: Secrets/config storage. The `value` column is JSONB — keys can store strings, objects, arrays, numbers, or booleans. Keys are **unencrypted by default**; use `--encrypt`/`-e` (CLI) or `"encrypted": true` (API) to encrypt. When encrypted, the JSON value is serialised to a compact string, encrypted with AES-256-GCM, and stored as a JSON string; decryption reverses this. The `encrypt_json`/`decrypt_json` helpers in `attune_common::crypto` handle this — **all services use this single shared implementation** (the worker's `SecretManager` delegates directly to `attune_common::crypto::decrypt_json`; it no longer has its own bespoke encryption code). The ciphertext format is `BASE64(nonce_bytes ++ ciphertext_bytes)` everywhere. The worker's `SecretManager` returns `HashMap<String, JsonValue>` and secrets are merged directly into action parameters (no `Value::String` wrapping). The workflow `keystore` namespace already uses `JsonValue`, so structured secrets are natively accessible (e.g., `{{ keystore.db_credentials.password }}`). The CLI `key show` command displays a SHA-256 hash of the value by default; pass `--decrypt`/`-d` to reveal the actual value.
- **Artifact**: Tracked output from executions (files, logs, progress indicators). Metadata + optional structured `data` (JSONB). Linked to execution via plain BIGINT (no FK). Supports retention policies (version-count or time-based). File-type artifacts (FileBinary, FileDataTable, FileImage, FileText) use disk-based storage on a shared volume; Progress and Url artifacts use DB storage. Each artifact has a `visibility` field (`ArtifactVisibility` enum: `public` or `private`, DB default `private`). Public artifacts are viewable by all authenticated users; private artifacts are restricted based on the artifact's `scope` (Identity, Pack, Action, Sensor) and `owner` fields. **Type-aware API default**: when `visibility` is omitted from `POST /api/v1/artifacts`, the API defaults to `public` for Progress artifacts (informational status indicators anyone watching an execution should see) and `private` for all other types. Callers can always override by explicitly setting `visibility`. Full RBAC enforcement is deferred — the column and basic filtering are in place for future permission checks.
- **ArtifactVersion**: Immutable content snapshot for an artifact. File-type versions store a `file_path` (relative path on shared volume) with `content` BYTEA left NULL. DB-stored versions use `content` BYTEA and/or `content_json` JSONB. Version number auto-assigned via `next_artifact_version()`. Retention trigger auto-deletes oldest versions beyond limit. Invariant: exactly one of `content`, `content_json`, or `file_path` should be non-NULL per row.

## Key Tools & Libraries

### Shared Dependencies (workspace-level)
- **Async**: tokio, async-trait, futures
- **Web**: axum, tower, tower-http
- **Database**: sqlx (with postgres, json, chrono, uuid features)
- **Serialization**: serde, serde_json, serde_yaml_ng
- **Version Matching**: semver (with serde feature)
- **Logging**: tracing, tracing-subscriber
- **Error Handling**: anyhow, thiserror
- **Config**: config crate (YAML + env vars)
- **Validation**: validator
- **Auth**: jsonwebtoken, argon2
- **CLI**: clap
- **OpenAPI**: utoipa, utoipa-swagger-ui
- **Message Queue**: lapin (RabbitMQ)
- **HTTP Client**: reqwest
- **Archive/Compression**: tar, flate2 (used for pack upload/extraction)
- **Testing**: mockall, tempfile, serial_test

### Web UI Dependencies
- **Framework**: React 19 + react-router-dom
- **State**: Zustand, @tanstack/react-query
- **HTTP**: axios (with generated OpenAPI client)
- **Styling**: Tailwind CSS
- **Icons**: lucide-react
- **Build**: Vite, TypeScript

## Configuration System
- **Primary**: YAML config files (`config.yaml`, `config.{env}.yaml`)
- **Overrides**: Environment variables with prefix `ATTUNE__` and separator `__`
  - Example: `ATTUNE__DATABASE__URL`, `ATTUNE__SERVER__PORT`, `ATTUNE__RUNTIME_ENVS_DIR`
- **Loading Priority**: Base config → env-specific config → env vars
- **Required for Production**: `JWT_SECRET`, `ENCRYPTION_KEY` (32+ chars)
- **Location**: Root directory or `ATTUNE_CONFIG` env var path
- **Key Settings**:
  - `packs_base_dir` - Where pack files are stored (default: `/opt/attune/packs`)
  - `runtime_envs_dir` - Where isolated runtime environments are created (default: `/opt/attune/runtime_envs`)
  - `artifacts_dir` - Where file-backed artifacts are stored (default: `/opt/attune/artifacts`). Shared volume between API and workers.
  - `agent.binary_dir` - Directory containing agent binary files for download endpoint (default: `/opt/attune/agent`). Optional — only needed if serving agent binaries via `GET /api/v1/agent/binary`.
  - `agent.bootstrap_token` - Optional shared secret for authenticating agent binary downloads. If set, requests must provide it via `X-Agent-Token` header or `token` query parameter.

## Authentication & Security
- **Auth Type**: JWT (access tokens: 1h, refresh tokens: 7d)
- **Password Hashing**: Argon2id
- **Protected Routes**: Use `RequireAuth(user)` extractor in Axum
- **External Identity Providers**: OIDC and LDAP are supported as optional login methods alongside local username/password. Both upsert an `identity` row on first login and store provider-specific claims under `attributes.oidc` or `attributes.ldap` respectively. The web UI login page adapts dynamically based on the `GET /auth/settings` response, showing/hiding each method. The `?auth=<provider_name>` query parameter overrides which method is displayed (e.g., `?auth=direct`, `?auth=sso`, `?auth=ldap`).
  - **OIDC** (`crates/api/src/auth/oidc.rs`): Browser-redirect flow using the `openidconnect` crate. Config: `security.oidc` in YAML. Routes: `GET /auth/oidc/login` (redirect to provider), `GET /auth/callback` (authorization code exchange). Identity matched by `attributes->'oidc'->>'issuer'` + `attributes->'oidc'->>'sub'`. Supports PKCE, ID token verification via JWKS, userinfo endpoint enrichment, and provider-initiated logout via `end_session_endpoint`.
  - **LDAP** (`crates/api/src/auth/ldap.rs`): Server-side bind flow using the `ldap3` crate. Config: `security.ldap` in YAML. Route: `POST /auth/ldap/login` (accepts `{login, password}`, returns `TokenResponse`). Two authentication modes: **direct bind** (construct DN from `bind_dn_template` with `{login}` placeholder) or **search-and-bind** (bind as service account → search `user_search_base` with `user_filter` → re-bind as discovered DN). Identity matched by `attributes->'ldap'->>'server_url'` + `attributes->'ldap'->>'dn'`. Supports STARTTLS, TLS cert skip (`danger_skip_tls_verify`), and configurable attribute mapping (`login_attr`, `email_attr`, `display_name_attr`, `group_attr`).
  - **Login Page Config** (`security.login_page`): `show_local_login`, `show_oidc_login`, `show_ldap_login` — all default to `true`. Controls which methods are visible by default on the web UI login page.
- **Secrets Storage**: AES-GCM encrypted in `key` table (JSONB `value` column) with scoped ownership. Supports structured values (objects, arrays) in addition to plain strings. All encryption/decryption goes through `attune_common::crypto` (`encrypt_json`/`decrypt_json`) — the worker's `SecretManager` no longer has its own crypto implementation, eliminating a prior ciphertext format incompatibility between the API (`BASE64(nonce++ciphertext)`) and the old worker code (`BASE64(nonce):BASE64(ciphertext)`). The worker stores the raw encryption key string and passes it to the shared crypto module, which derives the AES-256 key internally via SHA-256.
- **User Info**: Stored in `identity` table

## Code Conventions & Patterns

### General
- **Error Handling**: Use `attune_common::error::Error` and `Result<T>` type alias
- **Async Everywhere**: All I/O operations use async/await with Tokio
- **Module Structure**: Public API exposed via `mod.rs` with `pub use` re-exports

### Database Layer
- **Schema**: All tables use unqualified names; schema determined by PostgreSQL `search_path`
- **Production**: Always uses `public` schema (configured explicitly in `config.production.yaml`)
- **Tests**: Each test uses isolated schema (e.g., `test_a1b2c3d4`) for true parallel execution
- **Schema Resolution**: PostgreSQL `search_path` mechanism, NO hardcoded schema prefixes in queries
- **Models**: Defined in `common/src/models.rs` with `#[derive(FromRow)]` for SQLx
- **Repositories**: One per entity in `common/src/repositories/`, provides CRUD + specialized queries
- **Pattern**: Services MUST interact with DB only through repository layer (no direct queries)
- **Transactions**: Use SQLx transactions for multi-table operations
- **IDs**: All IDs are `i64` (BIGSERIAL in PostgreSQL)
- **Timestamps**: `created`/`updated` columns auto-managed by DB triggers
- **JSON Fields**: Use `serde_json::Value` for flexible attributes/parameters, including `execution.workflow_task` JSONB
- **Enums**: PostgreSQL enum types mapped with `#[sqlx(type_name = "...")]`
- **Workflow Tasks**: Stored as JSONB in `execution.workflow_task` (consolidated from separate table 2026-01-27)
- **FK ON DELETE Policy**: Historical records (executions) use `ON DELETE SET NULL` so they survive entity deletion while preserving text ref fields (`action_ref`, `trigger_ref`, etc.) for auditing. The `event`, `enforcement`, and `execution` tables are TimescaleDB hypertables, so they **cannot be the target of FK constraints** — `enforcement.event`, `execution.enforcement`, `inquiry.execution`, `workflow_execution.execution`, `execution.parent`, and `execution.original_execution` are plain BIGINT columns (no FK) and may become dangling references if the referenced row is deleted. Pack-owned entities (actions, triggers, sensors, rules, runtimes) use `ON DELETE CASCADE` from pack. Workflow executions cascade-delete with their workflow definition.
- **Event Table (TimescaleDB Hypertable)**: The `event` table is a TimescaleDB hypertable partitioned on `created` (1-day chunks). Events are **immutable after insert** — there is no `updated` column, no update trigger, and no `Update` repository impl. The `Event` model has no `updated` field. Compression is segmented by `trigger_ref` (after 7 days) and retention is 90 days. The `event_volume_hourly` continuous aggregate queries the `event` table directly.
- **Enforcement Table (TimescaleDB Hypertable)**: The `enforcement` table is a TimescaleDB hypertable partitioned on `created` (1-day chunks). Enforcements are updated **exactly once** — the executor sets `status` from `created` to `processed` or `disabled` within ~1 second of creation, well before the 7-day compression window. The `resolved_at` column (nullable `TIMESTAMPTZ`) records when this transition occurred; it is `NULL` while status is `created`. There is no `updated` column. Compression is segmented by `rule_ref` (after 7 days) and retention is 90 days. The `enforcement_volume_hourly` continuous aggregate queries the `enforcement` table directly.
- **Execution Table (TimescaleDB Hypertable)**: The `execution` table is a TimescaleDB hypertable partitioned on `created` (1-day chunks). Executions are updated **~4 times** during their lifecycle (requested → scheduled → running → completed/failed), completing within at most ~1 day — well before the 7-day compression window. The `updated` column and its BEFORE UPDATE trigger are preserved (used by timeout monitor and UI). The `started_at` column (nullable `TIMESTAMPTZ`) records when the worker picked up the execution (status → `running`); it is `NULL` until then. **Duration** in the UI is computed as `updated - started_at` (not `updated - created`) so that queue/scheduling wait time is excluded. Compression is segmented by `action_ref` (after 7 days) and retention is 90 days. The `execution_volume_hourly` continuous aggregate queries the execution hypertable directly. The `execution_history` hypertable (field-level diffs) and its continuous aggregates (`execution_status_hourly`, `execution_throughput_hourly`) are preserved alongside — they serve complementary purposes (change tracking vs. volume monitoring).
- **Entity History Tracking (TimescaleDB)**: Append-only `<table>_history` hypertables track field-level changes to `execution` and `worker` tables. Populated by PostgreSQL `AFTER INSERT OR UPDATE OR DELETE` triggers — no Rust code changes needed for recording. Uses JSONB diff format (`old_values`/`new_values`) with a `changed_fields TEXT[]` column for efficient filtering. Worker heartbeat-only updates are excluded. There are **no `event_history` or `enforcement_history` tables** — events are immutable and enforcements have a single deterministic status transition, so both tables are hypertables themselves. See `docs/plans/timescaledb-entity-history.md` for full design. The execution history trigger tracks: `status`, `result`, `executor`, `workflow_task`, `env_vars`, `started_at`.
- **History Large-Field Guardrails**: The `execution` history trigger stores a compact **digest summary** instead of the full value for the `result` column (which can be arbitrarily large). The digest is produced by the `_jsonb_digest_summary(JSONB)` helper function and has the shape `{"digest": "md5:<hex>", "size": <bytes>, "type": "<jsonb_typeof>"}`. This preserves change-detection semantics while avoiding history table bloat. The full result is always available on the live `execution` row. When adding new large JSONB columns to history triggers, use `_jsonb_digest_summary()` instead of storing the raw value.
- **Nullable FK Fields**: `rule.action` and `rule.trigger` are nullable (`Option<Id>` in Rust) — a rule with NULL action/trigger is non-functional but preserved for traceability. `execution.action`, `execution.parent`, `execution.enforcement`, `execution.started_at`, and `event.source` are also nullable. `enforcement.event` is nullable but has no FK constraint (event is a hypertable). `execution.enforcement` is nullable but has no FK constraint (enforcement is a hypertable). All FK columns on the execution table (`action`, `parent`, `original_execution`, `enforcement`, `executor`, `workflow_def`) have no FK constraints (execution is a hypertable). `inquiry.execution` and `workflow_execution.execution` also have no FK constraints. `enforcement.resolved_at` is nullable — `None` while status is `created`, set when resolved. `execution.started_at` is nullable — `None` until the worker sets status to `running`.
**Table Count**: 21 tables total in the schema (including `runtime_version`, `artifact_version`, 2 `*_history` hypertables, and the `event`, `enforcement`, + `execution` hypertables)
**Migration Count**: 12 migrations (`000001` through `000012`) — see `migrations/` directory
- **Artifact System**: The `artifact` table stores metadata + structured data (progress entries via JSONB `data` column). The `artifact_version` table stores immutable content snapshots — either on disk (via `file_path` column) or in DB (via `content` BYTEA / `content_json` JSONB). Version numbering is auto-assigned via `next_artifact_version()` SQL function. A DB trigger (`enforce_artifact_retention`) auto-deletes oldest versions when count exceeds the artifact's `retention_limit`. `artifact.execution` is a plain BIGINT (no FK — execution is a hypertable). Progress-type artifacts use `artifact.data` (atomic JSON array append); file-type artifacts use `artifact_version` rows with `file_path` set. Binary content is excluded from default queries for performance (`SELECT_COLUMNS` vs `SELECT_COLUMNS_WITH_CONTENT`). **Visibility**: Each artifact has a `visibility` column (`artifact_visibility_enum`: `public` or `private`, DB default `private`). The `CreateArtifactRequest` DTO accepts `visibility` as `Option<ArtifactVisibility>` — when omitted the API route handler applies a **type-aware default**: `public` for Progress artifacts (informational status indicators), `private` for all other types. Callers can always override explicitly. Public artifacts are viewable by all authenticated users; private artifacts are restricted based on the artifact's `scope` (Identity, Pack, Action, Sensor) and `owner` fields. The visibility field is filterable via the search/list API (`?visibility=public`). Full RBAC enforcement is deferred — the column and basic query filtering are in place for future permission checks. **Notifications**: `artifact_created` and `artifact_updated` DB triggers (in migration `000008`) fire PostgreSQL NOTIFY with entity_type `artifact` and include `visibility` in the payload. The `artifact_updated` trigger extracts a progress summary (`progress_percent`, `progress_message`, `progress_entries`) from the last entry of the `data` JSONB array for progress-type artifacts. The Web UI `ExecutionProgressBar` component (`web/src/components/executions/ExecutionProgressBar.tsx`) renders an inline progress bar in the Execution Details card using the `useArtifactStream` hook (`web/src/hooks/useArtifactStream.ts`) for real-time WebSocket updates, with polling fallback via `useExecutionArtifacts`.
- **File-Based Artifact Storage**: File-type artifacts (FileBinary, FileDataTable, FileImage, FileText) use a shared filesystem volume instead of PostgreSQL BYTEA. The `artifact_version.file_path` column stores the relative path from the `artifacts_dir` root (e.g., `mypack/build_log/v1.txt`). Pattern: `{ref_with_dots_as_dirs}/v{version}.{ext}`. The artifact ref (globally unique) is used as the directory key — no execution ID in the path, so artifacts can outlive executions and be shared across them. **Endpoint**: `POST /api/v1/artifacts/{id}/versions/file` allocates a version number and file path without any file content; the execution process writes the file to `$ATTUNE_ARTIFACTS_DIR/{file_path}`. **Download**: `GET /api/v1/artifacts/{id}/download` and version-specific downloads check `file_path` first (read from disk), fall back to DB BYTEA/JSON. **Finalization**: After execution exits, the worker stats all file-backed versions for that execution and updates `size_bytes` on both `artifact_version` and parent `artifact` rows via direct DB access. **Cleanup**: Delete endpoints remove disk files before deleting DB rows; empty parent directories are cleaned up. **Backward compatible**: Existing DB-stored artifacts (`file_path = NULL`) continue to work unchanged.
- **Pack Component Loading Order**: Runtimes → Triggers → Actions (+ workflow definitions) → Sensors (dependency order). Both `PackComponentLoader` (Rust) and `load_core_pack.py` (Python) follow this order. When an action YAML contains a `workflow_file` field, the loader creates/updates the referenced `workflow_definition` record and links it to the action during the Actions phase.

### Workflow Execution Orchestration
- **Detection**: The `ExecutionScheduler` checks `action.workflow_def.is_some()` before dispatching to a worker. Workflow actions are orchestrated by the executor, not sent to workers.
- **Orchestration Flow**: Scheduler loads the `WorkflowDefinition`, builds a `TaskGraph`, creates a `workflow_execution` record, marks the parent execution as Running, builds an initial `WorkflowContext` from execution parameters and workflow vars, then dispatches entry-point tasks as child executions via MQ with rendered inputs.
- **Template Resolution**: Task inputs are rendered through `WorkflowContext.render_json()` before dispatching. Uses the expression engine for full operator/function support inside `{{ }}`. Canonical namespaces: `parameters`, `workflow` (mutable vars), `task` (results), `config` (pack config), `keystore` (secrets), `item`, `index`, `system`. Backward-compat aliases: `vars`/`variables` → `workflow`, `tasks` → `task`, bare names → `workflow` fallback. **Type-preserving**: pure template expressions like `"{{ item }}"` preserve the JSON type (integer `5` stays as `5`, not string `"5"`). Mixed expressions like `"Sleeping for {{ item }} seconds"` remain strings.
- **Function Expressions**: `{{ result() }}` returns the last completed task's result. `{{ result().field.subfield }}` navigates into it. `{{ succeeded() }}`, `{{ failed() }}`, `{{ timed_out() }}` return booleans. These are evaluated by `WorkflowContext.try_evaluate_function_call()`.
- **Publish Directives**: Transition `publish` directives are evaluated when a transition fires. Published variables are persisted to the `workflow_execution.variables` column and available to subsequent tasks via the `workflow` namespace (e.g., `{{ workflow.number_list }}`). Values can be **any JSON-compatible type**: string templates (e.g., `number_list: "{{ result().data.items }}"`), booleans (`validation_passed: true`), numbers (`count: 42`), arrays, objects, or null. The `PublishDirective::Simple` variant stores `HashMap<String, serde_json::Value>`. String values are template-rendered with type preservation (pure `{{ }}` expressions preserve the underlying JSON type); non-string values (booleans, numbers, null) pass through `render_json` unchanged — `true` stays as boolean `true`, not string `"true"`. The `PublishVar` struct in `graph.rs` uses a `value: JsonValue` field (with `#[serde(alias = "expression")]` for backward compat with stored task graphs).
- **Child Task Dispatch**: Each workflow task becomes a child execution with the task's actual action ref (e.g., `core.echo`), `workflow_task` metadata linking it to the `workflow_execution` record, and a parent reference to the workflow execution. Child executions re-enter the normal scheduling pipeline, so nested workflows work recursively.
- **with_items Expansion**: Tasks declaring `with_items: "{{ expr }}"` are expanded into child executions. The expression is resolved via the `WorkflowContext` to produce a JSON array, then each item gets its own child execution with `item`/`index` set on the context and `task_index` in `WorkflowTaskMetadata`. Completion tracking waits for ALL sibling items to finish before marking the task as completed/failed and advancing the workflow.
- **with_items Concurrency Limiting**: ALL child execution records are created in the database up front (with fully-rendered inputs), but only the first `N` are published to the message queue where `N` is the task's `concurrency` value (**default: 1**, i.e. serial execution). The remaining children stay at `Requested` status in the DB. As each item completes, `advance_workflow` counts in-flight siblings (`scheduling`/`scheduled`/`running`), calculates free slots (`concurrency - in_flight`), and calls `publish_pending_with_items_children()` which queries for `Requested`-status siblings ordered by `task_index` and publishes them. The DB `status = 'requested'` query is the authoritative source of undispatched items — no auxiliary state in workflow variables needed. The task is only marked complete when all siblings reach a terminal state. To run all items in parallel, explicitly set `concurrency` to the list length or a suitably large number.
- **Advancement**: The `CompletionListener` detects when a completed execution has `workflow_task` metadata and calls `ExecutionScheduler::advance_workflow()`. The scheduler rebuilds the `WorkflowContext` from persisted `workflow_execution.variables` plus all completed child execution results, sets `last_task_outcome`, evaluates transitions (succeeded/failed/always/timed_out/custom with context-based condition evaluation), processes publish directives, schedules successor tasks with rendered inputs, and completes the workflow when all tasks are done.
- **Transition Evaluation**: `succeeded()`, `failed()`, `timed_out()`, and `always` (no condition) are supported. Custom conditions are evaluated via `WorkflowContext.evaluate_condition()` with fallback to fire-on-success if evaluation fails.
- **Legacy Coordinator**: The prototype `WorkflowCoordinator` in `crates/executor/src/workflow/coordinator.rs` is bypassed — it has hardcoded schema prefixes and is not integrated with the MQ pipeline.

### Pack File Loading & Action Execution
- **Pack Base Directory**: Configured via `packs_base_dir` in config (defaults to `/opt/attune/packs`, development uses `./packs`)
- **Pack Volume Strategy**: Packs are mounted as volumes (NOT copied into Docker images)
  - Host `./packs/` → `packs_data` volume via `init-packs` service → mounted at `/opt/attune/packs` in all services
  - Development packs in `./packs.dev/` are bind-mounted directly for instant updates
- **Pack Binaries**: Native binaries (sensors) built separately with `./scripts/build-pack-binaries.sh`
- **Action Script Resolution**: Worker constructs file paths as `{packs_base_dir}/{pack_ref}/actions/{entrypoint}`
- **Workflow Action YAML (`workflow_file` field)**: An action YAML may include a `workflow_file` field (e.g., `workflow_file: workflows/timeline_demo.yaml`) pointing to a workflow definition file relative to the `actions/` directory. When present, the `PackComponentLoader` reads and parses the referenced workflow YAML, creates/updates a `workflow_definition` record, and links the action to it via `action.workflow_def`. This separates action-level metadata (ref, label, parameters, policies) from the workflow graph (tasks, transitions, variables), and allows **multiple actions to reference the same workflow file** with different parameter schemas or policy configurations. Workflow actions have no `runner_type` (runtime is `None`) — the executor orchestrates child task executions rather than sending to a worker.
  - **Action-linked workflow files omit action-level metadata**: Workflow files referenced via `workflow_file` should contain **only the execution graph**: `version`, `vars`, `tasks`, `output_map`. The `ref`, `label`, `description`, `parameters`, `output`, and `tags` fields are omitted — the action YAML is the single authoritative source for those values. The `WorkflowDefinition` parser accepts empty `ref`/`label` (defaults to `""`), and the loader / registrar fall back to the action YAML (or filename-derived values) when they are missing. Standalone workflow files (in `workflows/`) still carry their own `ref`/`label` since they have no companion action YAML.
- **Workflow File Storage**: The visual workflow builder save endpoints (`POST /api/v1/packs/{pack_ref}/workflow-files` and `PUT /api/v1/workflows/{ref}/file`) write **two files** per workflow:
  1. **Action YAML** at `{packs_base_dir}/{pack_ref}/actions/{name}.yaml` — action-level metadata (`ref`, `label`, `description`, `parameters`, `output`, `tags`, `workflow_file` reference, `enabled`). Built by `build_action_yaml()` in `crates/api/src/routes/workflows.rs`.
  2. **Workflow YAML** at `{packs_base_dir}/{pack_ref}/actions/workflows/{name}.workflow.yaml` — graph-only (`version`, `vars`, `tasks`, `output_map`). The `strip_action_level_fields()` function removes `ref`, `label`, `description`, `parameters`, `output`, and `tags` from the definition before writing.
  Pack-bundled workflows use the same directory layout and are discovered during pack registration when their companion action YAML contains `workflow_file`.
- **Workflow File Discovery (dual-directory scanning)**: The `WorkflowLoader` scans **two** directories when loading workflows for a pack: (1) `{pack_dir}/workflows/` (legacy standalone workflow files), and (2) `{pack_dir}/actions/workflows/` (visual-builder and action-linked workflow files). Files with `.workflow.yaml` suffix have the `.workflow` portion stripped when deriving the workflow name/ref (e.g., `deploy.workflow.yaml` → name `deploy`, ref `pack.deploy`). If the same ref appears in both directories, `actions/workflows/` wins. The `reload_workflow` method searches `actions/workflows/` first, trying `.workflow.yaml`, `.yaml`, `.workflow.yml`, and `.yml` extensions.
- **Task Model (Orquesta-aligned)**: Tasks are purely action invocations — there is no task `type` field or task-level `when` condition in the UI model. Parallelism is implicit (multiple `do` targets in a transition fan out into parallel branches). Conditions belong exclusively on transitions (`next[].when`). Each task has: `name`, `action`, `input`, `next` (transitions), `delay`, `retry`, `timeout`, `with_items`, `batch_size`, `concurrency`, `join`.
  - The backend `Task` struct (`crates/common/src/workflow/parser.rs`) still supports `type` and task-level `when` for backward compatibility, but the UI never sets them.
- **Task Transition Model (Orquesta-style)**: Tasks use an ordered `next` array of transitions instead of flat `on_success`/`on_failure`/`on_complete`/`on_timeout` fields. Each transition has:
  - `when` — condition expression (e.g., `{{ succeeded() }}`, `{{ failed() }}`, `{{ timed_out() }}`, or custom). Omit for unconditional.
  - `publish` — key-value pairs to publish into the workflow context (e.g., `- result: "{{ result() }}"`)
  - `do` — list of next task names to invoke when the condition is met
  - `label` — optional custom display label (overrides auto-derived label from `when` expression)
  - `color` — optional custom CSS color for the transition edge (e.g., `"#ff6600"`)
  - `edge_waypoints` — optional `Record<string, NodePosition[]>` of intermediate routing points per target task name (chart-only, stored in `__chart_meta__`)
  - `label_positions` — optional `Record<string, NodePosition>` of custom label positions per target task name (chart-only, stored in `__chart_meta__`)
  - **Example YAML**:
    ```
    next:
      - when: "{{ succeeded() }}"
        label: "main path"
        color: "#22c55e"
        publish:
          - msg: "task done"
        do:
          - log
          - next_task
      - when: "{{ failed() }}"
        do:
          - error_handler
    ```
  - **Legacy format support**: The parser (`crates/common/src/workflow/parser.rs`) auto-converts legacy `on_success`/`on_failure`/`on_complete`/`on_timeout`/`decision` fields into `next` transitions during parsing. The canonical internal representation always uses `next`.
  - **Frontend types**: `TaskTransition` in `web/src/types/workflow.ts` (includes `edge_waypoints`, `label_positions` for visual routing); `TransitionPreset` ("succeeded" | "failed" | "always") for quick-access drag handles; `WorkflowEdge` includes per-edge `waypoints` and `labelPosition` derived from the transition; `SelectedEdgeInfo` and `EdgeHoverInfo` (includes `targetTaskId`) in `WorkflowEdges.tsx`
  - **Backend types**: `TaskTransition` in `crates/common/src/workflow/parser.rs`; `GraphTransition` in `crates/executor/src/workflow/graph.rs`
  - **NOT this** (legacy format): `on_success: task2` / `on_failure: error_handler` — still parsed for backward compat but normalized to `next`
- **Runtime YAML Loading**: Pack registration reads `runtimes/*.yaml` files and inserts them into the `runtime` table. Runtime refs use format `{pack_ref}.{name}` (e.g., `core.python`, `core.shell`). If the YAML includes a `versions` array, each entry is inserted into the `runtime_version` table with its own `execution_config`, `distributions`, and optional `is_default` flag.
- **Runtime Version Constraints**: Actions and sensors can declare `runtime_version: ">=3.12"` (or any semver constraint like `~3.12`, `^3.12`, `>=3.12,<4.0`) in their YAML. This is stored in the `runtime_version_constraint` column. At execution time the worker can select the highest available version satisfying the constraint. A bare version like `"3.12"` is treated as tilde (`~3.12` → >=3.12.0, <3.13.0).
- **Version Matching Module**: `crates/common/src/version_matching.rs` provides `parse_version()` (lenient semver parsing), `parse_constraint()`, `matches_constraint()`, `select_best_version()`, and `extract_version_components()`. Uses the `semver` crate internally.
- **Runtime Version Table**: `runtime_version` stores version-specific execution configs per runtime. Each row has: `runtime` (FK), `version` (string), `version_major/minor/patch` (ints for range queries), `execution_config` (complete, not a diff), `distributions` (verification metadata), `is_default`, `available`, `verified_at`, `meta`. Unique on `(runtime, version)`.
- **Runtime Selection**: Determined by action's runtime field (e.g., "Shell", "Python") - compared case-insensitively; when an explicit `runtime_name` is set in execution context, it is authoritative (no fallback to extension matching). When the action also declares a `runtime_version_constraint`, the executor queries `runtime_version` rows, calls `select_best_version()`, and passes the selected version's `execution_config` as an override through `ExecutionContext.runtime_config_override`. The `ProcessRuntime` uses this override instead of its built-in config.
- **Worker Runtime Loading**: Worker loads all runtimes from DB that have a non-empty `execution_config` (i.e., runtimes with an interpreter configured). Native runtimes (e.g., `core.native` with empty config) are automatically skipped since they execute binaries directly.
- **Worker Startup Sequence**: (1) Connect to DB and MQ, (2) Load runtimes from DB → create `ProcessRuntime` instances, (3) Register worker and set up MQ infrastructure, (4) **Verify runtime versions** — run verification commands from `distributions` JSONB for each `RuntimeVersion` row and update `available` flag (`crates/worker/src/version_verify.rs`), (5) **Set up runtime environments** — create per-version environments for packs, (6) Start heartbeat, execution consumer, and pack registration consumer.
- **Agent Startup Sequence** (`attune-agent`): (0) **Auto-detect runtimes** — probes the container for interpreter binaries using `runtime_detect::detect_runtimes()`, sets `ATTUNE_WORKER_RUNTIMES` env var with discovered names, (0b) **Dynamic runtime registration** — calls `auto_register_detected_runtimes()` to ensure each detected runtime has a DB entry (from template or minimal), then (1–6) follows the same startup sequence as `attune-worker`. If `ATTUNE_WORKER_RUNTIMES` is already set, auto-detection is skipped (explicit override). The `--detect-only` flag runs detection, prints a report, and exits without starting the worker.
- **Agent Runtime Auto-Detection** (`crates/worker/src/runtime_detect.rs`): Database-free runtime discovery for the agent. Probes 8 interpreter families in order: shell (`bash`/`sh`), python (`python3`/`python`), node (`node`/`nodejs`), ruby, go, java, r (`Rscript`), perl. Uses `which`-style PATH lookup with fallbacks for absolute paths (`/bin/bash`, `/bin/sh`) and `command -v`. Captures version strings via interpreter-specific version commands. Returns `Vec<DetectedRuntime>` with name, path, and optional version. The `format_as_env_value()` helper converts to comma-separated format for `ATTUNE_WORKER_RUNTIMES`.
- **Dynamic Runtime Registration** (`crates/worker/src/dynamic_runtime.rs`): When the agent detects a runtime that has no corresponding entry in the database, `auto_register_detected_runtimes()` auto-registers it before `WorkerService::new()`. Strategy: (1) look up by normalized name — if found, skip; (2) look for a template runtime in loaded packs (e.g., `core.ruby`) — if found, clone with `auto_detected = true` and the detected binary path substituted into the execution config; (3) if no template, create a minimal runtime with just the interpreter binary and file extension. Auto-registered runtimes use ref format `auto.<name>` (e.g., `auto.ruby`). The `Runtime` model has `auto_detected: bool` and `detection_config: JsonDict` columns (migration `000012`). The `detection_config` JSONB stores `detected_path`, `detected_name`, and optional `detected_version`.
- **Runtime Name Normalization**: The `ATTUNE_WORKER_RUNTIMES` filter (e.g., `shell,node`) uses alias-aware matching via `normalize_runtime_name()` in `crates/common/src/runtime_detection.rs`. This ensures that filter value `"node"` matches DB runtime name `"Node.js"` (lowercased to `"node.js"`). Alias groups: `node`/`nodejs`/`node.js` → `node`, `python`/`python3` → `python`, `shell`/`bash`/`sh` → `shell`, `native`/`builtin`/`standalone` → `native`, `ruby`/`rb` → `ruby`, `go`/`golang` → `go`, `java`/`jdk`/`openjdk` → `java`, `perl`/`perl5` → `perl`, `r`/`rscript` → `r`. Used in worker service runtime loading and environment setup.
- **Runtime Execution Environment Variables**: `RuntimeExecutionConfig.env_vars` (HashMap<String, String>) specifies template-based environment variables injected during action execution. Example: `{"NODE_PATH": "{env_dir}/node_modules"}` ensures Node.js finds packages in the isolated environment. Template variables (`{env_dir}`, `{pack_dir}`, `{interpreter}`, `{manifest_path}`) are resolved at execution time by `ProcessRuntime::execute`.
- **Native Runtime Detection**: Runtime detection is purely data-driven via `execution_config` in the runtime table. A runtime with empty `execution_config` (or empty `interpreter.binary`) is native — the entrypoint is executed directly without an interpreter. There is no special "builtin" runtime concept.
- **Sensor Runtime Assignment**: Sensors declare their `runner_type` in YAML (e.g., `python`, `native`). The pack loader resolves this to the correct runtime from the database. Default is `native` (compiled binary, no interpreter). Legacy values `standalone` and `builtin` map to `core.native`.
- **Runtime Environment Setup**: Worker creates isolated environments (virtualenvs, node_modules) proactively at startup and via `pack.registered` MQ events at `{runtime_envs_dir}/{pack_ref}/{runtime_name}`; setup is idempotent. Environment `create_command` and dependency `install_command` templates MUST use `{env_dir}` (not `{pack_dir}`) since pack directories are mounted read-only in Docker. For Node.js, `create_command` copies `package.json` to `{env_dir}` and `install_command` uses `npm install --prefix {env_dir}`.
- **Per-Version Environment Isolation**: When runtime versions are registered, the worker creates per-version environments at `{runtime_envs_dir}/{pack_ref}/{runtime_name}-{version}` (e.g., `python-3.12`). This ensures different versions maintain isolated environments with their own interpreter binaries and installed dependencies. A base (unversioned) environment is also created for backward compatibility. The `ExecutionContext.runtime_env_dir_suffix` field controls which env dir the `ProcessRuntime` uses at execution time.
- **Runtime Version Verification**: At worker startup, `version_verify::verify_all_runtime_versions()` runs each version's verification commands (from `distributions.verification.commands` JSONB) and updates the `available` and `verified_at` columns in the database. Only versions marked `available = true` are considered by `select_best_version()`. Verification respects the `ATTUNE_WORKER_RUNTIMES` filter.
- **Schema Format (Unified)**: ALL schemas (`param_schema`, `out_schema`, `conf_schema`) use the same flat format with `required` and `secret` inlined per-parameter (NOT standard JSON Schema). Stored as JSONB columns.
  - **Example YAML**: `parameters:\n  url:\n    type: string\n    required: true\n  token:\n    type: string\n    secret: true`
  - **Stored JSON**: `{"url": {"type": "string", "required": true}, "token": {"type": "string", "secret": true}}`
  - **NOT this** (legacy JSON Schema): `{"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}`
  - **Web UI**: `extractProperties()` in `ParamSchemaForm.tsx` is the single extraction function for all schema types. Only handles flat format.
  - **SchemaBuilder**: Visual schema editor reads and writes flat format with `required` and `secret` checkboxes per parameter.
  - **Backend Validation**: `flat_to_json_schema()` in `crates/api/src/validation/params.rs` converts flat format to JSON Schema internally for `jsonschema` crate validation. This conversion is an implementation detail — external interfaces always use flat format.
- **Execution Config Format (Flat)**: The `execution.config` JSONB column always stores parameters in **flat format** — the object itself IS the parameters map (e.g., `{"url": "https://...", "method": "GET"}`). This is consistent across all execution sources: manual API calls, rule-triggered enforcements, and workflow task children. There is **no `{"parameters": {...}}` wrapper** — never nest parameters under a `"parameters"` key. The worker reads `config` as a flat object and passes each key-value pair as an action parameter. The scheduler's `extract_workflow_params()` helper treats the config object directly as the parameters map.
- **Parameter Delivery**: Actions receive parameters via stdin as JSON (never environment variables)
- **Output Format**: Actions declare output format (text/json/yaml) - json/yaml are parsed into execution.result JSONB
- **Standard Environment Variables**: Worker provides execution context via `ATTUNE_*` environment variables:
  - `ATTUNE_ACTION` - Action ref (always present)
  - `ATTUNE_EXEC_ID` - Execution database ID (always present)
  - `ATTUNE_API_TOKEN` - Execution-scoped API token (always present)
  - `ATTUNE_API_URL` - API base URL (always present)
  - `ATTUNE_ARTIFACTS_DIR` - Absolute path to shared artifact volume (always present, e.g., `/opt/attune/artifacts`)
  - `ATTUNE_RULE` - Rule ref (if triggered by rule)
  - `ATTUNE_TRIGGER` - Trigger ref (if triggered by event/trigger)
- **Custom Environment Variables**: Optional, set via `execution.env_vars` JSONB field (for debug flags, runtime config only)

### API Service (`crates/api`)
- **Structure**: `routes/` (endpoints) + `dto/` (request/response) + `auth/` + `middleware/`
- **Responses**: Standardized `ApiResponse<T>` wrapper with `data` field
- **Protected Routes**: Apply `RequireAuth` middleware
- **OpenAPI**: Documented with `utoipa` attributes (`#[utoipa::path]`)
- **Error Handling**: Custom `ApiError` type with proper HTTP status codes
- **Available at**: `http://localhost:8080` (dev), `/api-spec/openapi.json` for spec

### Common Library (`crates/common`)
- **Modules**: `models`, `repositories`, `db`, `config`, `error`, `mq`, `crypto`, `utils`, `workflow` (includes `expression` sub-module), `pack_registry`, `template_resolver`, `version_matching`, `runtime_detection`
- **Exports**: Commonly used types re-exported from `lib.rs`
- **Repository Layer**: All DB access goes through repositories in `repositories/`
- **Message Queue**: Abstractions in `mq/` for RabbitMQ communication
- **Template Resolver**: Resolves `{{ }}` template variables in rule `action_params` during enforcement creation. Re-exported from `attune_common::{TemplateContext, resolve_templates}`.

### Template Variable Syntax
Rule `action_params` support Jinja2-style `{{ source.path }}` templates resolved at enforcement creation time:

| Namespace | Example | Description |
|-----------|---------|-------------|
| `event.payload.*` | `{{ event.payload.service }}` | Event payload fields |
| `event.id` | `{{ event.id }}` | Event database ID |
| `event.trigger` | `{{ event.trigger }}` | Trigger ref that generated the event |
| `event.created` | `{{ event.created }}` | Event creation timestamp (RFC 3339) |
| `pack.config.*` | `{{ pack.config.api_token }}` | Pack configuration values |
| `system.*` | `{{ system.timestamp }}` | System variables (timestamp, rule info) |

- **Implementation**: `crates/common/src/template_resolver.rs` (also re-exported from `attune_sensor::template_resolver`)
- **Integration**: `crates/executor/src/event_processor.rs` calls `resolve_templates()` in `create_enforcement()`
- **IMPORTANT**: The old `trigger.payload.*` syntax was renamed to `event.payload.*` — the payload data comes from the Event, not the Trigger

### Workflow Expression Engine
Workflow templates (`{{ expr }}`) support a full expression language for evaluating conditions, computing values, and transforming data. The engine is in `crates/common/src/workflow/expression/` (tokenizer → parser → evaluator) and is integrated into `WorkflowContext` via the `EvalContext` trait.

**Canonical Namespaces** — all data inside `{{ }}` expressions is organised into well-defined, non-overlapping namespaces:

| Namespace | Example | Description |
|-----------|---------|-------------|
| `parameters` | `{{ parameters.url }}` | Immutable workflow input parameters |
| `workflow` | `{{ workflow.counter }}` | Mutable workflow-scoped variables (set via `publish`) |
| `task` | `{{ task.fetch.result.data }}` | Completed task results keyed by task name |
| `config` | `{{ config.api_token }}` | Pack configuration values (read-only) |
| `keystore` | `{{ keystore.secret_key }}` | Encrypted secrets from the key store (read-only). Values are `JsonValue` — strings, objects, arrays, etc. Access nested fields with dot notation: `{{ keystore.db_credentials.password }}` |
| `item` | `{{ item }}` / `{{ item.name }}` | Current element in a `with_items` loop |
| `index` | `{{ index }}` | Zero-based iteration index in a `with_items` loop |
| `system` | `{{ system.workflow_start }}` | System-provided variables |

Backward-compatible aliases (kept for existing workflow definitions):
- `vars` / `variables` → same as `workflow`
- `tasks` → same as `task`
- Bare variable names (e.g. `{{ my_var }}`) resolve against the `workflow` variable store as a last-resort fallback.

**IMPORTANT**: New workflow definitions should always use the canonical namespace names. The `config` and `keystore` namespaces are populated by the scheduler from the pack's `config` JSONB column and decrypted `key` table entries (JSONB values) respectively. If not populated, they resolve to `null`. Keystore values preserve their JSON type — a key storing `{"host":"db.example.com","port":5432}` is accessible as `{{ keystore.db_config.host }}` and `{{ keystore.db_config.port }}` (the latter resolves to integer `5432`, not string `"5432"`).

**Operators** (lowest to highest precedence):
1. `or` — logical OR (short-circuit)
2. `and` — logical AND (short-circuit)
3. `not` — logical NOT (unary)
4. `==`, `!=`, `<`, `>`, `<=`, `>=`, `in` — comparison & membership
5. `+`, `-` — addition/subtraction (also string/array concatenation for `+`)
6. `*`, `/`, `%` — multiplication, division, modulo
7. Unary `-` — negation
8. `.field`, `[index]`, `(args)` — postfix access & function calls

**Type Rules**:
- **No implicit type coercion**: `"3" == 3` → `false`, `"hello" + 5` → error
- **Int/float cross-comparison allowed**: `3 == 3.0` → `true`
- **Integer preservation**: `2 + 3` → `5` (int), `2 + 1.5` → `3.5` (float), `10 / 4` → `2.5` (float), `10 / 5` → `2` (int)
- **Python-like truthiness**: `null`, `false`, `0`, `""`, `[]`, `{}` are falsy
- **Deep equality**: `==`/`!=` recursively compare objects and arrays
- **Negative indexing**: `arr[-1]` returns last element

**Built-in Functions**:
- Type conversion: `string(v)`, `number(v)`, `int(v)`, `bool(v)`
- Introspection: `type_of(v)`, `length(v)`, `keys(obj)`, `values(obj)`
- Math: `abs(n)`, `floor(n)`, `ceil(n)`, `round(n)`, `min(a,b)`, `max(a,b)`, `sum(arr)`
- String: `lower(s)`, `upper(s)`, `trim(s)`, `split(s, sep)`, `join(arr, sep)`, `replace(s, old, new)`, `starts_with(s, prefix)`, `ends_with(s, suffix)`, `match(pattern, s)` (regex)
- Collection: `contains(haystack, needle)`, `reversed(v)`, `sort(arr)`, `unique(arr)`, `flat(arr)`, `zip(a, b)`, `range(n)` / `range(start, end)`, `slice(v, start, end)`, `index_of(haystack, needle)`, `count(haystack, needle)`, `merge(obj_a, obj_b)`, `chunks(arr, size)`
- Workflow: `result()`, `succeeded()`, `failed()`, `timed_out()` (resolved via `EvalContext` trait)

**Usage in Conditions** (`when:` on transitions):
```
when: "succeeded() and result().code == 200"
when: "length(workflow.items) > 3 and \"admin\" in workflow.roles"
when: "not failed()"
when: "result().status == \"ok\" or result().status == \"accepted\""
when: "config.retries > 0"
```

**Usage in Templates** (`{{ expr }}`):
```
input:
  count: "{{ length(workflow.items) }}"
  greeting: "{{ parameters.first + \" \" + parameters.last }}"
  doubled: "{{ parameters.x * 2 }}"
  names: "{{ join(sort(keys(workflow.data)), \", \") }}"
  auth: "Bearer {{ keystore.api_key }}"
  endpoint: "{{ config.base_url + \"/api/v1\" }}"
  prev_output: "{{ task.fetch.result.data.id }}"
```

**Implementation Files**:
- `crates/common/src/workflow/expression/mod.rs` — module entry point, `eval_expression()`, `parse_expression()`
- `crates/common/src/workflow/expression/tokenizer.rs` — lexer
- `crates/common/src/workflow/expression/parser.rs` — recursive-descent parser
- `crates/common/src/workflow/expression/evaluator.rs` — AST evaluator, `EvalContext` trait, built-in functions
- `crates/common/src/workflow/expression/ast.rs` — AST node types (`Expr`, `BinaryOp`, `UnaryOp`)
- `crates/executor/src/workflow/context.rs` — `WorkflowContext` implements `EvalContext`

### Web UI (`web/`)
- **Generated Client**: OpenAPI client auto-generated from API spec
  - Run: `npm run generate:api` (requires API running on :8080)
  - Location: `src/api/`
- **State Management**: Zustand for global state, TanStack Query for server state
- **Styling**: Tailwind utility classes
- **Dev Server**: `npm run dev` (typically :3000 or :5173)
- **Build**: `npm run build`
- **Workflow Timeline DAG**: Prefect-style workflow run timeline visualization on the execution detail page for workflow executions
  - Components in `web/src/components/executions/workflow-timeline/` (WorkflowTimelineDAG, TimelineRenderer, types, data, layout)
  - Pure SVG renderer — no D3, no React Flow, no additional npm dependencies
  - Renders child task executions as horizontal duration bars on a time axis with curved Bezier dependency edges
  - **Data flow**: `WorkflowTimelineDAG` (orchestrator) fetches child executions via `useChildExecutions` + workflow definition via `useWorkflow(actionRef)` → `data.ts` transforms into `TimelineTask[]`/`TimelineEdge[]`/`TimelineMilestone[]` → `layout.ts` computes lane assignments + positions → `TimelineRenderer` renders SVG
  - **Edge coloring from workflow metadata**: Fetches the workflow definition's `next` transition array, classifies `when` expressions (`{{ succeeded() }}` → green, `{{ failed() }}` → red dashed, `{{ timed_out() }}` → orange dash-dot, unconditional → gray), and reads `__chart_meta__` custom labels/colors
  - **Task bars**: Colored by state (green=completed, blue=running with pulse animation, red=failed, gray=pending, orange=timeout). Left accent bar, text label with ellipsis clipping, timeout indicator badge.
  - **Milestones**: Synthetic start/end diamond nodes + merge/fork junctions when fan-in/fan-out exceeds 3 tasks
  - **Lane packing**: Greedy algorithm assigns tasks to non-overlapping y-lanes sorted by start time, with optional reordering to cluster tasks sharing upstream dependencies
  - **Interactions**: Hover tooltip (name, state, times, duration, retries, upstream/downstream counts), click-to-select with BFS path highlighting, double-click to navigate to child execution, horizontal zoom (mouse wheel anchored to cursor), alt+drag pan, expand/compact toggle
  - **Fallback**: When no workflow definition is available, infers dependency edges from task timing heuristics
  - **Integration**: Rendered in `ExecutionDetailPage.tsx` above `WorkflowTasksPanel`, conditioned on `isWorkflow`. Shares TanStack Query cache with WorkflowTasksPanel. Accepts `ParentExecutionInfo` interface (satisfied by both `ExecutionResponse` and `ExecutionSummary`).
- **Workflow Builder**: Visual node-based workflow editor at `/actions/workflows/new` and `/actions/workflows/:ref/edit`
  - Components in `web/src/components/workflows/` (ActionPalette, WorkflowCanvas, TaskNode, WorkflowEdges, TaskInspector)
  - Types and conversion utilities in `web/src/types/workflow.ts`
  - Hooks in `web/src/hooks/useWorkflows.ts`
  - Saves workflow files to `{packs_base_dir}/{pack_ref}/actions/workflows/{name}.workflow.yaml` via dedicated API endpoints
  - **Visual / Raw YAML toggle**: Toolbar has a segmented toggle to switch between the visual node-based builder and a two-panel read-only YAML preview (generated via `js-yaml`). Raw YAML mode replaces the canvas, palette, and inspector with side-by-side panels: **Action YAML** (left, blue — `actions/{name}.yaml`: ref, label, parameters, output, tags, `workflow_file` reference) and **Workflow YAML** (right, green — `actions/workflows/{name}.workflow.yaml`: version, vars, tasks, output_map — graph only). Each panel has its own copy button and a description bar explaining the file's role. The `builderStateToGraph()` function extracts the graph-only definition, and `builderStateToActionYaml()` extracts the action metadata.
  - **Drag-handle connections**: TaskNode has output handles (green=succeeded, red=failed, gray=always) and an input handle (top). Drag from an output handle to another node's input handle to create a transition.
  - **Transition customization**: Users can rename transitions (custom `label`) and assign custom colors (CSS color string or preset swatches) via the TaskInspector. Custom colors/labels are persisted in the workflow YAML and rendered on the canvas edges.
  - **Edge waypoints & label dragging**: Transition edges support intermediate waypoints for custom routing. Click an edge to select it, then:
    - Drag existing waypoint handles (colored circles) to reposition the edge path
    - Hover near the midpoint of any edge segment to reveal a "+" handle; click or drag it to insert a new waypoint
    - Drag the transition label to reposition it independently of the edge path
    - Double-click a waypoint to remove it; double-click a label to reset its position
    - Waypoints and label positions are stored per-edge (keyed by target task name) in `TaskTransition.edge_waypoints` and `TaskTransition.label_positions`, serialized via `__chart_meta__` in the workflow YAML
    - Edge selection state (`SelectedEdgeInfo`) is managed in `WorkflowCanvas`; only the selected edge shows interactive handles
    - Multi-segment paths use Catmull-Rom → cubic Bezier conversion for smooth curves through waypoints (`buildSmoothPath` in `WorkflowEdges.tsx`)
  - **Orquesta-style `next` transitions**: Tasks use a `next: TaskTransition[]` array instead of flat `on_success`/`on_failure` fields. Each transition has `when` (condition), `publish` (variables), `do` (target tasks), plus optional `label`, `color`, `edge_waypoints`, and `label_positions`. See "Task Transition Model" above.
  - **No task type or task-level condition**: The UI does not expose task `type` or task-level `when` — all tasks are actions (workflows are also actions), and conditions belong on transitions. Parallelism is implicit via multiple `do` targets.
  - **Ref immutability**: When editing an existing workflow, the pack selector and workflow name fields are disabled — the ref cannot be changed after creation.

## Development Workflow

### Common Commands (Makefile)
```bash
make build              # Build all services
make build-release      # Release build
make test               # Run all tests
make test-integration   # Run integration tests
make fmt                # Format code
make clippy             # Run linter
make lint               # fmt + clippy

make run-api            # Run API service
make run-executor       # Run executor service
make run-worker         # Run worker service
make run-agent          # Run universal worker agent
make run-sensor         # Run sensor service
make run-notifier       # Run notifier service

make db-create          # Create database
make db-migrate         # Run migrations
make db-reset           # Drop & recreate DB
```

### Database Operations
- **Migrations**: Located in `migrations/`, applied via `sqlx migrate run`
- **Test DB**: Separate `attune_test` database, setup with `make db-test-setup`
- **Schema**: All tables in `public` schema with auto-updating timestamps
- **Core Pack**: Load with `./scripts/load-core-pack.sh` after DB setup

### Testing
- **Architecture**: Schema-per-test isolation (each test gets unique `test_<uuid>` schema)
- **Parallel Execution**: Tests run concurrently without `#[serial]` constraints (4-8x faster)
- **Unit Tests**: In module files alongside code
- **Integration Tests**: In `tests/` directory
- **Test DB Required**: Use `make db-test-setup` before integration tests
- **Run**: `cargo test` or `make test` (parallel by default)
- **Verbose**: `cargo test -- --nocapture --test-threads=1`
- **Cleanup**: Schemas auto-dropped on test completion; orphaned schemas cleaned via `./scripts/cleanup-test-schemas.sh`
- **SQLx Offline Mode**: Enabled for compile-time query checking without live DB; regenerate with `cargo sqlx prepare`

### CLI Tool
```bash
cargo install --path crates/cli  # Install CLI
attune auth login                # Login
attune pack list                 # List packs
attune pack create --ref my_pack # Create empty pack (non-interactive)
attune pack create -i            # Create empty pack (interactive prompts)
attune pack upload ./path/to/pack  # Upload local pack to API (works with Docker)
attune pack register /opt/attune/packs/mypak  # Register from API-visible path
attune action execute <ref> --param key=value
attune execution list            # Monitor executions
attune key list                  # List all keys (values redacted)
attune key list --owner-type pack  # Filter keys by owner type
attune key show my_token         # Show key details (value shown as SHA-256 hash)
attune key show my_token -d      # Show key details with decrypted/actual value
attune key create --ref my_token --name "My Token" --value "secret123"  # Create unencrypted string key (default)
attune key create --ref my_token --name "My Token" --value '{"user":"admin","pass":"s3cret"}' # Create unencrypted structured key
attune key create --ref my_token --name "My Token" --value "secret123" -e  # Create encrypted string key
attune key create --ref my_token --name "My Token" --value "secret123" --encrypt --owner-type pack --owner-pack-ref core  # Create encrypted pack-scoped key
attune key update my_token --value "new_secret"  # Update key value (string)
attune key update my_token --value '{"host":"db.example.com","port":5432}'  # Update key value (structured)
attune key update my_token --name "Renamed Token"  # Update key name
attune key delete my_token       # Delete a key (with confirmation)
attune key delete my_token --yes # Delete without confirmation
attune workflow upload actions/deploy.yaml  # Upload workflow action to existing pack
attune workflow upload actions/deploy.yaml --force  # Update existing workflow
attune workflow list             # List all workflows
attune workflow list --pack core # List workflows in a pack
attune workflow show core.install_packs  # Show workflow details + task summary
attune workflow delete core.my_workflow --yes  # Delete a workflow
attune artifact list                 # List all artifacts
attune artifact list --type file_text --visibility public  # Filter artifacts
attune artifact list --execution 42  # List artifacts for an execution
attune artifact show 1               # Show artifact by ID
attune artifact show mypack.build_log  # Show artifact by ref
attune artifact create --ref mypack.build_log --scope action --owner mypack.deploy --type file_text --name "Build Log"
attune artifact upload 1 ./output.log  # Upload file as new version
attune artifact upload 1 ./data.json --content-type application/json --created-by "cli"
attune artifact download 1           # Download latest version to auto-named file
attune artifact download 1 -V 3     # Download specific version
attune artifact download 1 -o ./local.txt  # Download to specific path
attune artifact download 1 -o -     # Download to stdout
attune artifact delete 1             # Delete artifact (with confirmation)
attune artifact delete 1 --yes       # Delete without confirmation
attune artifact version list 1       # List all versions of artifact 1
attune artifact version show 1 3     # Show details of version 3
attune artifact version upload 1 ./new-file.txt  # Upload file as new version
attune artifact version create-json 1 '{"key":"value"}'  # Create JSON version
attune artifact version download 1 2 -o ./v2.txt  # Download version 2
attune artifact version delete 1 2 --yes  # Delete version 2
```

**Pack Upload vs Register**:
- `attune pack upload <local-path>` — Tarballs the local directory and POSTs it to `POST /api/v1/packs/upload`. Works regardless of whether the API is local or in Docker. This is the primary way to install packs from your local machine into a Dockerized system.
- `attune pack register <server-path>` — Sends a filesystem path string to the API (`POST /api/v1/packs/register`). Only works if the path is accessible from inside the API container (e.g. `/opt/attune/packs/...` or `/opt/attune/packs.dev/...`).

**Workflow Upload** (`attune workflow upload <action-yaml-path>`):
- Reads the local action YAML file and extracts the `workflow_file` field to find the companion workflow YAML
- Determines the pack from the action ref (e.g., `mypack.deploy` → pack `mypack`, name `deploy`)
- The `workflow_file` path is resolved relative to the action YAML's parent directory (same as how pack loaders resolve it relative to the `actions/` directory)
- Constructs a `SaveWorkflowFileRequest` JSON payload combining action metadata (label, parameters, output, tags) with the workflow definition (version, vars, tasks, output_map) and POSTs to `POST /api/v1/packs/{pack_ref}/workflow-files`
- On 409 Conflict (workflow already exists), fails unless `--force` is passed, in which case it PUTs to `PUT /api/v1/workflows/{ref}/file` to update
- Does NOT require a full pack upload — individual workflow actions can be added to existing packs independently
- **Important**: The action YAML MUST contain a `workflow_file` field; regular (non-workflow) actions should be uploaded as part of a pack

**Pack Upload API endpoint**: `POST /api/v1/packs/upload` — accepts `multipart/form-data` with:
- `pack` (required): a `.tar.gz` archive of the pack directory
- `force` (optional, text): `"true"` to overwrite an existing pack with the same ref
- `skip_tests` (optional, text): `"true"` to skip test execution after registration

The server extracts the archive to a temp directory, finds the `pack.yaml` (at root or one level deep), then moves it to `{packs_base_dir}/{pack_ref}/` and calls `register_pack_internal`.

## Test Failure Protocol

**Proactively investigate and fix test failures when discovered, even if unrelated to the current task.**

### Guidelines:
- **ALWAYS report test failures** to the user with relevant error output
- **ALWAYS run tests** after making changes: `make test` or `cargo test`
- **DO fix immediately** if the cause is obvious and fixable in 1-2 attempts
- **DO ask the user** if the failure is complex, requires architectural changes, or you're unsure of the cause
- **NEVER silently ignore** test failures or skip tests without approval
- **Gather context**: Run with `cargo test -- --nocapture --test-threads=1` for details

### Priority:
- **Critical** (build/compile failures): Fix immediately
- **Related** (affects current work): Fix before proceeding
- **Unrelated**: Report and ask if you should fix now or defer

When reporting, ask: "Should I fix this first or continue with [original task]?"

## Code Quality: Zero Warnings Policy

**Maintain zero compiler warnings across the workspace.** Clean builds ensure new issues are immediately visible.

### Workflow
- **Check after changes:** `cargo check --all-targets --workspace`
- **Before completing work:** Fix or document any warnings introduced
- **End of session:** Verify zero warnings before finishing

### Handling Warnings
- **Fix first:** Remove dead code, unused imports, unnecessary variables
- **Prefix `_`:** For intentionally unused variables that document intent
- **Use `#[allow(dead_code)]`:** For API methods intended for future use (add doc comment explaining why)
- **Never ignore blindly:** Every suppression needs a clear rationale

### Conservative Approach
- Preserve methods that complete a logical API surface
- Keep test helpers that are part of shared infrastructure
- When uncertain about removal, ask the user

### Red Flags
- ❌ Introducing new warnings
- ❌ Blanket `#[allow(warnings)]` without specific justification
- ❌ Accumulating warnings over time

## File Naming & Location Conventions

### When Adding Features:
- **New API Endpoint**:
  - Route handler in `crates/api/src/routes/<domain>.rs`
  - DTO in `crates/api/src/dto/<domain>.rs`
  - Update `routes/mod.rs` and main router
- **New Domain Model**:
  - Add to `crates/common/src/models.rs`
  - Create migration in `migrations/YYYYMMDDHHMMSS_description.sql`
  - Add repository in `crates/common/src/repositories/<entity>.rs`
- **New Service**: Add to `crates/` and update workspace `Cargo.toml` members
- **Configuration**: Update `crates/common/src/config.rs` with serde defaults
- **Documentation**: Add to `docs/` directory

### Important Files
- `crates/common/src/models.rs` - All domain models
- `crates/common/src/error.rs` - Error types
- `crates/common/src/config.rs` - Configuration structure
- `crates/api/src/routes/mod.rs` - API routing
- `crates/worker/src/agent_main.rs` - Universal worker agent entrypoint
- `crates/worker/src/runtime_detect.rs` - Runtime auto-detection module (probes for interpreters)
- `crates/worker/src/dynamic_runtime.rs` - Dynamic runtime registration (auto-registers detected runtimes into DB)
- `config.development.yaml` - Dev configuration
- `Cargo.toml` - Workspace dependencies
- `Makefile` - Development commands
- `docker/Dockerfile.optimized` - Optimized service builds (api, executor, notifier)
- `docker/Dockerfile.worker.optimized` - Optimized worker builds (shell, python, node, full)
- `docker/Dockerfile.sensor.optimized` - Optimized sensor builds (base, full)
- `docker/Dockerfile.agent` - Statically-linked agent binary (musl, for injection into any container)
- `docker/Dockerfile.pack-binaries` - Separate pack binary builder
- `scripts/build-pack-binaries.sh` - Build pack binaries script

## Common Pitfalls to Avoid
1. **NEVER** bypass repositories - always use the repository layer for DB access
2. **NEVER** forget `RequireAuth` middleware on protected endpoints
3. **NEVER** hardcode service URLs - use configuration
4. **NEVER** commit secrets in config files (use env vars in production)
5. **NEVER** hardcode schema prefixes in SQL queries - rely on PostgreSQL `search_path` mechanism
6. **NEVER** copy packs into Dockerfiles - they are mounted as volumes
7. **NEVER** put workflow definition content directly in action YAML — use a separate `.workflow.yaml` file in `actions/workflows/` and reference it via `workflow_file` in the action YAML
8. **ALWAYS** use PostgreSQL enum type mappings for custom enums
9. **ALWAYS** use transactions for multi-table operations
10. **ALWAYS** start with `attune/` or correct crate name when specifying file paths
11. **ALWAYS** convert runtime names to lowercase for comparison (database may store capitalized)
12. **ALWAYS** use optimized Dockerfiles for new services (selective crate copying)
13. **REMEMBER** IDs are `i64`, not `i32` or `uuid`
14. **REMEMBER** schema is determined by `search_path`, not hardcoded in queries (production uses `attune`, development uses `public`)
15. **REMEMBER** to regenerate SQLx metadata after schema-related changes: `cargo sqlx prepare`
16. **REMEMBER** packs are volumes - update with restart, not rebuild
17. **REMEMBER** to build pack binaries separately: `./scripts/build-pack-binaries.sh`
18. **REMEMBER** when adding mutable columns to `execution` or `worker`, add a corresponding `IS DISTINCT FROM` check to the entity's history trigger function in the TimescaleDB migration. Events and enforcements are hypertables without history tables — do NOT add frequently-mutated columns to them. Execution is both a hypertable AND has an `execution_history` table (because it is mutable with ~4 updates per row).
19. **REMEMBER** for large JSONB columns in history triggers (like `execution.result`), use `_jsonb_digest_summary()` instead of storing the raw value — see migration `000009_timescaledb_history`
20. **NEVER** use `SELECT *` on tables that have DB-only columns not in the Rust `FromRow` struct (e.g., `execution.is_workflow`, `execution.workflow_def` exist in SQL but not in the `Execution` model). Define a `SELECT_COLUMNS` constant in the repository (see `execution.rs`, `pack.rs`, `runtime_version.rs` for examples) and reference it from all queries — including queries outside the repository (e.g., `timeout_monitor.rs` imports `execution::SELECT_COLUMNS`).ause runtime deserialization failures.
21. **REMEMBER** `execution`, `event`, and `enforcement` are all TimescaleDB hypertables — they **cannot be the target of FK constraints**. Any column referencing them (e.g., `inquiry.execution`, `workflow_execution.execution`, `execution.parent`) is a plain BIGINT with no FK and may become a dangling reference.

## Deployment
- **Target**: Distributed deployment with separate service instances
- **Docker**: Dockerfiles for each service (planned in `docker/` dir)
- **Config**: Use environment variables for secrets in production
- **Database**: PostgreSQL 14+ with connection pooling
- **Message Queue**: RabbitMQ required for service communication
- **Web UI**: Static files served separately or via API service

## Current Development Status
- ✅ **Complete**: Database migrations (21 tables, 12 migration files), API service (most endpoints), common library, message queue infrastructure, repository layer, JWT auth, CLI tool, Web UI (basic + workflow builder + workflow timeline DAG), Executor service (core functionality + workflow orchestration), Worker service (shell/Python execution), Runtime version data model, constraint matching, worker version selection pipeline, version verification at startup, per-version environment isolation, TimescaleDB entity history tracking (execution, worker), Event, enforcement, and execution tables as TimescaleDB hypertables (time-series with retention/compression), History API endpoints (generic + entity-specific with pagination & filtering), History UI panels on entity detail pages (execution), TimescaleDB continuous aggregates (6 hourly rollup views with auto-refresh policies), Analytics API endpoints (7 endpoints under `/api/v1/analytics/` — dashboard, execution status/throughput/failure-rate, event volume, worker status, enforcement volume), Analytics dashboard widgets (bar charts, stacked status charts, failure rate ring gauge, time range selector), Workflow execution orchestration (scheduler detects workflow actions, creates child task executions, completion listener advances workflow via transitions), Workflow template resolution (type-preserving `{{ }}` rendering in task inputs), Workflow `with_items` expansion (parallel child executions per item), Workflow `with_items` concurrency limiting (sliding-window dispatch with pending items stored in workflow variables), Workflow `publish` directive processing (variable propagation between tasks), Workflow function expressions (`result()`, `succeeded()`, `failed()`, `timed_out()`), Workflow expression engine (full arithmetic/comparison/boolean/membership operators, 30+ built-in functions, recursive-descent parser), Canonical workflow namespaces (`parameters`, `workflow`, `task`, `config`, `keystore`, `item`, `index`, `system`), Artifact content system (versioned file/JSON storage, progress-append semantics, binary upload/download, retention enforcement, execution-linked artifacts, 18 API endpoints under `/api/v1/artifacts/`, file-backed disk storage via shared volume for file-type artifacts), CLI artifact management (`attune artifact list/show/create/upload/download/delete` + `attune artifact version list/show/upload/create-json/download/delete` — full CRUD for artifacts and their versions with multipart file upload, binary download, JSON version creation, auto-detected MIME types, human-readable size formatting, and pagination), CLI `--wait` flag (WebSocket-first with polling fallback — connects to notifier on port 8081, subscribes to execution, returns immediately on terminal status; falls back to exponential-backoff REST polling if WS unavailable; polling always gets at least 10s budget regardless of how long WS path ran), Workflow Timeline DAG visualization (Prefect-style time-aligned Gantt+DAG on execution detail page, pure SVG, transition-aware edge coloring from workflow definition metadata, hover tooltips, click-to-highlight path, zoom/pan), Universal Worker Agent Phase 1 (static binary build infrastructure — `attune-agent` binary target in worker crate with musl cross-compilation, runtime auto-detection module probing 8 interpreter families, `--detect-only` diagnostic flag, `docker/Dockerfile.agent` multi-stage build, Makefile targets `build-agent`/`docker-build-agent`/`run-agent`), Universal Worker Agent Phase 2 (runtime auto-detection integration with worker registration — `DetectedRuntime` is serializable, `WorkerRegistration.set_detected_runtimes()` stores structured `detected_interpreters` capability with binary paths and versions, `WorkerRegistration.set_agent_mode()` sets `agent_mode` boolean capability, `WorkerService.with_detected_runtimes()` builder method passes agent detection results to registration during `start()`, `agent_main.rs` passes auto-detected runtimes through to worker registration), Universal Worker Agent Phase 3 (`WorkerService` dual-mode refactor — `StartupMode` enum with `Worker` and `Agent { detected_runtimes }` variants, `WorkerService.startup_mode` field replaces `detected_runtimes` option, `with_detected_runtimes()` sets `StartupMode::Agent`, `start()` conditionally skips proactive version verification and environment setup in agent mode, `ProcessRuntime::execute()` performs lazy on-demand environment creation when env dir is missing instead of just warning, `StartupMode` re-exported from `attune_worker` crate), Universal Worker Agent Phase 4 (Docker Compose integration — `init-agent` service in `docker-compose.yaml` builds statically-linked agent binary and populates `agent_bin` volume, `docker-compose.agent.yaml` override file with example agent-based workers for Ruby/Python/GPU/custom images, Makefile targets `docker-up-agent`/`docker-down-agent`, quick-reference docs at `docs/QUICKREF-agent-workers.md`), Universal Worker Agent Phase 5 (API binary download endpoint — `GET /api/v1/agent/binary` streams the statically-linked agent binary with architecture selection via `?arch=x86_64|aarch64`, falls back from arch-specific to generic binary name, optional bootstrap token auth via `X-Agent-Token` header or `token` query param configured in `agent.bootstrap_token`; `GET /api/v1/agent/info` returns available architectures and binary sizes; `AgentConfig` in common config with `binary_dir` and `bootstrap_token`; `agent_bin` volume mounted read-only in API container; `scripts/attune-agent-wrapper.sh` bootstrap script with auto-detection of architecture, retry-based download from API, and `curl`/`wget` fallback), Universal Worker Agent Phase 6 (database & runtime registry extensions — `runtime.auto_detected` BOOLEAN column to distinguish agent-created vs. pack-loaded runtimes, `runtime.detection_config` JSONB column for detection metadata (detected path, version, binary name), runtime template packs for Ruby/Go/Java/Perl/R in `packs/core/runtimes/`, dynamic runtime registration module `crates/worker/src/dynamic_runtime.rs` with `auto_register_detected_runtimes()` that runs before `WorkerService::new()` in agent mode — looks up detected runtimes by alias-aware name matching, clones from pack template if available or creates minimal entry, marks auto-registered runtimes with `auto_detected = true`, `normalize_runtime_name()` extended with 5 new alias groups for the new runtimes, `SELECT_COLUMNS` constant added to RuntimeRepository), Universal Worker Agent Phase 7 (Kubernetes support — Helm chart `agent-workers.yaml` template creates a Deployment per `agentWorkers[]` values entry using the InitContainer pattern: `agent-loader` init container copies the statically-linked binary from the `attune-agent` image into an `emptyDir` volume, worker container runs any user-specified image with the agent as entrypoint; supports `nodeSelector`, `tolerations`, `runtimeClassName` for GPU/specialized scheduling, custom env vars, resource limits, runtime auto-detect or explicit override; `images.agent` added to `values.yaml` for registry-aware image resolution; `attune-agent` image added to Gitea Actions publish workflow as `agent-init` target; quick-reference docs at `docs/QUICKREF-kubernetes-agent-workers.md`)
- 🔄 **In Progress**: Sensor service, advanced workflow features (nested workflow context propagation), Python runtime dependency management, API/UI endpoints for runtime version management, Artifact UI (web UI for browsing/downloading artifacts), Notifier service WebSocket (functional but lacks auth — the WS connection is unauthenticated; the subscribe filter controls visibility)
- 📋 **Planned**: Execution policies, monitoring, pack registry system, configurable retention periods via admin settings, export/archival to external storage

## Quick Reference

### Start Development Environment
```bash
# Start PostgreSQL and RabbitMQ
# Load core pack: ./scripts/load-core-pack.sh
# Start API: make run-api
# Start Web UI: cd web && npm run dev
```

### File Path Examples
- Models: `attune/crates/common/src/models.rs`
- API routes: `attune/crates/api/src/routes/actions.rs`
- Repositories: `attune/crates/common/src/repositories/execution.rs`
- Migrations: `attune/migrations/*.sql`
- Web UI: `attune/web/src/`
- Config: `attune/config.development.yaml`

### Documentation Locations
- API docs: `attune/docs/api-*.md`
- Configuration: `attune/docs/configuration.md`
- Architecture: `attune/docs/*-architecture.md`, `attune/docs/*-service.md`
- Testing: `attune/docs/testing-*.md`, `attune/docs/running-tests.md`, `attune/docs/schema-per-test.md`
- Docker optimization: `attune/docs/docker-layer-optimization.md`, `attune/docs/QUICKREF-docker-optimization.md`, `attune/docs/QUICKREF-buildkit-cache-strategy.md`
- Packs architecture: `attune/docs/QUICKREF-packs-volumes.md`, `attune/docs/DOCKER-OPTIMIZATION-SUMMARY.md`
- AI Agent Work Summaries: `attune/work-summary/*.md`
- Deployment: `attune/docs/production-deployment.md`
- DO NOT create additional documentation files in the root of the project. all new documentation describing how to use the system should be placed in the `attune/docs` directory, and documentation describing the work performed should be placed in the `attune/work-summary` directory.

## Work Summary & Reporting

**Avoid redundant summarization - summarize changes once at completion, not continuously.**

### Guidelines:
- **Report progress** during work: brief status updates, blockers, questions
- **Summarize once** at completion: consolidated overview of all changes made
- **Work summaries**: Write to `attune/work-summary/*.md` only at task completion, not incrementally
- **Avoid duplication**: Don't re-explain the same changes multiple times in different formats
- **What changed, not how**: Focus on outcomes and impacts, not play-by-play narration

### Good Pattern:
```
[Making changes with tool calls and brief progress notes]
...
[At completion]
"I've completed the task. Here's a summary of changes: [single consolidated overview]"
```

### Bad Pattern:
```
[Makes changes]
"So I changed X, Y, and Z..."
[More changes]
"To summarize, I modified X, Y, and Z..."
[Writes work summary]
"In this session I updated X, Y, and Z..."
```

## Maintaining the AGENTS.md file

**IMPORTANT: Keep this file up-to-date as the project evolves.**

After making changes to the project, you MUST update this `AGENTS.md` file if any of the following occur:

- **New dependencies added or major dependencies removed** (check package.json, Cargo.toml, requirements.txt, etc.)
- **Project structure changes**: new directories/modules created, existing ones renamed or removed
- **Architecture changes**: new layers, patterns, or major refactoring that affects how components interact
- **New frameworks or tools adopted** (e.g., switching from REST to GraphQL, adding a new testing framework)
- **Deployment or infrastructure changes** (new CI/CD pipelines, different hosting, containerization added)
- **New major features** that introduce new subsystems or significantly change existing ones
- **Style guide or coding convention updates**

### `AGENTS.md` Content inclusion policy
- DO NOT simply summarize changes in the `AGENTS.md` file. If there are existing sections that need updating due to changes in the application architecture or project structure, update them accordingly.
- When relevant, work summaries should instead be written to `attune/work-summary/*.md`

### Update procedure:
1. After completing your changes, review if they affect any section of `AGENTS.md`
2. If yes, immediately update the relevant sections
3. Add a brief comment at the top of `AGENTS.md` with the date and what was updated (optional but helpful)

### Update format:
When updating, be surgical - modify only the affected sections rather than rewriting the entire file. Maintain the existing structure and tone.

**Treat `AGENTS.md` as living documentation.** An outdated `AGENTS.md` file is worse than no `AGENTS.md` file, as it will mislead future AI agents and waste time.

## Project Documentation Index
[Attune Project Documentation Index]
|root: ./
|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning
|IMPORTANT: This index provides a quick overview - use grep/read_file for details
|
| Format: path/to/dir:{file1,file2,...}
| '...' indicates truncated file list - use grep/list_directory for full contents
|
| To regenerate this index: make generate-agents-index
|
|docs:{MIGRATION-queue-separation-2026-02-03.md,QUICKREF-containerized-workers.md,QUICKREF-rabbitmq-queues.md,QUICKREF-sensor-worker-registration.md,QUICKREF-unified-runtime-detection.md,README.md,docker-deployment.md,pack-runtime-environments.md,worker-containerization.md,worker-containers-quickstart.md}
|docs/api:{api-actions.md,api-completion-plan.md,api-events-enforcements.md,api-executions.md,api-inquiries.md,api-pack-testing.md,api-pack-workflows.md,api-packs.md,api-rules.md,api-secrets.md,api-triggers-sensors.md,api-workflows.md,openapi-client-generation.md,openapi-spec-completion.md}
|docs/architecture:{executor-service.md,notifier-service.md,pack-management-architecture.md,queue-architecture.md,sensor-service.md,trigger-sensor-architecture.md,web-ui-architecture.md,webhook-system-architecture.md,worker-service.md}
|docs/authentication:{auth-quick-reference.md,authentication.md,secrets-management.md,security-review-2024-01-02.md,service-accounts.md,token-refresh-quickref.md,token-rotation.md}
|docs/cli:{cli-profiles.md,cli.md}
|docs/configuration:{CONFIG_README.md,config-troubleshooting.md,configuration.md,env-to-yaml-migration.md}
|docs/dependencies:{dependency-deduplication-results.md,dependency-deduplication.md,dependency-isolation.md,dependency-management.md,http-client-consolidation-complete.md,http-client-consolidation-plan.md,sea-query-removal.md,serde-yaml-migration.md,workspace-dependency-compliance-audit.md}
|docs/deployment:{ops-runbook-queues.md,production-deployment.md}
|docs/development:{QUICKSTART-vite.md,WORKSPACE_SETUP.md,agents-md-index.md,compilation-notes.md,dead-code-cleanup.md,documentation-organization.md,vite-dev-setup.md}
|docs/examples:{complete-workflow.yaml,pack-test-demo.sh,registry-index.json,rule-parameter-examples.md,simple-workflow.yaml}
|docs/guides:{QUICKREF-timer-happy-path.md,quick-start.md,quickstart-example.md,quickstart-timer-demo.md,timer-sensor-quickstart.md,workflow-quickstart.md}
|docs/migrations:{workflow-task-execution-consolidation.md}
|docs/packs:{PACK_TESTING.md,QUICKREF-git-installation.md,core-pack-integration.md,pack-install-testing.md,pack-installation-git.md,pack-registry-cicd.md,pack-registry-spec.md,pack-structure.md,pack-testing-framework.md}
|docs/performance:{QUICKREF-performance-optimization.md,log-size-limits.md,performance-analysis-workflow-lists.md,performance-before-after-results.md,performance-context-cloning-diagram.md}
|docs/plans:{schema-per-test-refactor.md,timescaledb-entity-history.md,universal-worker-agent.md}
|docs/sensors:{CHECKLIST-sensor-worker-registration.md,COMPLETION-sensor-worker-registration.md,SUMMARY-database-driven-detection.md,database-driven-runtime-detection.md,native-runtime.md,sensor-authentication-overview.md,sensor-interface.md,sensor-lifecycle-management.md,sensor-runtime.md,sensor-service-setup.md,sensor-worker-registration.md}
|docs/testing:{e2e-test-plan.md,running-tests.md,schema-per-test.md,test-user-setup.md,testing-authentication.md,testing-dashboard-rules.md,testing-status.md}
|docs/web-ui:{web-ui-pack-testing.md,websocket-usage.md}
|docs/webhooks:{webhook-manual-testing.md,webhook-testing.md}
|docs/workflows:{dynamic-parameter-forms.md,execution-hierarchy.md,inquiry-handling.md,parameter-mapping-status.md,rule-parameter-mapping.md,rule-trigger-params.md,workflow-execution-engine.md,workflow-implementation-plan.md,workflow-orchestration.md,workflow-summary.md}
|scripts:{check-workspace-deps.sh,cleanup-test-schemas.sh,create-test-user.sh,create_test_user.sh,generate-python-client.sh,generate_agents_md_index.py,load-core-pack.sh,load_core_pack.py,quick-test-happy-path.sh,seed_core_pack.sql,seed_runtimes.sql,setup-db.sh,setup-e2e-db.sh,setup_timer_echo_rule.sh,start-all-services.sh,start-e2e-services.sh,start_services_test.sh,status-all-services.sh,stop-all-services.sh,stop-e2e-services.sh,...}
|work-summary:{2025-01-console-logging-cleanup.md,2025-01-token-refresh-improvements.md,2025-01-websocket-duplicate-connection-fix.md,2026-02-02-unified-runtime-verification.md,2026-02-03-canonical-message-types.md,2026-02-03-inquiry-queue-separation.md,2026-02-04-event-generation-fix.md,README.md,auto-populate-ref-from-label.md,buildkit-cache-implementation.md,collapsible-navigation-implementation.md,containerized-workers-implementation.md,docker-build-race-fix.md,docker-containerization-complete.md,docker-migrations-startup-fix.md,empty-pack-creation-ui.md,git-pack-installation.md,pack-runtime-environments.md,sensor-service-cleanup-standalone-only.md,sensor-worker-registration.md,...}
|work-summary/changelogs:{API-COMPLETION-SUMMARY.md,CHANGELOG.md,CLEANUP_SUMMARY_2026-01-27.md,FIFO-ORDERING-COMPLETE.md,MIGRATION_CONSOLIDATION_SUMMARY.md,cli-integration-tests-summary.md,core-pack-setup-summary.md,web-ui-session-summary.md,webhook-phase3-summary.md,webhook-testing-summary.md,workflow-loader-summary.md}
|work-summary/features:{AUTOMATIC-SCHEMA-CLEANUP-ENHANCEMENT.md,TESTING-TIMER-DEMO.md,e2e-test-schema-issues.md,openapi-spec-verification.md,sensor-runtime-implementation.md,sensor-service-implementation.md}
|work-summary/migrations:{2026-01-17-orquesta-refactoring.md,2026-01-24-generated-client-migration.md,2026-01-27-workflow-migration.md,DEPLOYMENT-READY-performance-optimization.md,MIGRATION_NEXT_STEPS.md,migration_comparison.txt,migration_consolidation_status.md}
|work-summary/phases:{2025-01-policy-ordering-plan.md,2025-01-secret-passing-fix-plan.md,2025-01-workflow-performance-analysis.md,PHASE-5-COMPLETE.md,PHASE_1_1_SUMMARY.txt,PROBLEM.md,Pitfall-Resolution-Plan.md,SENSOR_SERVICE_README.md,StackStorm-Lessons-Learned.md,StackStorm-Pitfalls-Analysis.md,orquesta-refactor-plan.md,phase-1-1-complete.md,phase-1.2-models-repositories-complete.md,phase-1.2-repositories-summary.md,phase-1.3-test-infrastructure-summary.md,phase-1.3-yaml-validation-complete.md,phase-1.4-COMPLETE.md,phase-1.4-loader-registration-progress.md,phase-1.5-COMPLETE.md,phase-1.6-pack-integration-complete.md,...}
|work-summary/sessions:{2024-01-13-event-enforcement-endpoints.md,2024-01-13-inquiry-endpoints.md,2024-01-13-integration-testing-setup.md,2024-01-13-route-conflict-fix.md,2024-01-13-secret-management-api.md,2024-01-17-sensor-runtime.md,2024-01-17-sensor-service-session.md,2024-01-20-core-pack-unit-tests.md,2024-01-20-pack-testing-framework-phase1.md,2024-01-21-pack-registry-phase1.md,2024-01-21-pack-registry-phase2.md,2024-01-22-pack-registry-phase3.md,2024-01-22-pack-registry-phase4.md,2024-01-22-pack-registry-phase5.md,2024-01-22-pack-registry-phase6.md,2025-01-13-phase-1.4-session.md,2025-01-13-yaml-configuration.md,2025-01-16_migration_consolidation.md,2025-01-17-performance-optimization-complete.md,2025-01-18-timer-triggers.md,...}
|work-summary/status:{ACCOMPLISHMENTS.md,COMPILATION_STATUS.md,FIFO-ORDERING-STATUS.md,FINAL_STATUS.md,PROGRESS.md,SENSOR_STATUS.md,TEST-STATUS.md,TODO.OLD.md,TODO.md}
