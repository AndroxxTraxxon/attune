# Execution Artifacts Panel & Demo Action

**Date**: 2026-03-02

## Summary

Added an artifacts panel to the execution detail page that displays artifacts created by an execution, with support for file downloads and interactive progress tracking. Also created a Python example action that demonstrates the artifact system by creating both file and progress artifacts.

## Changes

### Web UI — Execution Artifacts Panel

**New files:**
- `web/src/hooks/useArtifacts.ts` — React Query hooks for fetching artifacts by execution ID, individual artifact details (with auto-refresh for progress), and artifact versions. Typed interfaces for `ArtifactSummary`, `ArtifactResponse`, and `ArtifactVersionSummary` matching the backend DTOs.
- `web/src/components/executions/ExecutionArtifactsPanel.tsx` — Collapsible panel component that:
  - Lists all artifacts for an execution in a table with type icon, name, ref, size, and creation time
  - Shows summary badges (file count, progress count) in the header
  - Supports authenticated file download (fetches with JWT, triggers browser download)
  - Inline expandable progress detail view with progress bar, percentage, message, and timestamped entry table
  - Auto-polls for new artifacts and progress updates while execution is running
  - Auto-hides when no artifacts exist (returns `null`)

**Modified files:**
- `web/src/pages/executions/ExecutionDetailPage.tsx` — Integrated `ExecutionArtifactsPanel` between the Workflow Tasks panel and Change History panel. Passes `executionId` and `isRunning` props.

### Python Example Pack — Artifact Demo Action

**New files:**
- `packs.external/python_example/actions/artifact_demo.yaml` — Action definition for `python_example.artifact_demo` with parameters for iteration count and API credentials
- `packs.external/python_example/actions/artifact_demo.py` — Python action that:
  1. Authenticates to the Attune API using provided credentials
  2. Creates a `file_text` artifact and a `progress` artifact, both linked to the current execution ID
  3. Runs N iterations (default 50), each iteration:
     - Appends a timestamped log line and uploads the full log as a new file version
     - Appends a progress entry with iteration number, percentage (increments by `100/iterations` per step), message, and timestamp
     - Sleeps 0.5 seconds between iterations
  4. Returns artifact IDs and completion status as JSON output

## Technical Notes

- Download uses `fetch()` with `Authorization: Bearer` header from `localStorage` since artifact endpoints require JWT auth — a plain `<a href>` would fail
- Progress artifact detail auto-refreshes every 3 seconds via `refetchInterval` on the `useArtifact` hook
- Artifact list polls every 10 seconds to pick up new artifacts created during execution
- The demo action uses `ATTUNE_API_URL` and `ATTUNE_EXEC_ID` environment variables injected by the worker, plus explicit login for auth (since `ATTUNE_API_TOKEN` is not yet implemented)
- Artifact refs include execution ID and timestamp to avoid collisions across runs