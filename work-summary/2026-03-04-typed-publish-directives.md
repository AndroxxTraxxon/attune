# Typed Publish Directives in Workflow Definitions

**Date**: 2026-03-04

## Problem

The `python_example.timeline_demo` workflow action failed to execute with:

```
Runtime not found: No runtime found for action: python_example.timeline_demo
(available: node.js, python, shell)
```

This error was misleading — the real issue was that the workflow definition YAML
failed to parse during pack registration, so the `workflow_definition` record was
never created and the action's `workflow_def` FK remained NULL. Without a linked
workflow definition, the executor treated it as a regular action and dispatched it
to a worker, which couldn't find a runtime for a workflow action.

### Root Cause

The YAML parsing error was:

```
tasks[7].next[0].publish: data did not match any variant of untagged enum
PublishDirective at line 234 column 11
```

The `PublishDirective::Simple` variant was defined as `HashMap<String, String>`,
but the workflow YAML contained non-string publish values:

```yaml
publish:
  - validation_passed: true    # boolean, not a string
  - validation_passed: false   # boolean, not a string
```

YAML parses `true`/`false` as booleans, which couldn't deserialize into `String`.

## Solution

Changed `PublishDirective::Simple` from `HashMap<String, String>` to
`HashMap<String, serde_json::Value>` so publish directives can carry any
JSON-compatible type: strings (including template expressions), booleans,
numbers, arrays, objects, and null.

### Files Modified

| File | Change |
|------|--------|
| `crates/common/src/workflow/parser.rs` | `PublishDirective::Simple` value type → `serde_json::Value` |
| `crates/executor/src/workflow/parser.rs` | Same change (executor's local copy) |
| `crates/executor/src/workflow/graph.rs` | Renamed `PublishVar.expression: String` → `PublishVar.value: JsonValue` with `#[serde(alias = "expression")]` for backward compat with stored task graphs; imported `serde_json::Value` |
| `crates/executor/src/scheduler.rs` | Updated publish map from `HashMap<String, String>` to `HashMap<String, JsonValue>` |
| `crates/executor/src/workflow/context.rs` | `publish_from_result` accepts `HashMap<String, JsonValue>`, passes values directly to `render_json` (strings get template-rendered, non-strings pass through unchanged) |
| `crates/common/src/workflow/expression_validator.rs` | Only validates string values as templates; non-string literals are skipped |
| `packs.external/python_example/actions/workflows/timeline_demo.yaml` | Fixed `result().items` → `result().data.items` (secondary bug in workflow definition) |

### Type Preservation

The rendering pipeline now correctly preserves types end-to-end:

- **String values** (e.g., `"{{ result().data }}"`) → rendered through expression engine with type preservation
- **Boolean values** (e.g., `true`) → stored as `JsonValue::Bool(true)`, pass through `render_json` unchanged
- **Numeric values** (e.g., `42`, `3.14`) → stored as `JsonValue::Number`, pass through unchanged
- **Null** → stored as `JsonValue::Null`, passes through unchanged
- **Arrays/Objects** → stored as-is, with any nested string templates rendered recursively

### Tests Added

- `parser::tests::test_typed_publish_values_in_transitions` — verifies YAML parsing of booleans, numbers, strings, templates, and null in publish directives
- `graph::tests::test_typed_publish_values` — verifies typed values survive graph construction
- `context::tests::test_publish_typed_values` — verifies typed values pass through `publish_from_result` with correct types (boolean stays boolean, not string "true")

## Verification

After deploying the fix:

1. Re-registered `python_example` pack — workflow definition created successfully (ID: 2)
2. Action `python_example.timeline_demo` linked to `workflow_def = 2`
3. Executed the workflow — executor correctly identified it as a workflow action and orchestrated 15 child task executions through all stages: initialize → parallel fan-out (build/lint/scan) → merge join → generate items → with_items(×3) → validate → finalize
4. Workflow variables confirmed type preservation: `validation_passed: true` (boolean), `items_processed: 3` (integer), `number_list: [1, 2, 3]` (array)