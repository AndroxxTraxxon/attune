# Workflow Orchestration Implementation Plan

## Executive Summary

This document outlines the implementation plan for adding **workflow orchestration** capabilities to Attune. Workflows enable composing multiple actions into complex, conditional execution graphs with variable passing, iteration, and error handling.

## Key Design Decisions

### 1. Workflows as Actions
Workflows are first-class actions that can be:
- Triggered by rules (event-driven)
- Invoked by other workflows (composable)
- Executed directly via API
- Referenced in the same way as regular actions

### 2. YAML-Based Definition
Workflows are defined declaratively in YAML files within pack directories, making them:
- Version-controllable
- Human-readable
- Easy to author and maintain
- Portable across environments

### 3. Event-Driven Execution
Workflows leverage the existing message queue infrastructure:
- Each task creates a child execution
- Tasks execute asynchronously via workers
- Progress is tracked via execution status messages
- No blocking or polling required

### 4. Multi-Scope Variable System
Variables are accessible from 6 scopes (in precedence order):
1. `task.*` - Results from completed tasks
2. `vars.*` - Workflow-scoped variables
3. `parameters.*` - Input parameters
4. `pack.config.*` - Pack configuration
5. `system.*` - System variables (execution_id, timestamp, identity)
6. `kv.*` - Key-value datastore

## Architecture Overview

```
┌────────────────────────────────────────────────────────────┐
│                    Attune Platform                          │
├────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐  ┌────────────┐  ┌──────────────────┐    │
│  │ API Service │  │  Executor  │  │ Worker Service   │    │
│  │             │  │  Service   │  │                  │    │
│  │  Workflow   │  │            │  │  Runtime Engine  │    │
│  │  CRUD       │  │ ┌────────┐ │  │                  │    │
│  │             │  │ │Workflow│ │  │  Execute Actions │    │
│  │             │  │ │Engine  │ │  │                  │    │
│  └─────────────┘  │ │        │ │  └──────────────────┘    │
│                    │ │- Parser│ │                          │
│                    │ │- Graph │ │                          │
│                    │ │- Context│ │                         │
│                    │ │- Sched │ │                          │
│                    │ └────────┘ │                          │
│                    └────────────┘                          │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           PostgreSQL Database                        │  │
│  │  - workflow_definition                               │  │
│  │  - workflow_execution                                │  │
│  │  - workflow_task_execution                           │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
└────────────────────────────────────────────────────────────┘
```

## Database Schema Changes

### New Tables

1. **`workflow_definition`**
   - Stores parsed workflow YAML as JSON
   - Links to pack
   - Contains parameter/output schemas
   - Full task graph definition

2. **`workflow_execution`**
   - Tracks runtime state of workflow
   - Stores variable context
   - Maintains task completion tracking
   - Links to parent execution

3. **`workflow_task_execution`**
   - Individual task execution tracking
   - Supports iteration (with-items)
   - Retry tracking
   - Result storage

### Modified Tables

- **`action`** table gets two new columns:
  - `is_workflow` (boolean)
  - `workflow_def` (foreign key)

## Core Features

### 1. Sequential Execution
Tasks execute one after another based on transitions:
```yaml
tasks:
  - name: task1
    action: pack.action1
    on_success: task2
  - name: task2
    action: pack.action2
```

### 2. Parallel Execution
Multiple tasks execute concurrently:
```yaml
tasks:
  - name: parallel_checks
    type: parallel
    tasks:
      - name: check_db
        action: db.health
      - name: check_cache
        action: cache.health
    on_success: deploy
```

### 3. Conditional Branching
Execute tasks based on conditions:
```yaml
tasks:
  - name: check_env
    action: core.noop
    decision:
      - when: "{{ parameters.env == 'production' }}"
        next: require_approval
      - default: deploy_directly
```

### 4. Iteration (with-items)
Process lists with optional batching:
```yaml
tasks:
  - name: deploy_regions
    action: deploy.to_region
    with_items: "{{ parameters.regions }}"
    batch_size: 5  # Process 5 at a time
    input:
      region: "{{ item }}"
```

### 5. Variable Publishing
Tasks can publish results to workflow scope:
```yaml
tasks:
  - name: create_resource
    action: cloud.create
    publish:
      - resource_id: "{{ task.create_resource.result.id }}"
      - resource_url: "{{ task.create_resource.result.url }}"
```

### 6. Error Handling & Retry
Built-in retry with backoff:
```yaml
tasks:
  - name: flaky_task
    action: http.request
    retry:
      count: 5
      delay: 10
      backoff: exponential
    on_success: next_task
    on_failure: cleanup_task
```

### 7. Human-in-the-Loop
Integrate inquiry (approval) steps:
```yaml
tasks:
  - name: require_approval
    action: core.inquiry
    input:
      prompt: "Approve deployment?"
      schema:
        type: object
        properties:
          approved:
            type: boolean
    decision:
      - when: "{{ task.require_approval.result.approved }}"
        next: deploy
      - default: cancel
```

### 8. Nested Workflows
Workflows can invoke other workflows:
```yaml
tasks:
  - name: provision_infra
    action: infrastructure.full_stack  # This is also a workflow
    input:
      environment: "{{ parameters.env }}"
```

## Template System

### Template Engine: Tera (Rust)
- Jinja2-like syntax
- Variable interpolation: `{{ vars.name }}`
- Filters: `{{ text | upper }}`
- Conditionals: `{{ value ? 'yes' : 'no' }}`

### Helper Functions
```yaml
# String operations
message: "{{ parameters.name | upper | trim }}"

# List operations
first: "{{ vars.list | first }}"
count: "{{ vars.list | length }}"

# JSON operations
parsed: "{{ vars.json_string | from_json }}"

# Batching
batches: "{{ vars.items | batch(size=100) }}"

# Key-value store
value: "{{ kv.get('config.key', default='fallback') }}"
secret: "{{ kv.get_secret('api.token') }}"
```

## Workflow Lifecycle

```
1. Rule/API triggers workflow action
   ↓
2. Executor detects is_workflow=true
   ↓
3. Load workflow_definition from database
   ↓
4. Create workflow_execution record
   ↓
5. Initialize variable context with parameters
   ↓
6. Build task dependency graph
   ↓
7. Schedule initial tasks (entry points)
   ↓
8. For each task:
   a. Template task inputs
   b. Create child execution
   c. Create workflow_task_execution record
   d. Publish execution.scheduled message
   ↓
9. Worker executes task, publishes result
   ↓
10. Workflow Engine receives completion:
    a. Update workflow_task_execution
    b. Publish variables to context
    c. Evaluate transitions
    d. Schedule next tasks
    ↓
11. Repeat until all tasks complete
    ↓
12. Update workflow_execution status
    ↓
13. Publish workflow.completed event
```

## Implementation Phases

### Phase 1: Foundation (2 weeks)
**Goal**: Core data structures and parsing

- [ ] Database migration for workflow tables
- [ ] Add workflow models to `common/src/models.rs`
- [ ] Create workflow repositories
- [ ] Implement YAML parser for workflow definitions
- [ ] Integrate Tera template engine
- [ ] Create variable context manager

**Deliverables**:
- Migration file: `migrations/020_workflow_orchestration.sql`
- Models: `common/src/models/workflow.rs`
- Repositories: `common/src/repositories/workflow*.rs`
- Parser: `executor/src/workflow/parser.rs`
- Context: `executor/src/workflow/context.rs`

### Phase 2: Execution Engine (2 weeks)
**Goal**: Core workflow execution logic

- [ ] Implement task graph builder
- [ ] Implement graph traversal logic
- [ ] Create workflow executor service
- [ ] Add workflow message handlers
- [ ] Implement task scheduling
- [ ] Handle task completion events

**Deliverables**:
- Graph engine: `executor/src/workflow/graph.rs`
- Executor: `executor/src/workflow/executor.rs`
- Message handlers: `executor/src/workflow/messages.rs`
- State machine: `executor/src/workflow/state.rs`

### Phase 3: Advanced Features (2 weeks)
**Goal**: Iteration, parallelism, error handling

- [ ] Implement with-items iteration
- [ ] Add batching support
- [ ] Implement parallel task execution
- [ ] Add retry logic with backoff
- [ ] Implement timeout handling
- [ ] Add conditional branching (decision trees)

**Deliverables**:
- Iterator: `executor/src/workflow/iterator.rs`
- Parallel executor: `executor/src/workflow/parallel.rs`
- Retry handler: `executor/src/workflow/retry.rs`

### Phase 4: API & Tools (2 weeks)
**Goal**: Management interface and tooling

- [ ] Workflow CRUD API endpoints
- [ ] Workflow execution monitoring API
- [ ] Control operations (pause/resume/cancel)
- [ ] Workflow validation CLI command
- [ ] Workflow visualization endpoint
- [ ] Pack registration workflow scanning

**Deliverables**:
- API routes: `api/src/routes/workflows.rs`
- API handlers: `api/src/handlers/workflows.rs`
- CLI commands: `cli/src/commands/workflow.rs` (future)
- Documentation updates

### Phase 5: Testing & Documentation (1 week)
**Goal**: Comprehensive testing and docs

- [ ] Unit tests for all components
- [ ] Integration tests for workflows
- [ ] Example workflows (simple, complex, failure cases)
- [ ] User documentation
- [ ] API documentation
- [ ] Migration guide

**Deliverables**:
- Test suite: `executor/tests/workflow_tests.rs`
- Examples: `docs/examples/workflow-*.yaml`
- User guide: `docs/workflow-user-guide.md`
- Migration guide: `docs/workflow-migration.md`

## Total Timeline: 9 Weeks

## Testing Strategy

### Unit Tests
- Template rendering with all scope types
- Graph construction and traversal
- Condition evaluation
- Variable publishing
- Task scheduling logic
- Retry logic
- Timeout handling

### Integration Tests
- Simple sequential workflow
- Parallel execution workflow
- Conditional branching workflow
- Iteration workflow (with batching)
- Error handling and retry
- Nested workflow execution
- Workflow cancellation
- Long-running workflow

### Example Test Workflows
Located in `docs/examples/`:
- `simple-workflow.yaml` - Basic sequential flow
- `complete-workflow.yaml` - All features demonstrated
- `parallel-workflow.yaml` - Parallel execution
- `conditional-workflow.yaml` - Branching logic
- `iteration-workflow.yaml` - with-items examples

## API Endpoints

### Workflow Management
```
POST   /api/v1/packs/{pack_ref}/workflows          - Create workflow
GET    /api/v1/packs/{pack_ref}/workflows          - List workflows in pack
GET    /api/v1/workflows                           - List all workflows
GET    /api/v1/workflows/{workflow_ref}            - Get workflow definition
PUT    /api/v1/workflows/{workflow_ref}            - Update workflow
DELETE /api/v1/workflows/{workflow_ref}            - Delete workflow
POST   /api/v1/workflows/{workflow_ref}/execute    - Execute workflow directly
POST   /api/v1/workflows/{workflow_ref}/validate   - Validate workflow definition
```

### Workflow Execution Management
```
GET    /api/v1/workflow-executions                 - List workflow executions
GET    /api/v1/workflow-executions/{id}            - Get workflow execution details
GET    /api/v1/workflow-executions/{id}/tasks      - List task executions
GET    /api/v1/workflow-executions/{id}/graph      - Get execution graph (visualization)
GET    /api/v1/workflow-executions/{id}/context    - Get variable context
POST   /api/v1/workflow-executions/{id}/pause      - Pause workflow
POST   /api/v1/workflow-executions/{id}/resume     - Resume paused workflow
POST   /api/v1/workflow-executions/{id}/cancel     - Cancel workflow
POST   /api/v1/workflow-executions/{id}/retry      - Retry failed workflow
```

## Pack Structure with Workflows

```
packs/
└── my_pack/
    ├── pack.yaml               # Pack metadata
    ├── config.yaml             # Pack configuration schema
    ├── actions/
    │   ├── action1.py
    │   ├── action2.py
    │   └── action.yaml
    ├── sensors/
    │   ├── sensor1.py
    │   └── sensor.yaml
    ├── workflows/              # NEW: Workflow definitions
    │   ├── deploy.yaml
    │   ├── backup.yaml
    │   ├── migrate.yaml
    │   └── rollback.yaml
    ├── rules/
    │   └── on_push.yaml
    └── tests/
        ├── test_actions.py
        └── test_workflows.yaml  # Workflow test definitions
```

### Pack Registration Process

When a pack is registered:
1. Scan `workflows/` directory for `.yaml` files
2. Parse and validate each workflow definition
3. Create `workflow_definition` record in database
4. Create synthetic `action` record with `is_workflow=true`
5. Link action to workflow via `workflow_def` foreign key
6. Workflow is now invokable like any other action

## Performance Considerations

### Optimizations
1. **Graph Caching**: Cache parsed task graphs per workflow definition
2. **Template Compilation**: Compile templates once, reuse for iterations
3. **Parallel Scheduling**: Schedule independent tasks concurrently
4. **Database Batching**: Batch task creation/updates when using with-items
5. **Context Serialization**: Use efficient JSON serialization for variable context

### Resource Limits
- Max workflow depth: 10 levels (prevent infinite recursion)
- Max tasks per workflow: 1000 (prevent resource exhaustion)
- Max iterations per with-items: 10,000 (configurable)
- Max parallel tasks: 100 (configurable)
- Variable context size: 10MB (prevent memory issues)

## Security Considerations

1. **Template Injection**: Sanitize all template inputs, no arbitrary code execution
2. **Variable Scoping**: Strict isolation between workflow executions
3. **Secret Access**: Only allow `kv.get_secret()` for authorized identities
4. **Resource Limits**: Enforce max task count, depth, iterations
5. **Audit Trail**: Log all workflow decisions, transitions, variable changes
6. **RBAC**: Workflow execution requires action execution permissions
7. **Input Validation**: Validate parameters against param_schema

## Monitoring & Observability

### Metrics to Track
- Workflow executions per second
- Average workflow duration
- Task execution duration (p50, p95, p99)
- Workflow success/failure rates
- Task retry counts
- Queue depth for workflow tasks
- Variable context size distribution

### Logging Standards
```
INFO  [workflow.start] execution=123 workflow=deploy_app version=1.0.0
INFO  [workflow.task.schedule] execution=123 task=build_image
INFO  [workflow.task.complete] execution=123 task=build_image duration=45s
INFO  [workflow.vars.publish] execution=123 vars=["image_uri"]
INFO  [workflow.task.schedule] execution=123 tasks=["deploy","health_check"]
WARN  [workflow.task.retry] execution=123 task=flaky_api attempt=2
ERROR [workflow.task.failed] execution=123 task=deploy_db error="connection_timeout"
INFO  [workflow.complete] execution=123 status=success duration=2m30s
```

### Distributed Tracing
- Propagate `trace_id` through entire workflow
- Link all task executions to parent workflow
- Enable end-to-end request tracing
- Integration with OpenTelemetry (future)

## Dependencies

### New Rust Crates
- **tera** (^1.19) - Template engine
- **petgraph** (^0.6) - Graph data structures and algorithms

### Existing Dependencies
- sqlx - Database access
- serde/serde_json - Serialization
- tokio - Async runtime
- lapin - RabbitMQ client

## Future Enhancements

### Short Term (3-6 months)
- Workflow versioning (multiple versions of same workflow)
- Workflow pausing/resuming with state persistence
- Advanced retry strategies (circuit breaker, adaptive)
- Workflow templates (reusable patterns)

### Medium Term (6-12 months)
- Dynamic workflows (generate graph at runtime)
- Workflow debugging tools (step-through execution)
- Performance analytics and optimization suggestions
- Workflow marketplace (share workflows)

### Long Term (12+ months)
- Visual workflow editor (drag-and-drop UI)
- AI-powered workflow generation
- Workflow optimization recommendations
- Multi-cloud orchestration patterns

## Success Criteria

This implementation will be considered successful when:

1. ✅ Workflows can be defined in YAML and registered via packs
2. ✅ Workflows execute reliably with all features working
3. ✅ Variables are properly scoped and templated across all scopes
4. ✅ Parallel execution works correctly with proper synchronization
5. ✅ Iteration handles lists efficiently with batching
6. ✅ Error handling and retry work as specified
7. ✅ Human-in-the-loop (inquiry) tasks integrate seamlessly
8. ✅ Nested workflows execute correctly
9. ✅ API provides full CRUD and control operations
10. ✅ Comprehensive tests cover all features
11. ✅ Documentation enables users to create workflows easily

## References

- Full design: `docs/workflow-orchestration.md`
- Simple example: `docs/examples/simple-workflow.yaml`
- Complex example: `docs/examples/complete-workflow.yaml`
- Migration SQL: `docs/examples/workflow-migration.sql`

## Next Steps

1. Review this plan with stakeholders
2. Prioritize features if timeline needs adjustment
3. Set up project tracking (GitHub issues/milestones)
4. Begin Phase 1 implementation
5. Schedule weekly progress reviews