# API Endpoint Changes: Ref-Only Access for Deployable Components

**Date:** 2025-01-27  
**Status:** ✅ Complete  
**Impact:** Breaking change for API consumers using ID-based endpoints

## Overview

Updated all API endpoints to use **reference identifiers (`ref`)** exclusively for accessing deployable components. Removed redundant ID-based alternative endpoints (`/id/{id}`) to enforce consistency and align with the architectural principle that deployable components should be accessed by portable reference strings, not database IDs.

## Motivation

- **Consistency**: Single, clear way to access deployable components
- **Portability**: Refs are portable across environments (dev, staging, prod)
- **Semantics**: Reference strings (e.g., `core.http.get`) are more meaningful than numeric IDs
- **Architectural alignment**: Enforces design principle that IDs are only for transient data (executions, events, enforcements)

## Changes Made

### 1. Code Changes (Phase 1)

Removed ID-based endpoints and their handler functions from:

**Files Modified:**
- `crates/api/src/routes/actions.rs`
  - Removed: `get_action_by_id()` function
  - Removed: `.route("/actions/id/{id}", get(get_action_by_id))`
  - Removed: Unused `FindById` import

- `crates/api/src/routes/triggers.rs`
  - Removed: `get_trigger_by_id()` function
  - Removed: `get_sensor_by_id()` function
  - Removed: `.route("/triggers/id/{id}", get(get_trigger_by_id))`
  - Removed: `.route("/sensors/id/{id}", get(get_sensor_by_id))`
  - Removed: Unused `FindById` import

- `crates/api/src/routes/rules.rs`
  - Removed: `get_rule_by_id()` function
  - Removed: `.route("/rules/id/{id}", get(get_rule_by_id))`
  - Removed: Unused `FindById` import

- `crates/api/src/routes/workflows.rs`
  - Removed: `get_workflow_by_id()` function
  - Removed: `.route("/workflows/id/{id}", get(get_workflow_by_id))`
  - Removed: Unused `FindById` import

- `crates/api/src/routes/packs.rs`
  - Removed: `get_pack_by_id()` function
  - Removed: `.route("/packs/id/{id}", get(get_pack_by_id))`

- `crates/api/src/openapi.rs`
  - Removed: OpenAPI path references for all deleted `*_by_id` endpoints
  - Updated: Test assertions for expected path/operation counts (62→57 paths, 86→81 operations)

**Total Removals:**
- 6 handler functions (~180 lines)
- 6 route registrations
- 6 OpenAPI path annotations
- 5 unused imports

### 2. Documentation Updates (Phase 2)

**Files Updated:**
- `docs/api-actions.md`
  - Removed: "Get Action by ID" section (35 lines)
  
- `docs/api-triggers-sensors.md`
  - Removed: "Get Trigger by ID" section (16 lines)
  - Removed: "Get Sensor by ID" section (16 lines)
  
- `docs/api-rules.md`
  - Removed: "Get Rule by ID" section (16 lines)
  
- `docs/api-packs.md`
  - Removed: "Get Pack by ID" section (21 lines)
  - Fixed: Section numbering (3→8 renumbered to 2→7)

**Total Documentation:**
- ~104 lines removed
- Section numbering corrected in pack documentation

### 3. Verification (Phase 3)

**Tests:**
- ✅ All unit tests pass (`cargo test -p attune-api --lib`)
- ✅ No test files reference deleted endpoints
- ✅ OpenAPI spec tests updated and passing
- ✅ Full project builds without errors

**API Impact Analysis:**
- ✅ No E2E test scripts use ID-based endpoints
- ✅ No web UI code references ID-based endpoints
- ✅ No shell scripts reference deleted endpoints

## API Changes

### Removed Endpoints

| Old Endpoint | Replacement |
|--------------|-------------|
| `GET /api/v1/actions/id/{id}` | `GET /api/v1/actions/{ref}` |
| `GET /api/v1/triggers/id/{id}` | `GET /api/v1/triggers/{ref}` |
| `GET /api/v1/sensors/id/{id}` | `GET /api/v1/sensors/{ref}` |
| `GET /api/v1/rules/id/{id}` | `GET /api/v1/rules/{ref}` |
| `GET /api/v1/workflows/id/{id}` | `GET /api/v1/workflows/{ref}` |
| `GET /api/v1/packs/id/{id}` | `GET /api/v1/packs/{ref}` |

### Unchanged Endpoints (Correct from the start)

These endpoints already correctly use `ref` as the primary identifier:

**Actions:**
- ✅ `GET /api/v1/actions/{ref}`
- ✅ `PUT /api/v1/actions/{ref}`
- ✅ `DELETE /api/v1/actions/{ref}`
- ✅ `GET /api/v1/packs/{pack_ref}/actions`

**Triggers:**
- ✅ `GET /api/v1/triggers/{ref}`
- ✅ `PUT /api/v1/triggers/{ref}`
- ✅ `DELETE /api/v1/triggers/{ref}`
- ✅ `POST /api/v1/triggers/{ref}/enable`
- ✅ `POST /api/v1/triggers/{ref}/disable`
- ✅ `GET /api/v1/packs/{pack_ref}/triggers`

**Sensors:**
- ✅ `GET /api/v1/sensors/{ref}`
- ✅ `PUT /api/v1/sensors/{ref}`
- ✅ `DELETE /api/v1/sensors/{ref}`
- ✅ `POST /api/v1/sensors/{ref}/enable`
- ✅ `POST /api/v1/sensors/{ref}/disable`
- ✅ `GET /api/v1/packs/{pack_ref}/sensors`
- ✅ `GET /api/v1/triggers/{trigger_ref}/sensors`

**Rules:**
- ✅ `GET /api/v1/rules/{ref}`
- ✅ `PUT /api/v1/rules/{ref}`
- ✅ `DELETE /api/v1/rules/{ref}`
- ✅ `POST /api/v1/rules/{ref}/enable`
- ✅ `POST /api/v1/rules/{ref}/disable`
- ✅ `GET /api/v1/packs/{pack_ref}/rules`
- ✅ `GET /api/v1/actions/{action_ref}/rules`
- ✅ `GET /api/v1/triggers/{trigger_ref}/rules`

**Workflows:**
- ✅ `GET /api/v1/workflows/{ref}`
- ✅ `PUT /api/v1/workflows/{ref}`
- ✅ `DELETE /api/v1/workflows/{ref}`
- ✅ `GET /api/v1/packs/{pack_ref}/workflows`

**Packs:**
- ✅ `GET /api/v1/packs/{ref}`
- ✅ `PUT /api/v1/packs/{ref}`
- ✅ `DELETE /api/v1/packs/{ref}`
- ✅ `POST /api/v1/packs/{ref}/test`
- ✅ `GET /api/v1/packs/{ref}/tests`
- ✅ `POST /api/v1/packs/{ref}/workflows/sync`
- ✅ `POST /api/v1/packs/{ref}/workflows/validate`

**Transient Resources (Correctly use ID):**
- ✅ `GET /api/v1/executions/{id}`
- ✅ `GET /api/v1/events/{id}`
- ✅ `GET /api/v1/enforcements/{id}`
- ✅ `GET /api/v1/inquiries/{id}`

## Migration Guide

### For API Consumers

If you were using ID-based endpoints, update your code:

**Before:**
```bash
# ❌ No longer works
GET /api/v1/actions/id/123
GET /api/v1/triggers/id/456
```

**After:**
```bash
# ✅ Use ref instead
GET /api/v1/actions/core.http.get
GET /api/v1/triggers/webhooks.github_push
```

**How to migrate:**
1. If you have an ID, first fetch the list and find the ref
2. Update your code to store and use refs instead of IDs
3. Use refs in all API calls for deployable components

### For Pack Developers

No changes needed! Pack manifests already use refs:

```yaml
# pack.yaml - already correct
actions:
  - ref: mypack.my_action  # ✅ ref-based
rules:
  - ref: mypack.my_rule    # ✅ ref-based
    action_ref: mypack.my_action  # ✅ ref-based
    trigger_ref: core.timer       # ✅ ref-based
```

## Benefits

1. **Simplified API**: One way to access each resource type
2. **Better DX**: Refs are human-readable (e.g., `core.http.get` vs `123`)
3. **Cross-environment**: Refs work across dev/staging/prod without changes
4. **Enforced architecture**: Clear distinction between deployable and transient resources
5. **Reduced code**: 180+ lines of redundant code removed

## Breaking Changes

⚠️ **This is a breaking change** for any consumers using the `/id/{id}` endpoints for:
- Actions
- Triggers
- Sensors
- Rules
- Workflows
- Packs

**Mitigation:** Since the project has no current users (early development), no migration path is needed.

## Testing

All automated tests continue to pass:
- ✅ Unit tests: 57 passed
- ✅ Route structure tests: All passing
- ✅ OpenAPI spec generation: Valid
- ✅ Full project build: Successful

## Next Steps

None required. The change is complete and verified.

## Related Documentation

- [API Actions Documentation](../docs/api-actions.md)
- [API Triggers & Sensors Documentation](../docs/api-triggers-sensors.md)
- [API Rules Documentation](../docs/api-rules.md)
- [API Packs Documentation](../docs/api-packs.md)
- [API Workflows Documentation](../docs/api-workflows.md)