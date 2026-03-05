# CLI Workflow Upload Command

**Date**: 2026-03-04

## Summary

Added a `workflow` subcommand group to the Attune CLI, enabling users to upload individual workflow actions to existing packs without requiring a full pack upload. Also fixed a pre-existing `-y` short flag conflict across multiple CLI subcommands.

## Changes Made

### New File: `crates/cli/src/commands/workflow.rs`

New CLI subcommand module with four commands:

- **`attune workflow upload <action-yaml-path>`** — Reads a local action YAML file, extracts the `workflow_file` field to locate the companion workflow YAML, determines the pack from the action ref (e.g., `mypack.deploy` → pack `mypack`), and uploads both files to the API via `POST /api/v1/packs/{pack_ref}/workflow-files`. On 409 Conflict, fails unless `--force` is passed, which triggers a `PUT /api/v1/workflows/{ref}/file` update instead.
- **`attune workflow list`** — Lists workflows with optional `--pack`, `--tags`, and `--search` filters.
- **`attune workflow show <ref>`** — Shows workflow details including a task summary table (name, action, transition count).
- **`attune workflow delete <ref>`** — Deletes a workflow with `--yes` confirmation bypass.

### Modified Files

| File | Change |
|------|--------|
| `crates/cli/src/commands/mod.rs` | Added `pub mod workflow` |
| `crates/cli/src/main.rs` | Added `Workflow` variant to `Commands` enum, import, and dispatch |
| `crates/cli/src/commands/action.rs` | Fixed `-y` short flag conflict on `Delete.yes` |
| `crates/cli/src/commands/trigger.rs` | Fixed `-y` short flag conflict on `Delete.yes` |
| `crates/cli/src/commands/pack.rs` | Fixed `-y` short flag conflict on `Uninstall.yes` |
| `AGENTS.md` | Added workflow CLI documentation to CLI Tool section |

### New Test File: `crates/cli/tests/test_workflows.rs`

21 integration tests covering:
- List (authenticated, by pack, JSON/YAML output, empty, unauthenticated)
- Show (table, JSON, not found)
- Delete (with `--yes`, JSON output)
- Upload (success, JSON output, conflict without force, conflict with force, missing action file, missing workflow file, non-workflow action, invalid YAML)
- Help text (workflow help, upload help)

### Bug Fix: `-y` Short Flag Conflict

The global `--yaml` flag uses `-y` as its short form. Three existing subcommands (`action delete`, `trigger delete`, `pack uninstall`) also defined `-y` as a short flag for `--yes`. This caused a clap runtime panic when both flags were in scope (e.g., `attune --yaml action delete ref --yes`). Fixed by removing the short flag from all `yes` arguments — they now only accept `--yes` (long form).

## Design Decisions

- **Reuses existing API endpoints** — No new server-side code needed. The CLI constructs a `SaveWorkflowFileRequest` JSON payload from the two local YAML files and posts to the existing workflow-file endpoints.
- **Pack determined from action ref** — The pack ref is extracted from the action's `ref` field using the last-dot convention (e.g., `org.infra.deploy` → pack `org.infra`, name `deploy`).
- **Workflow path resolution** — The `workflow_file` value is resolved relative to the action YAML's parent directory, matching how the pack loader resolves it relative to the `actions/` directory.
- **Create-or-update pattern** — Upload attempts create first; on 409 with `--force`, falls back to update. This matches the `pack upload --force` UX pattern.

## Test Results

- **Unit tests**: 6 new (split_action_ref, resolve_workflow_path variants)
- **Integration tests**: 21 new
- **Total CLI tests**: 160 passed, 0 failed, 1 ignored (pre-existing)
- **Compiler warnings**: 0