# Workflow Timeline DAG Visualization

**Date**: 2026-02-05
**Component**: `web/src/components/executions/workflow-timeline/`
**Integration**: `web/src/pages/executions/ExecutionDetailPage.tsx`

## Overview

Added a Prefect-style workflow run timeline DAG visualization to the execution detail page for workflow executions. The component renders child task executions as horizontal duration bars on a time axis, connected by curved dependency edges that reflect the actual workflow definition transitions.

## Architecture

The implementation is a pure SVG renderer with no additional dependencies — it uses React, TypeScript, and inline SVG only (no D3, no React Flow, no new npm packages).

### Module Structure

```
web/src/components/executions/workflow-timeline/
├── index.ts                  # Barrel exports
├── types.ts                  # Type definitions, color constants, layout config
├── data.ts                   # Data transformation (executions → timeline structures)
├── layout.ts                 # Layout engine (lane assignment, time scaling, edge paths)
├── TimelineRenderer.tsx      # SVG renderer with interactions
└── WorkflowTimelineDAG.tsx   # Orchestrator component (data fetching + layout + render)
```

### Data Flow

1. **WorkflowTimelineDAG** (orchestrator) fetches child executions via `useChildExecutions` and the workflow definition via `useWorkflow(actionRef)`.
2. **data.ts** transforms `ExecutionSummary[]` + `WorkflowDefinition` into `TimelineTask[]`, `TimelineEdge[]`, and `TimelineMilestone[]`.
3. **layout.ts** computes lane assignments (greedy packing), time→pixel scale, node positions, grid lines, and cubic Bezier edge paths.
4. **TimelineRenderer** renders everything as SVG with interactive features.

## Key Features

### Visualization
- **Task bars**: Horizontal rounded rectangles colored by state (green=completed, blue=running, red=failed, gray=pending, orange=timeout). Left accent bar indicates state. Running tasks pulse.
- **Milestones**: Synthetic start/end diamond nodes plus merge/fork junctions inserted when fan-in/fan-out exceeds 3 tasks.
- **Edges**: Curved cubic Bezier dependency lines with transition-aware coloring and labels derived from the workflow definition (`succeeded`, `failed`, `timed out`, custom expressions). Failure edges are dashed, timeout edges use dash-dot pattern.
- **Time axis**: Vertical gridlines at "nice" intervals with timestamp labels along the top.
- **Lane packing**: Greedy algorithm assigns tasks to non-overlapping y-lanes, with optional lane reordering to cluster tasks with shared upstream dependencies.

### Workflow Metadata Integration
- Fetches the workflow definition to extract the `next` transition array from each task definition.
- Maps definition task names to execution IDs (handles `with_items` expansions with multiple executions per task name).
- Classifies `when` expressions (`{{ succeeded() }}`, `{{ failed() }}`, `{{ timed_out() }}`) into edge kinds with appropriate colors.
- Reads `__chart_meta__` labels and custom colors from workflow definition transitions.
- Falls back to timing-based heuristic edge inference when no workflow definition is available.

### Interactions
- **Hover tooltip**: Shows task name, state, action ref, start/end times, duration, retry info, upstream/downstream counts.
- **Click selection**: Clicking a task highlights its full upstream/downstream path (BFS traversal) and dims unrelated nodes/edges.
- **Double-click navigation**: Navigates to the child execution's detail page.
- **Horizontal zoom**: Mouse wheel zooms the x-axis while keeping y-lanes stable. Zoom anchors to cursor position.
- **Pan**: Alt+drag or middle-mouse-drag pans horizontally via native scroll.
- **Expand/compact toggle**: Expand button widens the chart for complex workflows.

### Performance
- Edge paths are memoized per layout computation.
- Node lookups use a `Map<string, TimelineNode>` for O(1) access.
- Grid lines and highlighted paths are memoized with stable dependency arrays.
- ResizeObserver tracks container width for responsive layout without polling.
- No additional npm dependencies; SVG rendering handles 300+ tasks efficiently.

## Integration Point

The `WorkflowTimelineDAG` component is rendered on the execution detail page (`ExecutionDetailPage.tsx`) above the existing `WorkflowTasksPanel`, conditioned on `isWorkflow` (action has `workflow_def`).

Both components share a single TanStack Query cache entry for child executions (`["executions", { parent: id }]`) and both subscribe to WebSocket execution streams for real-time updates.

The `WorkflowTimelineDAG` accepts a `ParentExecutionInfo` interface (satisfied by both `ExecutionResponse` and `ExecutionSummary`) to avoid type casting at the integration point.

## Files Changed

| File | Change |
|------|--------|
| `web/src/components/executions/workflow-timeline/types.ts` | New — type definitions |
| `web/src/components/executions/workflow-timeline/data.ts` | New — data transformation |
| `web/src/components/executions/workflow-timeline/layout.ts` | New — layout engine |
| `web/src/components/executions/workflow-timeline/TimelineRenderer.tsx` | New — SVG renderer |
| `web/src/components/executions/workflow-timeline/WorkflowTimelineDAG.tsx` | New — orchestrator |
| `web/src/components/executions/workflow-timeline/index.ts` | New — barrel exports |
| `web/src/pages/executions/ExecutionDetailPage.tsx` | Modified — import + render WorkflowTimelineDAG |