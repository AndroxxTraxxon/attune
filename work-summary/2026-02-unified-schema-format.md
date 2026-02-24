# Unified Schema Format: Flat Format for All Schema Types

**Date**: 2026-02-05

## Summary

Unified all schema types (`param_schema`, `out_schema`, `conf_schema`) to use the same flat StackStorm-style format with inline `required` and `secret` per parameter. Previously, `param_schema` used flat format while `out_schema` and `conf_schema` used standard JSON Schema (`{ type: "object", properties: { ... }, required: [...] }`). This inconsistency prevented features like `secret` badges from working on output and configuration schemas.

## Motivation

- No reason for `conf_schema` and `out_schema` to use a different format than `param_schema`
- Users should be able to mark `secret` and `required` inline on any schema type
- Eliminates dual-format shim logic in the web UI (`extractProperties` backward compatibility branch)
- Project is pre-production — no data migration needed, just adjust configurations

## Changes

### Pack YAML Files (12 files)

Converted all top-level `type: object` + `properties` wrappers to flat format, moving `required` array entries inline:

- `packs/core/pack.yaml` — `conf_schema`
- `packs/examples/pack.yaml` — `conf_schema`
- `packs/core/sensors/interval_timer_sensor.yaml` — `parameters`
- `packs/core/triggers/intervaltimer.yaml` — `output`
- `packs/core/triggers/crontimer.yaml` — `output`
- `packs/core/triggers/datetimetimer.yaml` — `output`
- `packs/core/actions/http_request.yaml` — `output_schema`
- `packs/core/actions/build_pack_envs.yaml` — `output_schema`
- `packs/core/actions/download_packs.yaml` — `output_schema`
- `packs/core/actions/get_pack_dependencies.yaml` — `output_schema`
- `packs/core/actions/register_packs.yaml` — `output_schema`
- `packs/core/workflows/install_packs.yaml` — `output_schema`
- `packs/examples/actions/list_example.yaml` — `output_schema`

Nested structures (e.g., `items: { type: object, properties: { ... } }` within array parameters) remain unchanged — only the top-level wrapper was converted.

### Web UI (6 files)

- **`ParamSchemaForm.tsx`** — Removed legacy JSON Schema branch from `extractProperties()`. Removed `extractJsonSchemaProperties()` (no longer needed). Single `extractProperties()` handles all schema types.
- **`ParamSchemaDisplay.tsx`** — Updated doc comment, tightened `schema` prop type from `ParamSchema | any` to `ParamSchema`.
- **`SchemaBuilder.tsx`** — Removed legacy JSON Schema reading from both `useEffect` initializer and `handleRawJsonChange`. Only reads/writes flat format.
- **`PackForm.tsx`** — Updated `confSchema` initial state from JSON Schema to `{}`. Updated `hasSchemaProperties` check (no longer looks for `.properties` sub-key). Updated config sync logic, validation, schema examples (API/Database/Webhook examples now use flat format with `secret` and `required` inline), and `ParamSchemaForm` pass-through (passes `confSchema` directly instead of `confSchema.properties`).
- **`TriggerForm.tsx`** — Updated `paramSchema` and `outSchema` initial states from JSON Schema to `{}`.
- **`TriggersPage.tsx`** — Uses `extractProperties()` for both `param_schema` and `out_schema`.

### Backend Rust (5 files)

- **`crates/api/src/validation/params.rs`** — Added `flat_to_json_schema()` function that converts flat format to JSON Schema internally before passing to `jsonschema::Validator`. Updated `validate_trigger_params()` and `validate_action_params()` to call the converter. Converted all 29 test schemas from JSON Schema to flat format. Added 4 unit tests for `flat_to_json_schema()` and 1 test for secret field validation.
- **`crates/api/src/dto/action.rs`** — Updated `out_schema` doc comment and utoipa example.
- **`crates/api/src/dto/trigger.rs`** — Updated `out_schema` and sensor `param_schema` doc comments and utoipa examples.
- **`crates/api/src/dto/workflow.rs`** — Updated `out_schema` doc comment and utoipa example.
- **`crates/api/src/dto/pack.rs`** — Updated `conf_schema` doc comment and utoipa example.
- **`crates/api/src/dto/inquiry.rs`** — Updated `response_schema` doc comment and utoipa example.

### Documentation

- **`AGENTS.md`** — Updated "Parameter Schema Format" section to "Schema Format (Unified)", reflecting that all schema types now use the same flat format.

## Test Results

- All 29 backend validation tests pass (converted to flat format schemas)
- TypeScript compilation clean (zero errors)
- Rust workspace compilation clean (zero warnings)

## Format Reference

**Before** (JSON Schema for out_schema/conf_schema):
```yaml
output:
  type: object
  properties:
    fired_at:
      type: string
      format: date-time
  required:
    - fired_at
```

**After** (unified flat format):
```yaml
output:
  fired_at:
    type: string
    format: date-time
    required: true
    secret: false  # optional, can mark outputs as secret too
```
