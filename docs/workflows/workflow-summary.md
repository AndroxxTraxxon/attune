# Workflow Orchestration - Summary

## Overview

Attune's workflow orchestration system enables the composition of multiple actions into complex, conditional execution graphs. Workflows are themselves actions that can be triggered by rules, invoked by other workflows, or executed directly via API.

## Key Concepts

### 1. Workflows are Actions
- Workflows are first-class actions with `is_workflow=true`
- Can be triggered by rules (event-driven)
- Can invoke other workflows (composable)
- Can be executed directly via API

### 2. YAML-Based Definitions
- Workflows defined in `packs/{pack}/workflows/*.yaml`
- Declarative, version-controlled
- Parsed and stored as JSON in database
- Portable across environments

### 3. Task Graph Execution
- Each task is a node in an execution graph
- Tasks transition based on success/failure/completion
- Supports sequential, parallel, and conditional execution
- Fully asynchronous via message queue

### 4. Multi-Scope Variables

Variables accessible from 6 scopes (precedence order):

1. **`task.*`** - Results from completed tasks
   - `{{ task.build_image.result.image_uri }}`
   - `{{ task.health_check.status }}`

2. **`vars.*`** - Workflow-scoped variables
   - `{{ vars.deployment_id }}`
   - Set via `publish` directives

3. **`parameters.*`** - Input parameters
   - `{{ parameters.app_name }}`
   - `{{ parameters.environment }}`

4. **`pack.config.*`** - Pack configuration
   - `{{ pack.config.api_key }}`
   - `{{ pack.config.base_url }}`

5. **`system.*`** - System variables
   - `{{ system.execution_id }}`
   - `{{ system.timestamp }}`
   - `{{ system.identity.login }}`

6. **`kv.*`** - Key-value datastore
   - `{{ kv.get('feature.flags.enabled') }}`
   - `{{ kv.get_secret('api.token') }}`

## Core Features

### Sequential Execution
```yaml
tasks:
  - name: task1
    action: pack.action1
    on_success: task2
  - name: task2
    action: pack.action2
    on_success: task3
```

### Parallel Execution
```yaml
tasks:
  - name: parallel_checks
    type: parallel
    tasks:
      - name: check_database
        action: db.health_check
      - name: check_cache
        action: redis.ping
      - name: check_queue
        action: rabbitmq.status
    on_success: deploy_app
```

### Conditional Branching
```yaml
tasks:
  - name: check_environment
    action: core.noop
    decision:
      - when: "{{ parameters.environment == 'production' }}"
        next: require_approval
      - when: "{{ parameters.environment == 'staging' }}"
        next: run_tests
      - default: deploy_directly
```

### Iteration (with-items)
```yaml
tasks:
  - name: deploy_to_regions
    action: cloud.deploy
    with_items: "{{ parameters.regions }}"
    batch_size: 5  # Process 5 regions at a time
    input:
      region: "{{ item }}"
      version: "{{ parameters.version }}"
```

### Variable Publishing
```yaml
tasks:
  - name: create_deployment
    action: deployments.create
    input:
      app_name: "{{ parameters.app_name }}"
    publish:
      - deployment_id: "{{ task.create_deployment.result.id }}"
      - health_url: "{{ task.create_deployment.result.url }}/health"
```

### Error Handling & Retry
```yaml
tasks:
  - name: flaky_api_call
    action: http.post
    input:
      url: "{{ vars.api_endpoint }}"
    retry:
      count: 5
      delay: 10
      backoff: exponential
      max_delay: 60
    on_success: process_response
    on_failure: log_error_and_continue
```

### Human-in-the-Loop
```yaml
tasks:
  - name: require_approval
    action: core.inquiry
    input:
      prompt: "Approve production deployment?"
      schema:
        type: object
        properties:
          approved:
            type: boolean
      timeout: 3600  # 1 hour
    decision:
      - when: "{{ task.require_approval.result.approved == true }}"
        next: deploy_to_production
      - default: cancel_deployment
```

### Nested Workflows
```yaml
tasks:
  - name: provision_infrastructure
    action: infra.full_stack_workflow  # This is also a workflow
    input:
      environment: "{{ parameters.environment }}"
      region: "{{ parameters.region }}"
    on_success: deploy_application
```

## Template System

### Tera Template Engine
Jinja2-like syntax for variable interpolation:

```yaml
# String operations
message: "{{ parameters.app_name | upper | trim }}"

# List operations
first_region: "{{ parameters.regions | first }}"
region_count: "{{ parameters.regions | length }}"

# JSON operations
config: "{{ vars.json_string | from_json }}"

# Batching helper
batches: "{{ vars.large_list | batch(size=100) }}"

# Conditionals
status: "{{ vars.success ? 'deployed' : 'failed' }}"

# Key-value store
api_key: "{{ kv.get_secret('service.api_key') }}"
feature_enabled: "{{ kv.get('flags.new_feature', default=false) }}"
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Workflow Execution                  │
├─────────────────────────────────────────────────┤
│                                                  │
│  1. Rule/API triggers workflow action           │
│  2. Executor loads workflow definition           │
│  3. Create workflow_execution record             │
│  4. Initialize variable context                  │
│  5. Build task dependency graph                  │
│  6. Schedule initial tasks                       │
│  7. For each task:                               │
│     a. Template inputs using context             │
│     b. Create child execution                    │
│     c. Worker executes action                    │
│     d. Update task result                        │
│     e. Publish variables                         │
│     f. Evaluate transitions                      │
│     g. Schedule next tasks                       │
│  8. Complete when all tasks done                 │
│                                                  │
└─────────────────────────────────────────────────┘
```

## Database Schema

### New Tables

1. **`workflow_definition`** - Stores workflow YAML as JSON
2. **`workflow_execution`** - Tracks workflow runtime state
3. **`workflow_task_execution`** - Individual task executions

### Modified Tables

- **`action`** table: Add `is_workflow` and `workflow_def` columns

## API Endpoints

### Workflow Management
```
POST   /api/v1/packs/{pack_ref}/workflows          - Create workflow
GET    /api/v1/packs/{pack_ref}/workflows          - List workflows
GET    /api/v1/workflows/{workflow_ref}            - Get workflow
PUT    /api/v1/workflows/{workflow_ref}            - Update workflow
DELETE /api/v1/workflows/{workflow_ref}            - Delete workflow
POST   /api/v1/workflows/{workflow_ref}/execute    - Execute workflow
```

### Execution Management
```
GET    /api/v1/workflow-executions/{id}            - Get execution
GET    /api/v1/workflow-executions/{id}/tasks      - List tasks
GET    /api/v1/workflow-executions/{id}/graph      - Get graph
POST   /api/v1/workflow-executions/{id}/pause      - Pause
POST   /api/v1/workflow-executions/{id}/resume     - Resume
POST   /api/v1/workflow-executions/{id}/cancel     - Cancel
```

## Implementation Timeline

### Phase 1: Foundation (2 weeks)
- Database schema and migration
- Data models and repositories
- YAML parser
- Template engine integration

### Phase 2: Execution Engine (2 weeks)
- Task graph builder
- Workflow executor
- Message handlers
- State management

### Phase 3: Advanced Features (2 weeks)
- Iteration support (with-items)
- Parallel execution
- Retry logic
- Conditional branching

### Phase 4: API & Tools (2 weeks)
- Workflow CRUD endpoints
- Execution monitoring API
- Control operations
- Validation tools

### Phase 5: Testing & Docs (1 week)
- Comprehensive tests
- Example workflows
- User documentation

**Total: 9 weeks**

## Example Workflow Structure

```yaml
ref: my_pack.deploy_workflow
label: "Deploy Application"
description: "Deploys application with health checks"
version: "1.0.0"

parameters:
  app_name:
    type: string
    required: true
  version:
    type: string
    required: true
  environment:
    type: string
    enum: [dev, staging, production]

vars:
  deployment_id: null
  health_url: null

tasks:
  - name: create_deployment
    action: deployments.create
    input:
      app_name: "{{ parameters.app_name }}"
      version: "{{ parameters.version }}"
    publish:
      - deployment_id: "{{ task.create_deployment.result.id }}"
    on_success: build_image

  - name: build_image
    action: docker.build
    input:
      app: "{{ parameters.app_name }}"
      tag: "{{ parameters.version }}"
    on_success: deploy
    on_failure: cleanup

  - name: deploy
    action: kubernetes.deploy
    input:
      image: "{{ task.build_image.result.image }}"
    on_success: health_check
    on_failure: rollback

  - name: health_check
    action: http.get
    input:
      url: "{{ task.deploy.result.health_url }}"
    retry:
      count: 5
      delay: 10
    on_success: notify_success
    on_failure: rollback

  - name: rollback
    action: kubernetes.rollback
    on_complete: notify_failure

  - name: notify_success
    action: slack.post
    input:
      message: "✅ Deployed {{ parameters.app_name }} v{{ parameters.version }}"

  - name: notify_failure
    action: slack.post
    input:
      message: "❌ Deployment failed"

output_map:
  deployment_id: "{{ vars.deployment_id }}"
  status: "success"
```

## Pack Structure

```
packs/my_pack/
├── pack.yaml
├── config.yaml
├── actions/
│   ├── action1.py
│   └── action.yaml
├── sensors/
│   └── sensor.yaml
├── workflows/           # NEW
│   ├── deploy.yaml
│   ├── backup.yaml
│   └── rollback.yaml
└── rules/
    └── on_push.yaml
```

## Benefits

1. **Composability** - Build complex workflows from simple actions
2. **Reusability** - Share workflows across packs and organizations
3. **Maintainability** - YAML definitions are easy to read and version
4. **Observability** - Full execution tracking and tracing
5. **Flexibility** - Conditional logic, iteration, parallel execution
6. **Reliability** - Built-in retry, error handling, rollback
7. **Human Control** - Inquiry tasks for approval workflows
8. **Event-Driven** - Fully async, no blocking or polling

## Resources

- **Full Design**: `docs/workflow-orchestration.md`
- **Implementation Plan**: `docs/workflow-implementation-plan.md`
- **Simple Example**: `docs/examples/simple-workflow.yaml`
- **Complex Example**: `docs/examples/complete-workflow.yaml`
- **Migration SQL**: `docs/examples/workflow-migration.sql`
