# Work Summary: Template Resolver Refactor

**Date:** 2026-02-05

## Overview

Moved the template resolver from the sensor crate to the common crate, renamed the template namespace from `trigger.payload` to `event.payload`, added event metadata fields, and wired the resolver into the executor's event processor for real template resolution during enforcement creation.

## Motivation

1. **Wrong location:** The template resolver lived in `crates/sensor/` but template resolution happens in the executor when rules create enforcements. The sensor crate was the wrong home.
2. **Misleading naming:** Templates used `trigger.payload.*` to reference data that actually comes from the *event* payload. The trigger defines the schema, but the runtime data is on the event.
3. **Stub in executor:** The executor's `event_processor.rs` had a `resolve_action_params()` stub that just passed action_params through unresolved. Template resolution was never actually happening.

## Changes

### Template Resolver Moved to Common Crate

- **New file:** `crates/common/src/template_resolver.rs`
- **Registered in:** `crates/common/src/lib.rs` with re-exports of `TemplateContext` and `resolve_templates`
- **Sensor crate:** `crates/sensor/src/lib.rs` now re-exports from common (`pub use attune_common::template_resolver::*`) for backward compatibility
- **Deleted:** `crates/sensor/src/template_resolver.rs` (old location)

### Renamed `trigger.payload` → `event.payload`

The template namespace was changed from `trigger` to `event` across the entire codebase:

- **Template syntax:** `{{ trigger.payload.field }}` → `{{ event.payload.field }}`
- **Struct field:** `trigger_payload: JsonValue` → `event: JsonValue` (restructured as a JSON object with `payload`, `id`, `trigger`, `created` sub-keys)
- **Context routing:** `get_value()` now routes `event.*` paths with a skip count of 1 (instead of `trigger` with skip 2)

### New Event Metadata in Templates

The `event.*` namespace now provides access to event metadata alongside the payload:

| Template | Description |
|----------|-------------|
| `{{ event.payload.* }}` | Event payload fields (same data as before, just renamed) |
| `{{ event.id }}` | Event database ID (i64) |
| `{{ event.trigger }}` | Trigger ref that generated the event |
| `{{ event.created }}` | Event creation timestamp (RFC 3339) |

Builder methods on `TemplateContext`:
- `.with_event_id(id: i64)`
- `.with_event_trigger(trigger_ref: &str)`
- `.with_event_created(created: &str)`

### Executor Integration

`crates/executor/src/event_processor.rs` — `resolve_action_params()` was rewritten from a pass-through stub to a real implementation:

- Builds a `TemplateContext` from the `Event` and `Rule` models
- Populates `event.id`, `event.trigger`, `event.created`, and `event.payload` from the Event
- Populates `system.timestamp`, `system.rule.id`, `system.rule.ref` from the Rule and current time
- Pack config is currently passed as `{}` (TODO: load from database)
- Calls `resolve_templates()` on the rule's `action_params`

### Documentation Updates

All documentation files updated from `trigger.payload` to `event.payload`:

- `docs/api/api-rules.md`
- `docs/cli/cli.md`
- `docs/examples/rule-parameter-examples.md`
- `docs/guides/quickstart-example.md`
- `docs/workflows/dynamic-parameter-forms.md`
- `docs/workflows/parameter-mapping-status.md` (also overhauled to reflect completed implementation)
- `docs/workflows/rule-parameter-mapping.md` (section headers and prose updated too)
- `docs/workflows/rule-trigger-params.md`
- `crates/cli/README.md`

### Test and Script Updates

- `scripts/setup-test-rules.sh` — updated template references
- `tests/e2e/tier3/test_t3_05_rule_criteria.py`
- `tests/e2e/tier3/test_t3_07_complex_workflows.py`
- `tests/e2e/tier3/test_t3_08_chained_webhooks.py`
- `tests/e2e/tier3/test_t3_16_rule_notifications.py`
- `tests/e2e/tier3/test_t3_17_container_runner.py`
- `tests/test_e2e_basic.py`

### Docker Fix (Separate Issue)

Fixed `ARG NODE_VERSION` scoping in Docker multi-stage builds:
- `docker/Dockerfile.sensor.optimized` — added `ARG NODE_VERSION=20` inside `sensor-full` stage
- `docker/Dockerfile.worker.optimized` — added `ARG NODE_VERSION=20` inside `worker-node` and `worker-full` stages

Global ARGs are only available in `FROM` instructions; they must be re-declared inside stages to use in `RUN` commands.

## Test Results

- **20 unit tests** in `attune_common::template_resolver` — all pass
- **78 executor lib tests** — all pass
- **Sensor tests** — all pass (re-export works correctly)
- **Zero compiler warnings** across workspace

## Remaining Work

- **Pack config loading:** The executor currently passes `{}` for pack config context. `{{ pack.config.* }}` templates will resolve to null until pack config is loaded from the database.
- **Work summaries left as-is:** Historical work summaries in `work-summary/` still reference the old `trigger.payload` syntax. These are historical records and were intentionally not updated.