# Fix: Pack Installation Virtualenv Ordering & FK ON DELETE Constraints

**Date:** 2026-02-05

## Problems

### 1. Virtualenv Not Created at Permanent Location

When installing a Python pack (e.g., `python_example`), no virtualenv was created at the permanent storage location. Attempting to run an action yielded:

```json
{
  "error": "Execution failed during preparation",
  "succeeded": false
}
```

### 2. Pack Deletion Blocked by Foreign Key Constraints

Deleting a pack that had been used (with executions) failed with:

```json
{
  "error": "Constraint violation: execution_action_fkey",
  "code": "CONFLICT"
}
```

## Root Causes

### Virtualenv Ordering Bug

In `install_pack` (`crates/api/src/routes/packs.rs`), the operation ordering was incorrect:

1. Pack downloaded to temp directory (`/tmp/attune-pack-installs/...`)
2. `register_pack_internal(temp_path)` called — creates DB record **and sets up virtualenv at temp path**
3. `storage.install_pack()` copies pack from temp to permanent storage (`packs/{pack_ref}/`)
4. Temp directory cleaned up

Python virtualenvs are **not relocatable** — they contain hardcoded paths in shebang lines, `pyvenv.cfg`, and pip scripts. The copied `.venv` was non-functional.

### Missing ON DELETE Clauses on Foreign Keys

Several foreign key constraints in the schema had no `ON DELETE` behavior (defaulting to `RESTRICT`), which blocked cascading deletes:

- `execution.action` → `action(id)` — **no ON DELETE** (blocks action deletion)
- `execution.parent` → `execution(id)` — **no ON DELETE**
- `execution.enforcement` → `enforcement(id)` — **no ON DELETE**
- `rule.action` → `action(id)` — **no ON DELETE**, also `NOT NULL`
- `rule.trigger` → `trigger(id)` — **no ON DELETE**, also `NOT NULL`
- `event.source` → `sensor(id)` — **no ON DELETE**
- `workflow_execution.workflow_def` → `workflow_definition(id)` — **no ON DELETE**

When deleting a pack, the cascade deleted actions (`action.pack ON DELETE CASCADE`), but executions referencing those actions blocked the delete.

## Fixes

### 1. Pack Installation Ordering

Restructured `install_pack` to move the pack to permanent storage **before** calling `register_pack_internal`:

1. Pack downloaded to temp directory
2. `pack.yaml` read to extract `pack_ref`
3. **Pack moved to permanent storage** (`packs/{pack_ref}/`)
4. `register_pack_internal(permanent_path)` called — virtualenv creation and dependency installation now happen at the final location
5. Temp directory cleaned up

Added error handling to clean up permanent storage if registration fails after the move.

### 2. Foreign Key ON DELETE Fixes (Merged into Original Migrations)

Fixed all missing ON DELETE behaviors directly in the original migration files (requires DB rebuild):

| Table.Column | Migration File | ON DELETE | Notes |
|---|---|---|---|
| `execution.action` | `000006_execution_system` | `SET NULL` | Already nullable; `action_ref` text preserved |
| `execution.parent` | `000006_execution_system` | `SET NULL` | Already nullable |
| `execution.enforcement` | `000006_execution_system` | `SET NULL` | Already nullable |
| `rule.action` | `000006_execution_system` | `SET NULL` | Made nullable; `action_ref` text preserved |
| `rule.trigger` | `000006_execution_system` | `SET NULL` | Made nullable; `trigger_ref` text preserved |
| `event.source` | `000004_trigger_sensor_event_rule` | `SET NULL` | Already nullable; `source_ref` preserved |
| `workflow_execution.workflow_def` | `000007_workflow_system` | `CASCADE` | Meaningless without definition |

### 3. Model & Code Updates

- **Rule model** (`crates/common/src/models.rs`): Changed `action: Id` and `trigger: Id` to `Option<Id>`
- **RuleResponse DTO** (`crates/api/src/dto/rule.rs`): Changed `action` and `trigger` to `Option<i64>`
- **Enforcement processor** (`crates/executor/src/enforcement_processor.rs`): Added guards to skip execution when a rule's action or trigger has been deleted (SET NULL)
- **Pack delete endpoint** (`crates/api/src/routes/packs.rs`): Added filesystem cleanup to remove pack directory from permanent storage on deletion

### 4. Test Updates

- `crates/common/tests/rule_repository_tests.rs`: Updated assertions to use `Some(id)` for nullable fields
- `crates/executor/src/enforcement_processor.rs` (tests): Updated test Rule construction with `Some()` wrappers

## Files Changed

- `migrations/20250101000004_trigger_sensor_event_rule.sql` — Added `ON DELETE SET NULL` to `event.source`
- `migrations/20250101000006_execution_system.sql` — Added `ON DELETE SET NULL` to `execution.action`, `.parent`, `.enforcement`; made `rule.action`/`.trigger` nullable with `ON DELETE SET NULL`
- `migrations/20250101000007_workflow_system.sql` — Added `ON DELETE CASCADE` to `workflow_execution.workflow_def`
- `crates/api/src/routes/packs.rs` — Reordered `install_pack`; added pack directory cleanup on delete
- `crates/api/src/dto/rule.rs` — Made `action`/`trigger` fields optional in `RuleResponse`
- `crates/common/src/models.rs` — Made `Rule.action`/`Rule.trigger` `Option<Id>`
- `crates/executor/src/enforcement_processor.rs` — Handle nullable action/trigger in enforcement processing
- `crates/common/tests/rule_repository_tests.rs` — Fixed test assertions

## Design Philosophy

Historical records (executions, events, enforcements) are preserved when their referenced entities are deleted. The text ref fields (`action_ref`, `trigger_ref`, `source_ref`, etc.) retain the reference for auditing, while the FK ID fields are set to NULL. Rules with deleted actions or triggers become non-functional but remain in the database for traceability.

## Verification

- `cargo check --all-targets --workspace` — zero warnings
- `cargo test --workspace --lib` — all 358 unit tests pass