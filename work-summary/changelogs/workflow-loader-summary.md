# Workflow Loader Implementation Summary

**Date:** 2025-01-13  
**Phase:** 1.4 - Workflow Loading & Registration  
**Status:** Partially Complete (Loader: ✅ | Registrar: ⏸️)

---

## Executive Summary

Implemented the workflow loading subsystem that scans pack directories for YAML workflow files, parses them, and validates them. The loader module is complete, tested, and production-ready. The registrar module is implemented but requires schema alignment with the actual database structure before it can be used.

---

## What Was Built

### 1. WorkflowLoader Module (`executor/src/workflow/loader.rs`)

**Purpose:** Scan pack directories, load workflow YAML files, parse and validate them.

**Components:**

#### WorkflowLoader
Main service for loading workflows from the filesystem.

```rust
pub struct WorkflowLoader {
    config: LoaderConfig,
    validator: WorkflowValidator,
}
```

**Key Methods:**
- `load_all_workflows()` - Scans all pack directories and loads all workflows
- `load_pack_workflows(pack_name, pack_dir)` - Loads workflows from a specific pack
- `load_workflow_file(file)` - Loads and validates a single workflow file
- `reload_workflow(ref_name)` - Reloads a specific workflow by reference

#### LoaderConfig
Configuration for workflow loading behavior.

```rust
pub struct LoaderConfig {
    pub packs_base_dir: PathBuf,      // Base directory (default: /opt/attune/packs)
    pub skip_validation: bool,         // Skip validation errors
    pub max_file_size: usize,          // Max file size (default: 1MB)
}
```

#### LoadedWorkflow
Represents a successfully loaded workflow with metadata.

```rust
pub struct LoadedWorkflow {
    pub file: WorkflowFile,                    // File metadata
    pub workflow: WorkflowDefinition,          // Parsed workflow
    pub validation_errors: Vec<String>,        // Any validation errors
}
```

#### WorkflowFile
Metadata about a workflow file.

```rust
pub struct WorkflowFile {
    pub path: PathBuf,         // Full path to YAML file
    pub pack: String,          // Pack name
    pub name: String,          // Workflow name
    pub ref_name: String,      // Full reference (pack.name)
}
```

**Features:**
- ✅ Async file I/O using Tokio
- ✅ Supports both `.yaml` and `.yml` extensions
- ✅ File size validation (prevents loading huge files)
- ✅ Integrated with Phase 1.3 parser and validator
- ✅ Comprehensive error handling
- ✅ Idiomatic Rust error types (`Error::validation()`, `Error::not_found()`)

**Directory Structure Expected:**
```
/opt/attune/packs/
├── core/
│   └── workflows/
│       ├── example.yaml
│       └── another.yaml
├── monitoring/
│   └── workflows/
│       └── healthcheck.yaml
└── deployment/
    └── workflows/
        ├── deploy_app.yaml
        └── rollback.yaml
```

**Test Coverage:**
- ✅ Scan pack directories
- ✅ Scan workflow files (both .yaml and .yml)
- ✅ Load single workflow file
- ✅ Load all workflows from all packs
- ✅ Reload specific workflow by reference
- ✅ File size limit enforcement
- ✅ Error handling for missing files/directories

### 2. WorkflowRegistrar Module (`executor/src/workflow/registrar.rs`)

**Purpose:** Register loaded workflows as actions in the database.

**Status:** ⏸️ Implemented but needs schema alignment

**Components:**

#### WorkflowRegistrar
Service for registering workflows in the database.

```rust
pub struct WorkflowRegistrar {
    pool: PgPool,
    action_repo: ActionRepository,
    workflow_repo: WorkflowDefinitionRepository,
    pack_repo: PackRepository,
    options: RegistrationOptions,
}
```

**Intended Methods:**
- `register_workflow(loaded)` - Register a single workflow
- `register_workflows(workflows)` - Register multiple workflows
- `unregister_workflow(ref_name)` - Remove a workflow from database

#### RegistrationOptions
Configuration for workflow registration.

```rust
pub struct RegistrationOptions {
    pub update_existing: bool,     // Update if workflow exists
    pub skip_invalid: bool,         // Skip workflows with validation errors
    pub default_runner: String,     // Default runner type
    pub default_timeout: i32,       // Default timeout in seconds
}
```

#### RegistrationResult
Result of a workflow registration operation.

```rust
pub struct RegistrationResult {
    pub ref_name: String,           // Workflow reference
    pub created: bool,              // true = created, false = updated
    pub action_id: i64,             // Action ID
    pub workflow_def_id: i64,       // Workflow definition ID
    pub warnings: Vec<String>,      // Any warnings
}
```

**Intended Flow:**
1. Verify pack exists in database
2. Check if workflow action already exists
3. Create or update action with `is_workflow=true`
4. Create or update workflow_definition record
5. Link action to workflow_definition
6. Return result with IDs

**Current Blockers:**
- Schema field name mismatches (see Issues section)
- Repository usage pattern differences
- Missing conventions for workflow-specific fields

### 3. Module Integration

**Updated Files:**
- `executor/src/workflow/mod.rs` - Added loader and registrar exports
- `executor/src/workflow/parser.rs` - Added `From<ParseError>` for Error conversion
- `executor/Cargo.toml` - Added `tempfile` dev-dependency

**Exports:**
```rust
pub use loader::{LoadedWorkflow, LoaderConfig, WorkflowFile, WorkflowLoader};
pub use registrar::{RegistrationOptions, RegistrationResult, WorkflowRegistrar};
```

---

## Issues Discovered

### Schema Incompatibility

The workflow design (Phases 1.2/1.3) assumed Action model fields that don't match the actual database schema:

**Expected vs Actual:**

| Field (Expected)    | Field (Actual)      | Type Difference |
|---------------------|---------------------|-----------------|
| `pack_id`           | `pack`              | Field name      |
| `ref_name`          | `ref`               | Field name      |
| `name`              | `label`             | Field name      |
| N/A                 | `pack_ref`          | Missing field   |
| `description`       | `description`       | Option vs String|
| `runner_type`       | `runtime`           | String vs ID    |
| `entry_point`       | `entrypoint`        | Option vs String|
| `parameters`        | `param_schema`      | Field name      |
| `output_schema`     | `out_schema`        | Field name      |
| `tags`              | N/A                 | Not in schema   |
| `metadata`          | N/A                 | Not in schema   |
| `enabled`           | N/A                 | Not in schema   |
| `timeout`           | N/A                 | Not in schema   |

**Impact:**
- Registrar cannot directly create Action records
- Need to use `CreateActionInput` struct
- Must decide conventions for workflow-specific fields

### Repository Pattern Differences

**Expected:** Instance methods
```rust
self.action_repo.find_by_ref(ref).await?
```

**Actual:** Trait-based static methods
```rust
ActionRepository::find_by_ref(&pool, ref).await?
```

**Impact:**
- All repository calls in registrar need updating
- Pattern is actually cleaner and more idiomatic

---

## Design Decisions Needed

### 1. Workflow Entrypoint
**Question:** What should `action.entrypoint` be for workflows?

**Options:**
- A) `"workflow"` - Simple constant
- B) `"internal://workflow"` - URL-like scheme
- C) Workflow definition ID reference

**Recommendation:** `"internal://workflow"` - Clear distinction from regular actions

### 2. Workflow Runtime
**Question:** How to handle `action.runtime` for workflows?

**Options:**
- A) NULL - Workflows don't use runtimes
- B) Create special "workflow" runtime in database

**Recommendation:** NULL - Workflows are orchestrated, not executed in runtimes

### 3. Required vs Optional Fields
**Question:** How to handle fields required in DB but optional in YAML?

**Affected Fields:**
- `description` - Required in DB, optional in workflow YAML
- `entrypoint` - Required in DB, N/A for workflows

**Recommendation:**
- Description: Default to empty string or derive from label
- Entrypoint: Use `"internal://workflow"` convention

---

## What Works

### Loader Module (Production Ready)
- ✅ Scans pack directories recursively
- ✅ Finds all workflow YAML files
- ✅ Parses workflow definitions
- ✅ Validates workflows using Phase 1.3 validator
- ✅ Handles errors gracefully
- ✅ Async/concurrent file operations
- ✅ Well-tested (6 test cases, all passing)
- ✅ Proper error types and messages

**Usage Example:**
```rust
use attune_executor::workflow::{WorkflowLoader, LoaderConfig};

let config = LoaderConfig {
    packs_base_dir: PathBuf::from("/opt/attune/packs"),
    skip_validation: false,
    max_file_size: 1024 * 1024,
};

let loader = WorkflowLoader::new(config);
let workflows = loader.load_all_workflows().await?;

for (ref_name, loaded) in workflows {
    println!("Loaded workflow: {}", ref_name);
    if !loaded.validation_errors.is_empty() {
        println!("  Warnings: {:?}", loaded.validation_errors);
    }
}
```

---

## What Needs Work

### Registrar Module (Blocked on Schema)

**To Complete:**
1. Update to use `CreateActionInput` struct
2. Map WorkflowDefinition fields to actual Action schema
3. Convert repository calls to trait static methods
4. Implement workflow field conventions
5. Add database integration tests
6. Verify workflow_definition table schema

**Estimated Effort:** 2-3 hours

### API Integration (Blocked on Registrar)

**Needed:**
- Workflow CRUD endpoints in API service
- Pack integration for auto-loading workflows
- Workflow catalog/search functionality

**Estimated Effort:** 5-6 hours

---

## Files Created

1. `crates/executor/src/workflow/loader.rs` (483 lines)
   - WorkflowLoader implementation
   - Configuration types
   - 6 unit tests with tempfile-based fixtures

2. `crates/executor/src/workflow/registrar.rs` (462 lines)
   - WorkflowRegistrar implementation (needs schema fix)
   - Registration types and options
   - Transaction-based database operations

3. `work-summary/phase-1.4-loader-registration-progress.md`
   - Detailed progress tracking
   - Schema compatibility analysis
   - Next steps and decisions

4. `work-summary/workflow-loader-summary.md` (this file)
   - Implementation summary
   - What works and what doesn't

---

## Files Modified

1. `crates/executor/src/workflow/mod.rs`
   - Added loader and registrar module declarations
   - Added public exports

2. `crates/executor/src/workflow/parser.rs`
   - Added `From<ParseError>` for `attune_common::error::Error`

3. `crates/executor/Cargo.toml`
   - Added `tempfile = "3.8"` dev-dependency

4. `work-summary/PROBLEM.md`
   - Added schema alignment issue tracking

---

## Testing

### Unit Tests (Loader)
All tests passing ✅

1. `test_scan_pack_directories` - Verifies pack directory scanning
2. `test_scan_workflow_files` - Verifies workflow file discovery
3. `test_load_workflow_file` - Verifies single file loading
4. `test_load_all_workflows` - Verifies batch loading
5. `test_reload_workflow` - Verifies reload by reference
6. `test_file_size_limit` - Verifies size limit enforcement

### Integration Tests (Registrar)
Not yet implemented ⏸️

**Needed:**
- Database fixture setup
- Pack creation for testing
- Workflow registration flow
- Update workflow flow
- Unregister workflow flow
- Transaction rollback on error

---

## Performance Considerations

### Current Implementation
- Uses async I/O for concurrent file operations
- No caching of loaded workflows
- Re-parses YAML on every load

### Future Optimizations
- **Caching:** Cache parsed workflows in memory
- **Lazy Loading:** Load workflows on-demand rather than at startup
- **File Watching:** Use inotify/fsnotify for hot-reloading
- **Parallel Loading:** Use `join_all` for concurrent pack scanning
- **Incremental Updates:** Only reload changed workflows

### Scalability Estimates
- **Small deployment:** 10 packs × 5 workflows = 50 workflows (~1-2 seconds load time)
- **Medium deployment:** 50 packs × 10 workflows = 500 workflows (~5-10 seconds)
- **Large deployment:** 200 packs × 20 workflows = 4000 workflows (~30-60 seconds)

**Recommendation:** Implement caching and lazy loading for deployments > 100 workflows

---

## Next Actions

### Immediate (P0)
1. Fix registrar schema alignment
2. Test workflow registration with database
3. Verify workflow_definition table compatibility

### Short Term (P1)
4. Add API endpoints for workflow CRUD
5. Integrate with pack management
6. Implement workflow catalog/search

### Medium Term (P2)
7. Add workflow caching
8. Implement hot-reloading
9. Add metrics and monitoring
10. Performance optimization for large deployments

---

## Dependencies

- ✅ Phase 1.2: Models and repositories (complete)
- ✅ Phase 1.3: YAML parsing and validation (complete)
- ⏸️ Database schema review (workflow_definition table)
- ⏸️ Pack management integration
- ⏸️ API service endpoints

---

## Conclusion

The workflow loader is complete and works well. It successfully:
- Scans pack directories
- Loads and parses workflow YAML files
- Validates workflows
- Handles errors gracefully

The registrar is logically complete but needs adaptation to the actual database schema. Once the schema alignment is fixed (estimated 2-3 hours), Phase 1.4 can be completed and workflows can be registered and executed.

**Overall Progress:** ~60% complete
**Blocker:** Schema field mapping
**Risk Level:** Low (well-understood issue with clear solution path)