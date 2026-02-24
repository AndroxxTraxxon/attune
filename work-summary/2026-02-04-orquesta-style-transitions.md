# Orquesta-Style Task Transition Model

**Date**: 2026-02-04

## Overview

Refactored the workflow builder's task transition model from flat `on_success`/`on_failure`/`on_complete`/`on_timeout` fields to an Orquesta-style ordered `next` array of transitions. Each transition can specify a `when` condition, `publish` directives, and multiple `do` targets — enabling far more expressive workflow definitions.

Also added visual drag handles to task nodes in the workflow builder for creating transitions via drag-and-drop.

## Motivation

The previous model only allowed a single target task per transition type and had no way to:
- Route to multiple tasks from a single transition
- Attach per-transition variable publishing
- Use custom condition expressions beyond the four fixed types
- Publish variables without transitioning to another task

The Orquesta model (from StackStorm) solves all of these with a simple, ordered list of conditional transitions.

## Changes

### Frontend (`web/`)

#### `web/src/types/workflow.ts`
- **Added** `TaskTransition` type: `{ when?: string; publish?: PublishDirective[]; do?: string[] }`
- **Added** `TransitionPreset` type and constants (`PRESET_WHEN`, `PRESET_LABELS`) for the three common quick-access patterns: succeeded, failed, always
- **Added** `classifyTransitionWhen()` and `transitionLabel()` for edge visualization
- **Added** `EdgeType` — simplified to `"success" | "failure" | "complete" | "custom"`
- **Added** helper functions: `findOrCreateTransition()`, `addTransitionTarget()`, `removeTaskFromTransitions()`
- **Removed** `on_success`, `on_failure`, `on_complete`, `on_timeout`, `decision`, `publish` fields from `WorkflowTask`
- **Removed** `DecisionBranch` type (subsumed by `TaskTransition.when`)
- **Updated** `WorkflowYamlTask` to use `next?: WorkflowYamlTransition[]`
- **Updated** `builderStateToDefinition()` to serialize `next` array
- **Updated** `definitionToBuilderState()` to load both new `next` format and legacy flat fields (auto-converts)
- **Updated** `deriveEdges()` to iterate `task.next[].do[]`
- **Updated** `validateWorkflow()` to validate `next[].do[]` targets

#### `web/src/components/workflows/TaskNode.tsx`
- **Redesigned output handles**: Three color-coded drag handles at bottom (green=succeeded, red=failed, gray=always)
- **Added input handle**: Neutral circle at top center as drop target, highlights purple during active connection
- **Removed** old footer link-icon buttons
- **Added** transition summary in node body (e.g., "2 targets via 1 transition")
- **Added** custom transitions badge

#### `web/src/components/workflows/WorkflowEdges.tsx`
- Updated edge colors for new `EdgeType` values
- Preview line color now uses `TransitionPreset` mapping
- Dynamic label width based on text content

#### `web/src/components/workflows/WorkflowCanvas.tsx`
- Updated to use `TransitionPreset` instead of `TransitionType`
- Added `onMouseUp` handler for drag-to-cancel on canvas background

#### `web/src/components/workflows/TaskInspector.tsx`
- **Replaced** four fixed `TransitionField` dropdowns with a dynamic transition list editor
- Each transition card shows: `when` expression (editable), `do` target list (add/remove), `publish` key-value pairs (add/remove)
- Quick-set buttons for common `when` presets (On Success, On Failure, Always)
- Add transition buttons: "On Success", "On Failure", "Custom transition"
- **Moved** publish variables from task-level section to per-transition
- **Removed** old `TransitionField` component
- **Added** Join section for barrier configuration

#### `web/src/pages/actions/WorkflowBuilderPage.tsx`
- Updated `handleSetConnection` to use `addTransitionTarget()` with `TransitionPreset`
- Updated `handleDeleteTask` to use `removeTaskFromTransitions()`

### Backend (`crates/`)

#### `crates/common/src/workflow/parser.rs`
- **Added** `TaskTransition` struct: `{ when, publish, do }`
- **Added** `Task::normalize_transitions()` — converts legacy fields into `next` array
- **Added** `Task::all_transition_targets()` — collects all referenced task names
- **Updated** `parse_workflow_yaml()` to call `normalize_all_transitions()` after parsing
- **Updated** `validate_task()` to use `all_transition_targets()` instead of checking individual fields
- Legacy fields (`on_success`, `on_failure`, `on_complete`, `on_timeout`, `decision`) retained for deserialization but cleared after normalization
- **Added** 12 new tests covering both new and legacy formats

#### `crates/common/src/workflow/validator.rs`
- Updated `build_graph()` and `find_entry_points()` to use `task.all_transition_targets()`

#### `crates/common/src/workflow/mod.rs`
- Exported new `TaskTransition` type

#### `crates/executor/src/workflow/graph.rs`
- **Replaced** `TaskTransitions` struct (flat fields) with `Vec<GraphTransition>`
- **Added** `GraphTransition`: `{ when, publish: Vec<PublishVar>, do_tasks: Vec<String> }`
- **Added** `PublishVar`: `{ name, expression }` — preserves both key and value
- **Added** `TransitionKind` enum and `GraphTransition::kind()` classifier
- **Added** `TaskGraph::matching_transitions()` — returns full transition objects for coordinators
- **Added** `TaskGraph::all_transition_targets()` — all target names from a task
- **Updated** `next_tasks()` to evaluate transitions by `TransitionKind`
- **Updated** `compute_inbound_edges()` to iterate `GraphTransition.do_tasks`
- **Updated** `extract_publish_vars()` to return `Vec<PublishVar>` instead of `Vec<String>`
- **Added** 12 new tests

#### `crates/executor/src/workflow/task_executor.rs`
- Updated variable publishing to extract from matching transitions instead of removed `task.publish` field

## YAML Format

### New (canonical)
```yaml
tasks:
  - name: task1
    action: core.echo
    next:
      - when: "{{ succeeded() }}"
        publish:
          - result: "{{ result() }}"
        do:
          - task2
          - log
      - when: "{{ failed() }}"
        do:
          - error_handler
```

### Legacy (still parsed, auto-converted)
```yaml
tasks:
  - name: task1
    action: core.echo
    on_success: task2
    on_failure: error_handler
```

## Test Results

- **Parser tests**: 37 passed (includes 12 new)
- **Graph tests**: 12 passed (includes 10 new)
- **TypeScript**: Zero errors
- **Rust workspace**: Zero warnings