# StackStorm-Style Parameter Schema Migration

**Date**: 2026-02-22

## Summary

Migrated `param_schema` format from standard JSON Schema to StackStorm-style flat parameter maps with `required` and `secret` inlined per-parameter. This makes parameter definitions more readable and eliminates the clunky top-level `required` array pattern from JSON Schema.

## Format Change

### Before (JSON Schema)
```yaml
parameters:
  type: object
  properties:
    url:
      type: string
      description: "Target URL"
    token:
      type: string
      secret: true
  required:
    - url
```

### After (StackStorm-style)
```yaml
parameters:
  url:
    type: string
    description: "Target URL"
    required: true
  token:
    type: string
    secret: true
```

The `type: object` / `properties:` wrapper is removed. `required` moves from a top-level array to an inline boolean per-parameter. `secret` was already inline and remains unchanged.

## Scope

- **`param_schema`** (action, trigger, sensor, workflow parameters): Converted to StackStorm-style
- **`out_schema`** (output schemas): Left as standard JSON Schema — `required`/`secret` are not meaningful for outputs
- **Database**: No migration needed — columns are JSONB, the JSON shape just changes
- **Backward compatibility**: Web UI `extractProperties()` handles both formats during transition

## Files Modified

### Pack YAML Files (13 files)
- `packs/core/actions/echo.yaml`
- `packs/core/actions/sleep.yaml`
- `packs/core/actions/noop.yaml`
- `packs/core/actions/http_request.yaml`
- `packs/core/actions/download_packs.yaml`
- `packs/core/actions/register_packs.yaml`
- `packs/core/actions/build_pack_envs.yaml`
- `packs/core/actions/get_pack_dependencies.yaml`
- `packs/core/triggers/intervaltimer.yaml`
- `packs/core/triggers/crontimer.yaml`
- `packs/core/triggers/datetimetimer.yaml`
- `packs/core/workflows/install_packs.yaml`
- `packs/examples/actions/list_example.yaml`

### Web UI (7 files)
- `web/src/components/common/ParamSchemaForm.tsx` — New `ParamSchemaProperty` interface with inline `required`/`secret`/`position`, new exported `extractProperties()` utility, updated `validateParamSchema()` logic
- `web/src/components/common/ParamSchemaDisplay.tsx` — Imports shared `extractProperties`, removed duplicate type definitions
- `web/src/components/common/ExecuteActionModal.tsx` — Uses shared `extractProperties` for parameter initialization
- `web/src/components/common/SchemaBuilder.tsx` — Produces StackStorm-style flat format, added Secret checkbox, handles both formats on input
- `web/src/components/forms/TriggerForm.tsx` — Updated empty-schema check for flat format
- `web/src/pages/actions/ActionsPage.tsx` — Uses `extractProperties`, added Secret badges
- `web/src/pages/triggers/TriggersPage.tsx` — Uses `extractProperties`, added Secret badges

### API DTOs (3 files)
- `crates/api/src/dto/action.rs` — Updated OpenAPI examples and doc comments
- `crates/api/src/dto/trigger.rs` — Updated OpenAPI examples and doc comments
- `crates/api/src/dto/workflow.rs` — Updated OpenAPI examples and doc comments

### Documentation
- `AGENTS.md` — Added Parameter Schema Format documentation in Pack File Loading section

## Key Design Decisions

1. **Shared `extractProperties()` utility**: Single exported function in `ParamSchemaForm.tsx` handles both StackStorm-style and legacy JSON Schema formats. All consumers import from one place instead of duplicating logic.

2. **Backward compatibility in Web UI**: The `extractProperties()` function detects the old format (presence of `type: "object"` + `properties` wrapper) and normalizes it to the flat format, merging the top-level `required` array into per-parameter `required: true` flags. This means existing database records in the old format will still render correctly.

3. **No Rust model changes needed**: `param_schema` is stored as `Option<JsonValue>` (aliased as `JsonSchema`). The Rust code doesn't deeply inspect the schema structure — it passes it through as opaque JSONB. The format change is transparent to the backend.

4. **Pack loaders unchanged**: Both `loader.rs` and `load_core_pack.py` read `data.get("parameters")` and serialize it to JSONB as-is. Since we changed the YAML format, the stored format automatically changes to match.

## Verification

- Rust: `cargo check --all-targets --workspace` — zero warnings
- Rust: `cargo test --workspace --lib` — 82 tests passed
- TypeScript: `npx tsc --noEmit` — clean
- Vite: `npx vite build` — successful production build