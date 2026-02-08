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
- **Database**: PostgreSQL 14+ (primary data store + LISTEN/NOTIFY pub/sub)
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
│   ├── common/                   # Shared library (models, db, repos, mq, config, error)
│   ├── api/                      # REST API service (8080)
│   ├── executor/                 # Execution orchestration service
│   ├── worker/                   # Action execution service (multi-runtime)
│   ├── sensor/                   # Event monitoring service
│   ├── notifier/                 # Real-time notification service
│   └── cli/                      # Command-line interface
├── migrations/                   # SQLx database migrations (18 tables)
├── web/                          # React web UI (Vite + TypeScript)
├── packs/                        # Pack bundles
│   └── core/                     # Core pack (timers, HTTP, etc.)
├── docs/                         # Technical documentation
├── scripts/                      # Helper scripts (DB setup, testing)
└── tests/                        # Integration tests
```

## Service Architecture (Distributed Microservices)

1. **attune-api**: REST API gateway, JWT auth, all client interactions
2. **attune-executor**: Manages execution lifecycle, scheduling, policy enforcement
3. **attune-worker**: Executes actions in multiple runtimes (Python/Node.js/containers)
4. **attune-sensor**: Monitors triggers, generates events
5. **attune-notifier**: Real-time notifications via PostgreSQL LISTEN/NOTIFY + WebSocket

**Communication**: Services communicate via RabbitMQ for async operations

## Docker Compose Orchestration

**All Attune services run via Docker Compose.**

- **Compose file**: `docker-compose.yaml` (root directory)
- **Configuration**: `config.docker.yaml` (Docker-specific settings)
- **Default user**: `test@attune.local` / `TestPass123!` (auto-created)

**Services**:
- **Infrastructure**: postgres, rabbitmq, redis
- **Init** (run-once): migrations, init-user, init-packs
- **Application**: api (8080), executor, worker-{shell,python,node,full}, sensor, notifier (8081), web (3000)

**Commands**:
```bash
docker compose up -d          # Start all services
docker compose down           # Stop all services
docker compose logs -f <svc>  # View logs
```

**Key environment overrides**: `JWT_SECRET`, `ENCRYPTION_KEY` (required for production)

### Docker Build Optimization
- **Optimized Dockerfiles**: `docker/Dockerfile.optimized` and `docker/Dockerfile.worker.optimized`
- **Strategy**: Selective crate copying - only copy crates needed for each service (not entire workspace)
- **Performance**: 90% faster incremental builds (~30 sec vs ~5 min for code changes)
- **BuildKit cache mounts**: Persist cargo registry and compilation artifacts between builds
  - **Cache strategy**: `sharing=shared` for registry/git (concurrent-safe), service-specific IDs for target caches
  - **Parallel builds**: 4x faster than old `sharing=locked` strategy - no serialization overhead
- **Documentation**: See `docs/docker-layer-optimization.md`, `docs/QUICKREF-docker-optimization.md`, `docs/QUICKREF-buildkit-cache-strategy.md`

### Packs Volume Architecture
- **Key Principle**: Packs are NOT copied into Docker images - they are mounted as volumes
- **Volume Flow**: Host `./packs/` → `init-packs` service → `packs_data` volume → mounted in all services
- **Benefits**: Update packs with restart (~5 sec) instead of rebuild (~5 min)
- **Pack Binaries**: Built separately with `./scripts/build-pack-binaries.sh` (GLIBC compatibility)
- **Development**: Use `./packs.dev/` for instant testing (direct bind mount, no restart needed)
- **Documentation**: See `docs/QUICKREF-packs-volumes.md`

## Domain Model & Event Flow

**Critical Event Flow**:
```
Sensor → Trigger fires → Event created → Rule evaluates →
Enforcement created → Execution scheduled → Worker executes Action
```

**Key Entities** (all in `public` schema, IDs are `i64`):
- **Pack**: Bundle of automation components (actions, sensors, rules, triggers)
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
- **Key**: Encrypted secrets storage

## Key Tools & Libraries

### Shared Dependencies (workspace-level)
- **Async**: tokio, async-trait, futures
- **Web**: axum, tower, tower-http
- **Database**: sqlx (with postgres, json, chrono, uuid features)
- **Serialization**: serde, serde_json, serde_yaml_ng
- **Logging**: tracing, tracing-subscriber
- **Error Handling**: anyhow, thiserror
- **Config**: config crate (YAML + env vars)
- **Validation**: validator
- **Auth**: jsonwebtoken, argon2
- **CLI**: clap
- **OpenAPI**: utoipa, utoipa-swagger-ui
- **Message Queue**: lapin (RabbitMQ)
- **HTTP Client**: reqwest
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
  - Example: `ATTUNE__DATABASE__URL`, `ATTUNE__SERVER__PORT`
- **Loading Priority**: Base config → env-specific config → env vars
- **Required for Production**: `JWT_SECRET`, `ENCRYPTION_KEY` (32+ chars)
- **Location**: Root directory or `ATTUNE_CONFIG` env var path

## Authentication & Security
- **Auth Type**: JWT (access tokens: 1h, refresh tokens: 7d)
- **Password Hashing**: Argon2id
- **Protected Routes**: Use `RequireAuth(user)` extractor in Axum
- **Secrets Storage**: AES-GCM encrypted in `key` table with scoped ownership
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
**Table Count**: 17 tables total in the schema

### Pack File Loading & Action Execution
- **Pack Base Directory**: Configured via `packs_base_dir` in config (defaults to `/opt/attune/packs`, development uses `./packs`)
- **Pack Volume Strategy**: Packs are mounted as volumes (NOT copied into Docker images)
  - Host `./packs/` → `packs_data` volume via `init-packs` service → mounted at `/opt/attune/packs` in all services
  - Development packs in `./packs.dev/` are bind-mounted directly for instant updates
- **Pack Binaries**: Native binaries (sensors) built separately with `./scripts/build-pack-binaries.sh`
- **Action Script Resolution**: Worker constructs file paths as `{packs_base_dir}/{pack_ref}/actions/{entrypoint}`
- **Runtime Selection**: Determined by action's runtime field (e.g., "Shell", "Python") - compared case-insensitively
- **Parameter Delivery**: Actions receive parameters via stdin as JSON (never environment variables)
- **Output Format**: Actions declare output format (text/json/yaml) - json/yaml are parsed into execution.result JSONB
- **Standard Environment Variables**: Worker provides execution context via `ATTUNE_*` environment variables:
  - `ATTUNE_ACTION` - Action ref (always present)
  - `ATTUNE_EXEC_ID` - Execution database ID (always present)
  - `ATTUNE_API_TOKEN` - Execution-scoped API token (always present)
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
- **Modules**: `models`, `repositories`, `db`, `config`, `error`, `mq`, `crypto`, `utils`, `workflow`, `pack_registry`
- **Exports**: Commonly used types re-exported from `lib.rs`
- **Repository Layer**: All DB access goes through repositories in `repositories/`
- **Message Queue**: Abstractions in `mq/` for RabbitMQ communication

### Web UI (`web/`)
- **Generated Client**: OpenAPI client auto-generated from API spec
  - Run: `npm run generate:api` (requires API running on :8080)
  - Location: `src/api/`
- **State Management**: Zustand for global state, TanStack Query for server state
- **Styling**: Tailwind utility classes
- **Dev Server**: `npm run dev` (typically :3000 or :5173)
- **Build**: `npm run build`

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
attune action execute <ref> --param key=value
attune execution list            # Monitor executions
```

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
- `config.development.yaml` - Dev configuration
- `Cargo.toml` - Workspace dependencies
- `Makefile` - Development commands
- `docker/Dockerfile.optimized` - Optimized service builds
- `docker/Dockerfile.worker.optimized` - Optimized worker builds
- `docker/Dockerfile.pack-binaries` - Separate pack binary builder
- `scripts/build-pack-binaries.sh` - Build pack binaries script

## Common Pitfalls to Avoid
1. **NEVER** bypass repositories - always use the repository layer for DB access
2. **NEVER** forget `RequireAuth` middleware on protected endpoints
3. **NEVER** hardcode service URLs - use configuration
4. **NEVER** commit secrets in config files (use env vars in production)
5. **NEVER** hardcode schema prefixes in SQL queries - rely on PostgreSQL `search_path` mechanism
6. **NEVER** copy packs into Dockerfiles - they are mounted as volumes
7. **ALWAYS** use PostgreSQL enum type mappings for custom enums
8. **ALWAYS** use transactions for multi-table operations
9. **ALWAYS** start with `attune/` or correct crate name when specifying file paths
10. **ALWAYS** convert runtime names to lowercase for comparison (database may store capitalized)
11. **ALWAYS** use optimized Dockerfiles for new services (selective crate copying)
12. **REMEMBER** IDs are `i64`, not `i32` or `uuid`
13. **REMEMBER** schema is determined by `search_path`, not hardcoded in queries (production uses `attune`, development uses `public`)
14. **REMEMBER** to regenerate SQLx metadata after schema-related changes: `cargo sqlx prepare`
15. **REMEMBER** packs are volumes - update with restart, not rebuild
16. **REMEMBER** to build pack binaries separately: `./scripts/build-pack-binaries.sh`

## Deployment
- **Target**: Distributed deployment with separate service instances
- **Docker**: Dockerfiles for each service (planned in `docker/` dir)
- **Config**: Use environment variables for secrets in production
- **Database**: PostgreSQL 14+ with connection pooling
- **Message Queue**: RabbitMQ required for service communication
- **Web UI**: Static files served separately or via API service

## Current Development Status
- ✅ **Complete**: Database migrations (17 tables), API service (most endpoints), common library, message queue infrastructure, repository layer, JWT auth, CLI tool, Web UI (basic), Executor service (core functionality), Worker service (shell/Python execution)
- 🔄 **In Progress**: Sensor service, advanced workflow features, Python runtime dependency management
- 📋 **Planned**: Notifier service, execution policies, monitoring, pack registry system

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
|docs/plans:{schema-per-test-refactor.md}
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
