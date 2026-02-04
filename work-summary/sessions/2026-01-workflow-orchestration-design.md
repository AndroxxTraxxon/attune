# Work Summary: Workflow Orchestration Design

**Date**: 2026-01-27  
**Session Focus**: Architecture design and planning for workflow orchestration feature  
**Status**: Design complete, Phase 1 implementation started (database migration complete)  

---

## Objective

Design a comprehensive workflow orchestration system for Attune that enables composing multiple actions into complex, conditional execution graphs with variable passing, iteration, and error handling.

---

## Accomplishments

### 1. Complete Technical Design ✅

Created `docs/workflow-orchestration.md` (1,063 lines):
- **Core Architecture**: Workflows as first-class actions
- **Execution Model**: Event-driven, graph-based task execution
- **Variable System**: 6-scope variable access (task, vars, parameters, pack.config, system, kv)
- **Template Engine**: Tera integration for Jinja2-like templating
- **Database Schema**: 3 new tables (workflow_definition, workflow_execution, workflow_task_execution)
- **Rust Implementation**: Detailed component specifications with code examples
- **Message Flow**: Integration with existing RabbitMQ infrastructure
- **Security & Performance**: Comprehensive considerations

### 2. Implementation Roadmap ✅

Created `docs/workflow-implementation-plan.md` (562 lines):
- **5-Phase Plan**: 9 weeks total implementation timeline
  - Phase 1: Foundation (2 weeks) - DB, models, parser, templates
  - Phase 2: Execution Engine (2 weeks) - Graph executor, state machine
  - Phase 3: Advanced Features (2 weeks) - Iteration, parallelism, retry
  - Phase 4: API & Tools (2 weeks) - CRUD endpoints, monitoring
  - Phase 5: Testing & Docs (1 week) - Comprehensive testing
- **API Specifications**: 16 REST endpoints defined
- **Testing Strategy**: Unit and integration test plans
- **Success Criteria**: 11 measurable outcomes

### 3. Executive Summary ✅

Created `docs/workflow-summary.md` (400 lines):
- Quick reference guide for stakeholders
- Feature overview with code examples
- Benefits and use cases
- Architecture diagrams
- Pack structure changes

### 4. Example Workflows ✅

**Simple Example** - `docs/examples/simple-workflow.yaml` (68 lines):
- Basic sequential workflow
- Variable publishing
- Template usage
- Easy learning resource

**Complex Example** - `docs/examples/complete-workflow.yaml` (710 lines):
- Production-ready deployment workflow
- All features demonstrated:
  - Sequential and parallel execution
  - Conditional branching (production approval)
  - Human-in-the-loop (inquiry tasks)
  - Iteration with batching (multi-region deployment)
  - Error handling and retry
  - Rollback procedures
  - Multiple notification paths
- Real-world scenario: CI/CD deployment pipeline

### 5. Database Migration ✅

Created `docs/examples/workflow-migration.sql` (225 lines):
- Complete schema for 3 new tables
- Indexes and triggers
- Helper views for monitoring
- Rollback support
- Comprehensive comments

### 6. TODO.md Integration ✅

Updated `work-summary/TODO.md`:
- Expanded Phase 8.1 (Workflow Orchestration) with detailed task breakdown
- 250 lines of granular implementation tasks across 5 phases
- Success criteria and deliverables for each phase
- Updated timeline estimates (Phase 8: 13-15 weeks total)
- Added workflow documentation references
- Updated project timeline to 9-12 months total

### 7. Database Migration Implementation ✅

Created and applied migration `migrations/20250127000002_workflow_orchestration.sql`:
- ✅ Created 3 new tables (workflow_definition, workflow_execution, workflow_task_execution)
- ✅ Modified action table with is_workflow and workflow_def columns
- ✅ Added all indexes for performance
- ✅ Added triggers for timestamp updates
- ✅ Created 3 helper views (workflow_execution_summary, workflow_task_detail, workflow_action_link)
- ✅ Applied migration successfully to database
- ✅ Verified all tables, columns, indexes, and views created correctly

---

## Key Design Decisions

### 1. Workflows as Actions
- **Decision**: Workflows are first-class actions with `is_workflow=true` flag
- **Rationale**: 
  - Can be triggered by rules (event-driven)
  - Can be invoked by other workflows (composable)
  - Can be executed directly via API
  - Leverages existing execution infrastructure
  - No special handling required in most of the system

### 2. YAML-Based Definitions
- **Decision**: Declarative YAML files in pack directories
- **Rationale**:
  - Human-readable and easy to author
  - Version-controllable (Git-friendly)
  - Industry standard (similar to GitHub Actions, Argo, etc.)
  - Portable across environments
  - Can be validated before execution

### 3. Event-Driven Execution
- **Decision**: Asynchronous execution via existing message queue
- **Rationale**:
  - Leverages existing RabbitMQ infrastructure
  - No blocking or polling required
  - Natural fit with Attune's architecture
  - Scalable and distributed
  - Each task is a regular execution with full tracing

### 4. Multi-Scope Variable System
- **Decision**: 6 scopes with precedence order
- **Rationale**:
  - Flexible variable access patterns
  - Clear precedence rules prevent confusion
  - Supports all common use cases:
    - Task results (`task.build.result.image_uri`)
    - Workflow state (`vars.deployment_id`)
    - User inputs (`parameters.app_name`)
    - Pack configuration (`pack.config.api_key`)
    - System context (`system.identity.login`)
    - Global state (`kv.get('feature.flags')`)

### 5. Tera Template Engine
- **Decision**: Use Tera crate for templating
- **Rationale**:
  - Jinja2-like syntax (familiar to users)
  - Compile-time template validation
  - Rust-native (no FFI overhead)
  - Extensible with custom filters/functions
  - Good performance with caching

---

## Core Features Designed

1. **Sequential Execution** - Tasks execute in order with transitions
2. **Parallel Execution** - Multiple tasks run concurrently
3. **Conditional Branching** - Decision trees based on task results
4. **Iteration (with-items)** - Process lists with optional batching
5. **Variable Publishing** - Tasks publish results to workflow scope
6. **Error Handling & Retry** - Built-in retry with exponential backoff
7. **Human-in-the-Loop** - Inquiry tasks for approvals
8. **Nested Workflows** - Workflows can invoke other workflows
9. **Timeout Management** - Per-task timeouts with handlers
10. **Pause/Resume** - Workflow execution control
11. **Cancellation** - Stop workflows mid-execution

---

## Database Schema Changes

### New Tables

1. **`workflow_definition`**
   - Stores parsed workflow YAML as JSON
   - Links to pack
   - Contains parameter/output schemas
   - Version and metadata

2. **`workflow_execution`**
   - Runtime state of workflow
   - Variable context (JSONB)
   - Task completion tracking (arrays)
   - Status and error information

3. **`workflow_task_execution`**
   - Individual task execution records
   - Supports iteration (task_index, task_batch)
   - Retry tracking
   - Result and error storage

### Modified Tables

- **`action`** table:
  - Add `is_workflow BOOLEAN` column
  - Add `workflow_def BIGINT` foreign key

---

## API Endpoints Designed

### Workflow Management (8 endpoints)
- Create, list, get, update, delete workflows
- Execute workflow directly
- Validate workflow definition

### Execution Management (7 endpoints)
- List, get workflow executions
- Get task list, graph, context
- Pause, resume, cancel, retry workflows

---

## Implementation Timeline

**Total: 9 weeks**

- **Week 1-2**: Foundation (DB, models, parser, templates)
- **Week 3-4**: Execution Engine (graph, executor, messages)
- **Week 5-6**: Advanced Features (iteration, parallel, retry)
- **Week 7-8**: API & Tools (endpoints, validation, pack integration)
- **Week 9**: Testing & Documentation

---

## Dependencies

### New Crates Required
- **tera** (^1.19) - Template engine
- **petgraph** (^0.6) - Graph data structures (optional, can use custom)

### Existing Crates
- sqlx - Database access
- serde/serde_json - Serialization
- tokio - Async runtime
- lapin - RabbitMQ client

---

## Success Metrics

The workflow implementation will be considered successful when:

1. ✅ Workflows defined in YAML and registered via packs
2. ✅ All execution patterns work (sequential, parallel, conditional, iteration)
3. ✅ Variables properly scoped and templated
4. ✅ Error handling and retry functional
5. ✅ Human-in-the-loop integration seamless
6. ✅ Nested workflows execute correctly
7. ✅ API provides full CRUD and control
8. ✅ Comprehensive test coverage
9. ✅ Documentation enables easy workflow authoring
10. ✅ Performance meets requirements (graph caching, template caching)
11. ✅ Security considerations addressed (template injection, resource limits)

---

## Documentation Deliverables

All documentation complete and ready for implementation:

1. **Technical Design**: `docs/workflow-orchestration.md`
2. **Implementation Plan**: `docs/workflow-implementation-plan.md`
3. **Executive Summary**: `docs/workflow-summary.md`
4. **Simple Example**: `docs/examples/simple-workflow.yaml`
5. **Complex Example**: `docs/examples/complete-workflow.yaml`
6. **Migration Script**: `docs/examples/workflow-migration.sql`
7. **TODO Integration**: `work-summary/TODO.md` updated with 250 lines of tasks

---

## Next Steps

1. **Review & Feedback**: Present design to stakeholders
2. **Prioritization**: Confirm 9-week timeline or adjust scope
3. **Project Setup**: Create GitHub issues/milestones for 5 phases
4. **Begin Phase 1**: Start with database migration and models
5. **Weekly Reviews**: Schedule progress check-ins

---

## References

- **Full Design**: `docs/workflow-orchestration.md` (1,063 lines)
- **Implementation Plan**: `docs/workflow-implementation-plan.md` (562 lines)
- **Quick Reference**: `docs/workflow-summary.md` (400 lines)
- **Examples**: `docs/examples/simple-workflow.yaml`, `docs/examples/complete-workflow.yaml`
- **Migration**: `docs/examples/workflow-migration.sql` (225 lines)
- **Task List**: `work-summary/TODO.md` Phase 8.1

---

## Notes

- This is a significant feature addition that will position Attune as a comprehensive automation platform comparable to StackStorm, Argo Workflows, and AWS Step Functions
- The design leverages existing infrastructure (message queue, execution model) minimizing architectural changes
- YAML-based approach is industry-standard and user-friendly
- Implementation can proceed incrementally - basic workflows first, advanced features later
- All design decisions documented with rationale for future reference

---

## Implementation Progress

### Phase 1: Foundation (Week 1-2) - IN PROGRESS

#### Completed Tasks ✅
1. **Database Migration** (2026-01-27)
   - Created migration file: `migrations/20250127000002_workflow_orchestration.sql`
   - Applied migration successfully
   - Verified schema:
     - ✅ `workflow_definition` table (14 columns)
     - ✅ `workflow_execution` table with status tracking
     - ✅ `workflow_task_execution` table with retry/timeout support
     - ✅ `action` table modifications (is_workflow, workflow_def)
     - ✅ All indexes created
     - ✅ All triggers created
     - ✅ 3 helper views created

#### Next Tasks 📋
2. Add workflow models to `common/src/models.rs`
3. Create workflow repositories
4. Implement YAML parser
5. Integrate template engine

---

**Status**: ✅ Design complete. 🔄 Phase 1 implementation started (database migration complete).