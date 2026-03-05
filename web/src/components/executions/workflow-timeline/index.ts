/**
 * Workflow Timeline DAG — barrel exports.
 *
 * Usage:
 *   import WorkflowTimelineDAG from "@/components/executions/workflow-timeline";
 */

export { default } from "./WorkflowTimelineGraph";
export type { ParentExecutionInfo } from "./WorkflowTimelineGraph";
export { default as TimelineRenderer } from "./TimelineRenderer";
export { default as TimelineModal } from "./TimelineModal";

// Re-export types consumers might need
export type {
  TimelineTask,
  TimelineEdge,
  TimelineMilestone,
  TimelineNode,
  ComputedLayout,
  TaskState,
  EdgeKind,
  MilestoneKind,
  TooltipData,
  LayoutConfig,
  WorkflowDefinition,
  WithItemsGroupInfo,
} from "./types";

export { WITH_ITEMS_COLLAPSE_THRESHOLD } from "./types";

// Re-export data utilities for testing / advanced usage
export {
  buildTimelineTasks,
  buildEdges,
  buildMilestones,
  findConnectedPath,
  edgeKey,
} from "./data";

// Re-export layout utilities
export { computeLayout, computeGridLines, computeEdgePath } from "./layout";
