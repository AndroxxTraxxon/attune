# Phase 1.4: Workflow Loading & Registration - Progress Summary

**Date:** 2025-01-13  
**Status:** Complete - Schema Alignment Fixed  
**Completion:** 100%

---

## Overview

Phase 1.4 implements workflow loading from pack directories and registration as actions in the database. This phase bridges the gap between YAML workflow definitions and the Attune execution system.

---

## Completed Work

### 1. Workflow Loader Module (`executor/src/workflow/loader.rs`)

✅ **Implemented:**
- `WorkflowLoader` - Main loader for scanning and parsing workflows
- `LoadedWorkflow` - Represents a loaded workflow with validation results
- `WorkflowFile` - Metadata about workflow YAML files
- `LoaderConfig` - Configuration for loader behavior

**Features:**
- Scans pack directories for workflow YAML files (`.yaml` and `.yml` extensions)
- Parses workflows using the YAML parser from Phase 1.3
- Validates workflows and collects validation errors
- Supports file size limits and validation skipping
- Async file I/O with Tokio
- Comprehensive error handling using `Error::validation()`

**Key Methods:**
- `load_all_workflows()` - Scan all packs and load workflows
- `load_pack_workflows()` - Load workflows from a specific pack
- `load_workflow_file()` - Load and validate a single workflow file
- `reload_workflow()` - Reload a workflow by reference name

**Tests:**
- ✅ Scan pack directories
- ✅ Scan workflow files
- ✅ Load workflow file
- ✅ Load all workflows
- ✅ Reload workflow
- ✅ File size limit enforcement

### 2. Workflow Registrar Module (`executor/src/workflow/registrar.rs`)

✅ **Implemented:**
- `WorkflowRegistrar` - Registers workflows in database
- `RegistrationOptions` - Configuration for registration behavior
- `RegistrationResult` - Result of workflow registration

**Features:**
- Register workflows as workflow_definition records
- Store complete workflow YAML as JSON in definition field
- Update existing workflows
- Unregister workflows and clean up database
- Uses repository trait pattern correctly

**Status:** Complete and compiling

### 3. Module Exports

✅ Updated `executor/src/workflow/mod.rs` to export:
- `WorkflowLoader`, `LoadedWorkflow`, `LoaderConfig`, `WorkflowFile`
- `WorkflowRegistrar`, `RegistrationOptions`, `RegistrationResult`

### 4. Dependencies

✅ Added to `executor/Cargo.toml`:
- `tempfile = "3.8"` (dev-dependency for tests)

---

## Issues Discovered

### Schema Incompatibility

The workflow orchestration design (from Phase 1.2) assumed different database schema fields than what actually exists:

**Expected (from workflow design):**
```rust
Action {
    pack_id: i64,
    ref_name: String,
    name: String,
    description: Option<String>,
    runner_type: String,
    enabled: bool,
    entry_point: Option<String>,
    parameters: JsonValue,
    output_schema: Option<JsonValue>,
    tags: Vec<String>,
    metadata: Option<JsonValue>,
    is_workflow: bool,
    workflow_def: Option<i64>,
    timeout: Option<i32>,
}
```

**Actual (from migrations):**
```rust
Action {
    id: i64,
    ref: String,           // NOT ref_name
    pack: i64,             // NOT pack_id
    pack_ref: String,      // Additional field
    label: String,         // NOT name
    description: String,   // NOT Option<String>
    entrypoint: String,    // NOT Option<String>
    runtime: Option<i64>,  // NOT runner_type
    param_schema: Option<JsonSchema>,
    out_schema: Option<JsonSchema>,
    is_workflow: bool,
    workflow_def: Option<i64>,
}
```

### Repository Pattern Differences

**Expected:** Instance methods on repository structs
```rust
self.action_repo.find_by_ref(ref).await?
self.action_repo.delete(id).await?
```

**Actual:** Trait-based static methods
```rust
ActionRepository::find_by_ref(&pool, ref).await?
ActionRepository::delete(&pool, id).await?
```

---

## Completed Changes

### 1. Updated Registrar for Actual Schema ✅

Modified `workflow/registrar.rs`:

- ✅ Use `CreateWorkflowDefinitionInput` for workflow creation
- ✅ Discovered workflows are NOT stored as actions initially
- ✅ Workflows stored in `workflow_definition` table with full YAML as JSON
- ✅ Map workflow fields to workflow_definition schema:
  - `workflow.ref` → `workflow_definition.ref`
  - Pack ID from PackRepository lookup → `workflow_definition.pack`
  - Pack ref → `workflow_definition.pack_ref`
  - `workflow.label` → `workflow_definition.label`
  - `workflow.description` → `workflow_definition.description`
  - `workflow.parameters` → `workflow_definition.param_schema`
  - `workflow.output` → `workflow_definition.out_schema`
  - Complete workflow as JSON → `workflow_definition.definition`

### 2. Fixed Repository Usage ✅

- ✅ Replaced instance method calls with trait static methods
- ✅ Pass `&self.pool` as executor to all repository methods
- ✅ Use `Create`, `Update`, `Delete`, `FindByRef` traits correctly
- ✅ Proper type annotations for trait method calls

### 3. Resolved Schema Understanding ✅

**Key Discovery:**
- Workflows are stored in `workflow_definition` table, NOT as actions initially
- Actions can optionally link to workflows via `is_workflow` and `workflow_def` columns
- For Phase 1.4, we only create workflow_definition records
- Action creation for workflows will be handled in later phases

### 4. WorkflowDefinition Storage ✅

- ✅ Verified workflow_definition table structure matches model
- ✅ Complete workflow serialized to JSON and stored in `definition` field
- ✅ Task serialization format compatible (stored as part of definition JSON)
- ✅ Vars and output stored in both dedicated columns and definition JSON

---

## Testing Status

### Loader Tests
- ✅ All loader tests passing
- ✅ File system operations work correctly
- ✅ Error handling validated

### Registrar Tests
- ✅ Basic unit tests passing (2 tests)
- ⏸️ Database integration tests not yet implemented (requires database setup)
- ⏸️ Transaction rollback tests needed (future work)

---

## Next Steps

### Completed (Schema Alignment) ✅

1. **Reviewed workflow_definition table schema** ✅
   - Confirmed table structure matches model
   - Workflow stored as JSON in `definition` field
   - Separate columns for ref, pack, pack_ref, label, description, version, etc.

2. **Updated to use CreateWorkflowDefinitionInput** ✅
   - Serialize complete workflow to JSON
   - Store in workflow_definition table directly
   - No action creation needed in this phase

3. **Fixed registrar repository calls** ✅
   - Converted all to trait static methods
   - Updated error handling with Error::validation() and Error::not_found()
   - Proper type annotations

4. **Resolved entrypoint and runtime questions** ✅
   - Not applicable - workflows stored separately from actions
   - Actions can link to workflows in future phases
   - No entrypoint/runtime needed for workflow_definition records

### API Integration (After Schema Fix)

5. **Add workflow API endpoints** (`api/src/handlers/workflow.rs`):
   - `GET /api/v1/workflows` - List workflows
   - `GET /api/v1/workflows/:ref` - Get workflow by ref
   - `POST /api/v1/workflows` - Create/upload workflow
   - `PUT /api/v1/workflows/:ref` - Update workflow
   - `DELETE /api/v1/workflows/:ref` - Delete workflow
   - `GET /api/v1/packs/:pack/workflows` - List workflows in pack

6. **Pack integration**
   - Update pack loader to discover workflows
   - Register workflows during pack installation
   - Unregister during pack removal

7. **Workflow catalog**
   - Search/filter workflows by tags, pack, etc.
   - List workflow versions
   - Show workflow metadata and tasks

---

## Files Created/Modified

### Created
- `crates/executor/src/workflow/loader.rs` (483 lines)
- `crates/executor/src/workflow/registrar.rs` (462 lines)
- `work-summary/phase-1.4-loader-registration-progress.md` (this file)

### Modified
- `crates/executor/src/workflow/mod.rs` - Added loader/registrar exports
- `crates/executor/src/workflow/parser.rs` - Added `From<ParseError>` for Error
- `crates/executor/Cargo.toml` - Added tempfile dev-dependency

---

## Dependencies on Other Work

- ✅ Phase 1.2: Models and repositories (complete)
- ✅ Phase 1.3: YAML parsing and validation (complete)
- ⏸️ Runtime system: Need workflow runtime or convention
- ⏸️ Pack management: Integration for auto-loading workflows

---

## Notes

### Design Decisions Needed

1. **Workflow Entrypoint**: What should this be?
   - Option A: `"workflow"` (simple constant)
   - Option B: `"internal://workflow"` (URL-like scheme)
   - Option C: Reference to workflow definition ID
   - **Recommendation:** Use `"internal://workflow"` to distinguish from regular actions

2. **Workflow Runtime**: How to handle?
   - Option A: NULL (workflows don't use runtimes like actions do)
   - Option B: Create special "workflow" runtime in database
   - **Recommendation:** NULL since workflows are orchestrated, not executed in runtimes

3. **Description Field**: Required in DB, optional in YAML
   - Use empty string as default? Or derive from label?
   - **Recommendation:** Default to empty string if not provided

### Observations

- The loader is well-tested and production-ready
- The registrar logic is sound but needs schema adaptation
- Repository trait pattern is cleaner than instance methods
- Error handling with `Error::validation()` and `Error::not_found()` is more idiomatic

### Performance Considerations

- Loading all workflows at startup could be slow for large deployments
- Consider lazy loading or background workflow discovery
- Cache loaded workflows in memory to avoid re-parsing
- Use file system watchers for hot-reloading during development

---

## Completed Work Summary

- ✅ Schema alignment: 3 hours (COMPLETE)
- ⏸️ API endpoints: 3-4 hours (Phase 1.5)
- ⏸️ Pack integration: 2-3 hours (Phase 1.5)
- ⏸️ Database integration testing: 2-3 hours (Phase 1.5)
- ✅ Documentation: 2 hours (COMPLETE)

**Phase 1.4 Status:** COMPLETE
**Next Phase:** 1.5 - API Integration

---

## References

- `docs/workflow-orchestration.md` - Original design
- `docs/workflow-models-api.md` - Models API reference
- `migrations/20250101000004_execution_system.sql` - Actual schema
- `crates/common/src/repositories/action.rs` - Repository pattern
- `crates/common/src/repositories/workflow.rs` - Workflow repositories

## Compilation Status

**Final Build:** ✅ SUCCESS

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.50s
```

**Tests:** ✅ ALL PASSING

```
running 30 tests
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured
```

- 6 loader tests passing
- 2 registrar tests passing
- 6 parser tests passing
- 10 template tests passing
- 6 validator tests passing

**Warnings:** Only dead code warnings for unused methods (expected)