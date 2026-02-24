# Edge Waypoints & Label Dragging for Workflow Builder

**Date:** 2026-02-05

## Summary

Added interactive edge waypoints and label dragging to the workflow builder, allowing users to manually route transition arrows through intermediate control points and reposition transition labels for better visual clarity in complex workflows.

## Changes

### Types (`web/src/types/workflow.ts`)

- **`TaskTransition`**: Added `edge_waypoints?: Record<string, NodePosition[]>` and `label_positions?: Record<string, NodePosition>` fields, keyed by target task name, for per-edge routing data
- **`WorkflowEdge`**: Added `toName` (stable target task name key), `waypoints?: NodePosition[]`, and `labelPosition?: NodePosition` fields
- **`TransitionChartMeta`**: Added `edge_waypoints` and `label_positions` for YAML serialization via `__chart_meta__`
- **`EdgeHoverInfo`**: Added `targetTaskId` field to uniquely identify clicked edges
- **`deriveEdges()`**: Extracts per-edge waypoints and label positions from transition chart meta
- **`builderStateToDefinition()`**: Serializes waypoints and label positions into `__chart_meta__`
- **`definitionToBuilderState()`**: Deserializes them on load
- **`removeTaskFromTransitions()`**: Cleans up waypoint/label entries when a target task is removed
- **`renameTaskInTransitions()`**: Renames keys in `edge_waypoints` and `label_positions` when a task is renamed

### Edge Rendering (`web/src/components/workflows/WorkflowEdges.tsx`)

- **`SelectedEdgeInfo` interface**: Tracks which edge is selected for waypoint editing
- **`buildSmoothPath()`**: New function that draws smooth multi-segment SVG paths through waypoints using Catmull-Rom → cubic Bezier conversion
- **`computeDefaultLabelPosition()`**: Computes a default label position from path points
- **Waypoint handles**: Small colored circles at each waypoint, draggable when edge is selected; double-click to remove
- **Midpoint add handles**: "+" indicators appear on hover at segment midpoints of the selected edge; click to insert a new waypoint, or drag to insert and immediately reposition
- **Label dragging**: Transition labels are draggable when the edge is selected; double-click to reset to default position
- **Edge selection glow**: Selected edges render with a subtle glow effect and slightly thicker stroke
- **Effect-based drag handling**: Uses `useEffect` with `isDragging` state to manage document-level mouse listeners, with refs for latest callback values to avoid stale closures

### Canvas Integration (`web/src/components/workflows/WorkflowCanvas.tsx`)

- **`selectedEdge` state**: Tracks which edge is selected for waypoint manipulation
- **`handleEdgeClick()`**: Sets both edge selection and propagates to parent for task inspector highlighting
- **`handleSelectTask()`**: Clears edge selection when a different task is clicked
- **`handleWaypointUpdate()`**: Updates a task's transition `edge_waypoints` for a specific target
- **`handleLabelPositionUpdate()`**: Updates a task's transition `label_positions` for a specific target
- All new props passed through to `WorkflowEdges`

## User Interaction

1. **Click an edge** to select it — the edge highlights with a glow and shows waypoint handles
2. **Hover the midpoint** of any segment on the selected edge to reveal a "+" indicator
3. **Click or drag the "+"** to insert a new waypoint at that position
4. **Drag waypoint handles** to reposition the edge path
5. **Drag the label** to move it independently of the path
6. **Double-click a waypoint** to remove it
7. **Double-click a label** to reset it to default position
8. **Click canvas background** or another task to deselect the edge

## Data Persistence

Waypoints and label positions are stored in the workflow YAML via `__chart_meta__` on transitions, keyed by target task name. This ensures:
- Data survives save/reload cycles
- Per-edge granularity (a transition with `do: [taskA, taskB]` has independent waypoints for each target)
- Task renames and deletions properly update the keys
- Backend ignores `__chart_meta__` — it's purely visual metadata