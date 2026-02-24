import { memo, useMemo } from "react";
import type { WorkflowEdge, WorkflowTask, EdgeType } from "@/types/workflow";
import type { TransitionPreset } from "./TaskNode";

export interface EdgeHoverInfo {
  taskId: string;
  transitionIndex: number;
}

interface WorkflowEdgesProps {
  edges: WorkflowEdge[];
  tasks: WorkflowTask[];
  /** Width of each task node (must match TaskNode width) */
  nodeWidth?: number;
  /** Approximate height of each task node */
  nodeHeight?: number;
  /** The task ID currently being connected from (for preview line) */
  connectingFrom?: { taskId: string; preset: TransitionPreset } | null;
  /** Mouse position for drawing the preview connection line */
  mousePosition?: { x: number; y: number } | null;
  /** Called when the mouse enters/leaves an edge hit area */
  onEdgeHover?: (info: EdgeHoverInfo | null) => void;
}

const NODE_WIDTH = 240;
const NODE_HEIGHT = 120;

/** Color for each edge type */
const EDGE_COLORS: Record<EdgeType, string> = {
  success: "#22c55e", // green-500
  failure: "#ef4444", // red-500
  complete: "#6b7280", // gray-500 (unconditional / always)
  custom: "#8b5cf6", // violet-500
};

const EDGE_DASH: Record<EdgeType, string> = {
  success: "",
  failure: "6,4",
  complete: "4,4",
  custom: "8,4,2,4",
};

/** Map presets to edge colors for the preview line */
const PRESET_COLORS: Record<TransitionPreset, string> = {
  succeeded: EDGE_COLORS.success,
  failed: EDGE_COLORS.failure,
  always: EDGE_COLORS.complete,
};

/** Calculate the center-bottom of a task node */
function getNodeBottomCenter(
  task: WorkflowTask,
  nodeWidth: number,
  nodeHeight: number,
) {
  return {
    x: task.position.x + nodeWidth / 2,
    y: task.position.y + nodeHeight,
  };
}

/** Calculate the center-top of a task node */
function getNodeTopCenter(task: WorkflowTask, nodeWidth: number) {
  return {
    x: task.position.x + nodeWidth / 2,
    y: task.position.y,
  };
}

/** Calculate the left-center of a task node */
function getNodeLeftCenter(task: WorkflowTask, nodeHeight: number) {
  return {
    x: task.position.x,
    y: task.position.y + nodeHeight / 2,
  };
}

/** Calculate the right-center of a task node */
function getNodeRightCenter(
  task: WorkflowTask,
  nodeWidth: number,
  nodeHeight: number,
) {
  return {
    x: task.position.x + nodeWidth,
    y: task.position.y + nodeHeight / 2,
  };
}

/**
 * Determine the best connection points between two nodes.
 * Returns the start and end points for the edge.
 */
function getBestConnectionPoints(
  fromTask: WorkflowTask,
  toTask: WorkflowTask,
  nodeWidth: number,
  nodeHeight: number,
): { start: { x: number; y: number }; end: { x: number; y: number } } {
  const fromCenter = {
    x: fromTask.position.x + nodeWidth / 2,
    y: fromTask.position.y + nodeHeight / 2,
  };
  const toCenter = {
    x: toTask.position.x + nodeWidth / 2,
    y: toTask.position.y + nodeHeight / 2,
  };

  const dx = toCenter.x - fromCenter.x;
  const dy = toCenter.y - fromCenter.y;

  // If the target is mostly below the source, use bottom→top
  if (dy > 0 && Math.abs(dy) > Math.abs(dx) * 0.5) {
    return {
      start: getNodeBottomCenter(fromTask, nodeWidth, nodeHeight),
      end: getNodeTopCenter(toTask, nodeWidth),
    };
  }

  // If the target is mostly above the source, use top→bottom
  if (dy < 0 && Math.abs(dy) > Math.abs(dx) * 0.5) {
    return {
      start: getNodeTopCenter(fromTask, nodeWidth),
      end: getNodeBottomCenter(toTask, nodeWidth, nodeHeight),
    };
  }

  // If the target is to the right, use right→left
  if (dx > 0) {
    return {
      start: getNodeRightCenter(fromTask, nodeWidth, nodeHeight),
      end: getNodeLeftCenter(toTask, nodeHeight),
    };
  }

  // Target is to the left, use left→right
  return {
    start: getNodeLeftCenter(fromTask, nodeHeight),
    end: getNodeRightCenter(toTask, nodeWidth, nodeHeight),
  };
}

/**
 * Build an SVG path string for a curved edge between two points.
 * Uses a cubic bezier curve.
 */
function buildCurvePath(
  start: { x: number; y: number },
  end: { x: number; y: number },
): string {
  const dx = end.x - start.x;
  const dy = end.y - start.y;

  // Determine control points based on dominant direction
  let cp1: { x: number; y: number };
  let cp2: { x: number; y: number };

  if (Math.abs(dy) > Math.abs(dx) * 0.5) {
    // Mostly vertical connection
    const offset = Math.min(Math.abs(dy) * 0.5, 80);
    const direction = dy > 0 ? 1 : -1;
    cp1 = { x: start.x, y: start.y + offset * direction };
    cp2 = { x: end.x, y: end.y - offset * direction };
  } else {
    // Mostly horizontal connection
    const offset = Math.min(Math.abs(dx) * 0.5, 80);
    const direction = dx > 0 ? 1 : -1;
    cp1 = { x: start.x + offset * direction, y: start.y };
    cp2 = { x: end.x - offset * direction, y: end.y };
  }

  return `M ${start.x} ${start.y} C ${cp1.x} ${cp1.y}, ${cp2.x} ${cp2.y}, ${end.x} ${end.y}`;
}

function WorkflowEdgesInner({
  edges,
  tasks,
  nodeWidth = NODE_WIDTH,
  nodeHeight = NODE_HEIGHT,
  connectingFrom,
  mousePosition,
  onEdgeHover,
}: WorkflowEdgesProps) {
  const taskMap = useMemo(() => {
    const map = new Map<string, WorkflowTask>();
    for (const task of tasks) {
      map.set(task.id, task);
    }
    return map;
  }, [tasks]);

  // Calculate SVG bounds to cover all nodes + padding
  const svgBounds = useMemo(() => {
    if (tasks.length === 0) return { width: 2000, height: 2000 };
    let maxX = 0;
    let maxY = 0;
    for (const task of tasks) {
      maxX = Math.max(maxX, task.position.x + nodeWidth + 100);
      maxY = Math.max(maxY, task.position.y + nodeHeight + 100);
    }
    return {
      width: Math.max(maxX, 2000),
      height: Math.max(maxY, 2000),
    };
  }, [tasks, nodeWidth, nodeHeight]);

  const renderedEdges = useMemo(() => {
    return edges
      .map((edge, index) => {
        const fromTask = taskMap.get(edge.from);
        const toTask = taskMap.get(edge.to);
        if (!fromTask || !toTask) return null;

        const { start, end } = getBestConnectionPoints(
          fromTask,
          toTask,
          nodeWidth,
          nodeHeight,
        );

        const pathD = buildCurvePath(start, end);
        const color =
          edge.color || EDGE_COLORS[edge.type] || EDGE_COLORS.complete;
        const dash = EDGE_DASH[edge.type] || "";

        // Calculate label position (midpoint of curve)
        const labelX = (start.x + end.x) / 2;
        const labelY = (start.y + end.y) / 2 - 8;

        // Measure approximate label width
        const labelText = edge.label || "";
        const labelWidth = Math.max(labelText.length * 5.5 + 12, 48);
        const arrowId = edge.color
          ? `arrow-custom-${index}`
          : `arrow-${edge.type}`;

        return (
          <g key={`edge-${index}-${edge.from}-${edge.to}`}>
            {/* Edge path */}
            <path
              d={pathD}
              fill="none"
              stroke={color}
              strokeWidth={2}
              strokeDasharray={dash}
              markerEnd={`url(#${arrowId})`}
              className="transition-opacity"
              opacity={0.75}
            />
            {/* Wider invisible path for easier hovering */}
            <path
              d={pathD}
              fill="none"
              stroke="transparent"
              strokeWidth={12}
              className="cursor-pointer"
              onMouseEnter={() =>
                onEdgeHover?.({
                  taskId: edge.from,
                  transitionIndex: edge.transitionIndex,
                })
              }
              onMouseLeave={() => onEdgeHover?.(null)}
            />
            {/* Label */}
            {edge.label && (
              <g>
                <rect
                  x={labelX - labelWidth / 2}
                  y={labelY - 7}
                  width={labelWidth}
                  height={14}
                  rx={3}
                  fill="white"
                  stroke={color}
                  strokeWidth={0.5}
                  opacity={0.9}
                />
                <text
                  x={labelX}
                  y={labelY + 3}
                  textAnchor="middle"
                  fontSize={9}
                  fontWeight={500}
                  fill={color}
                  className="select-none pointer-events-none"
                >
                  {labelText.length > 24
                    ? labelText.slice(0, 21) + "..."
                    : labelText}
                </text>
              </g>
            )}
          </g>
        );
      })
      .filter(Boolean);
  }, [edges, taskMap, nodeWidth, nodeHeight, onEdgeHover]);

  // Preview line when connecting
  const previewLine = useMemo(() => {
    if (!connectingFrom || !mousePosition) return null;
    const fromTask = taskMap.get(connectingFrom.taskId);
    if (!fromTask) return null;

    const start = getNodeBottomCenter(fromTask, nodeWidth, nodeHeight);
    const end = mousePosition;
    const pathD = buildCurvePath(start, end);
    const color = PRESET_COLORS[connectingFrom.preset] || EDGE_COLORS.complete;

    return (
      <path
        d={pathD}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeDasharray="6,4"
        opacity={0.5}
        className="pointer-events-none"
      />
    );
  }, [connectingFrom, mousePosition, taskMap, nodeWidth, nodeHeight]);

  return (
    <svg
      className="absolute inset-0 pointer-events-none overflow-visible"
      width={svgBounds.width}
      height={svgBounds.height}
      style={{ zIndex: 1 }}
    >
      <defs>
        {/* Arrow markers for each edge type */}
        {Object.entries(EDGE_COLORS).map(([type, color]) => (
          <marker
            key={`arrow-${type}`}
            id={`arrow-${type}`}
            viewBox="0 0 10 10"
            refX={9}
            refY={5}
            markerWidth={8}
            markerHeight={8}
            orient="auto-start-reverse"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" fill={color} opacity={0.8} />
          </marker>
        ))}
      </defs>

      {/* Render edges */}
      <g className="pointer-events-auto">
        {/* Dynamic arrow markers for custom-colored edges */}
        {edges.map((edge, index) => {
          if (!edge.color) return null;
          return (
            <marker
              key={`arrow-custom-${index}`}
              id={`arrow-custom-${index}`}
              viewBox="0 0 10 10"
              refX={9}
              refY={5}
              markerWidth={8}
              markerHeight={8}
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" fill={edge.color} opacity={0.8} />
            </marker>
          );
        })}
        {renderedEdges}
      </g>

      {/* Preview line */}
      {previewLine}
    </svg>
  );
}

const WorkflowEdges = memo(WorkflowEdgesInner);
export default WorkflowEdges;
