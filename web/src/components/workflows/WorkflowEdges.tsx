import { memo, useMemo, useState, useCallback, useRef, useEffect } from "react";
import type {
  WorkflowEdge,
  WorkflowTask,
  NodePosition,
} from "@/types/workflow";
import { PRESET_COLORS, EDGE_TYPE_COLORS } from "@/types/workflow";
import type { TransitionPreset } from "./TaskNode";
import type { ScreenToCanvas } from "./WorkflowCanvas";

export interface EdgeHoverInfo {
  taskId: string;
  targetTaskId: string;
  transitionIndex: number;
}

/** Identifies a selected edge for waypoint editing */
export interface SelectedEdgeInfo {
  from: string;
  to: string;
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
  /** Called when an edge is clicked */
  onEdgeClick?: (info: EdgeHoverInfo | null) => void;
  /** Currently selected edge (shows waypoint handles) */
  selectedEdge?: SelectedEdgeInfo | null;
  /** Called when waypoints change for an edge */
  onWaypointUpdate?: (
    fromTaskId: string,
    transitionIndex: number,
    targetTaskName: string,
    waypoints: NodePosition[],
  ) => void;
  /** Called when label position changes for an edge (t-parameter 0–1 along path) */
  onLabelPositionUpdate?: (
    fromTaskId: string,
    transitionIndex: number,
    targetTaskName: string,
    position: number | undefined,
  ) => void;
  /** Convert screen (client) coordinates to canvas-space coordinates. */
  screenToCanvas?: ScreenToCanvas;
}

const NODE_WIDTH = 240;
const NODE_HEIGHT = 96;
const SELF_LOOP_RIGHT_OFFSET = 24;
const SELF_LOOP_TOP_OFFSET = 36;
const SELF_LOOP_BOTTOM_OFFSET = 30;
const ARROW_LENGTH = 12;
const ARROW_HALF_WIDTH = 5;
const ARROW_DIRECTION_LOOKBACK_PX = 10;
const ARROW_DIRECTION_SAMPLES = 48;
// Keep a small amount of shaft under the arrowhead so sample-based trimming
// does not leave a visible gap on simple bezier edges without waypoints.
const ARROW_SHAFT_OVERLAP_PX = 4;

/** Color for each edge type (alias for shared constant) */
const EDGE_COLORS = EDGE_TYPE_COLORS;

/** SVG stroke-dasharray values for each user-facing line style */
import type { LineStyle } from "@/types/workflow";

const LINE_STYLE_DASH: Record<LineStyle, string> = {
  solid: "",
  dashed: "6,4",
  dotted: "2,3",
  "dash-dot": "8,4,2,4",
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
 * Pick the closest of top / left / right edges on the destination card
 * to an approach point. Bottom is excluded — it's reserved for outgoing
 * transitions.
 */
function closestEntryEdge(
  task: WorkflowTask,
  approach: { x: number; y: number },
  nodeWidth: number,
  nodeHeight: number,
): { x: number; y: number } {
  const candidates = [
    getNodeTopCenter(task, nodeWidth),
    getNodeLeftCenter(task, nodeHeight),
    getNodeRightCenter(task, nodeWidth, nodeHeight),
  ];
  let best = candidates[0];
  let bestDist = Infinity;
  for (const c of candidates) {
    const d = (c.x - approach.x) ** 2 + (c.y - approach.y) ** 2;
    if (d < bestDist) {
      bestDist = d;
      best = c;
    }
  }
  return best;
}

/**
 * Determine the best connection points between two nodes.
 *
 * Rules:
 *  - Origin always exits from the bottom edge (where the output handles are).
 *  - Destination picks the closest of top / left / right (never bottom).
 *  - When waypoints exist the last waypoint is used as the approach hint
 *    for the destination edge, and the first waypoint is ignored for origin
 *    (origin is always bottom).
 */
function getBestConnectionPoints(
  fromTask: WorkflowTask,
  toTask: WorkflowTask,
  nodeWidth: number,
  nodeHeight: number,
  waypoints?: { x: number; y: number }[],
): {
  start: { x: number; y: number };
  end: { x: number; y: number };
  selfLoop?: boolean;
} {
  // Self-loop uses a dedicated route that stays outside the task card so the
  // arrowhead and label remain readable instead of being covered by the node.
  if (fromTask.id === toTask.id) {
    return {
      start: getNodeBottomCenter(fromTask, nodeWidth, nodeHeight),
      end: {
        x: fromTask.position.x + nodeWidth,
        y: fromTask.position.y + nodeHeight * 0.28,
      },
      selfLoop: true,
    };
  }

  // Origin always exits from bottom
  const start = getNodeBottomCenter(fromTask, nodeWidth, nodeHeight);

  // Use the last waypoint (if any) as the approach direction for the
  // destination, otherwise use the start point.
  const approach =
    waypoints && waypoints.length > 0 ? waypoints[waypoints.length - 1] : start;

  const end = closestEntryEdge(toTask, approach, nodeWidth, nodeHeight);

  return { start, end };
}

function buildSelfLoopRoute(
  task: WorkflowTask,
  nodeWidth: number,
  nodeHeight: number,
): { x: number; y: number }[] {
  const start = getNodeBottomCenter(task, nodeWidth, nodeHeight);
  const cardRight = task.position.x + nodeWidth;
  const cardTop = task.position.y;
  const loopRight = cardRight + SELF_LOOP_RIGHT_OFFSET;
  const loopTop = cardTop + SELF_LOOP_TOP_OFFSET;
  const loopBottom = start.y + SELF_LOOP_BOTTOM_OFFSET;

  return [
    start,
    { x: start.x, y: loopBottom },
    { x: loopRight, y: loopBottom },
    { x: loopRight, y: loopTop },
    { x: cardRight, y: task.position.y + nodeHeight * 0.28 },
  ];
}

/**
 * Build an SVG path string for a curved edge between two points.
 */
function buildCurvePath(
  start: { x: number; y: number },
  end: { x: number; y: number },
): string {
  const dx = end.x - start.x;
  const dy = end.y - start.y;

  let cp1: { x: number; y: number };
  let cp2: { x: number; y: number };

  if (Math.abs(dy) > Math.abs(dx) * 0.5) {
    const offset = Math.min(Math.abs(dy) * 0.5, 80);
    const direction = dy > 0 ? 1 : -1;
    cp1 = { x: start.x, y: start.y + offset * direction };
    cp2 = { x: end.x, y: end.y - offset * direction };
  } else {
    const offset = Math.min(Math.abs(dx) * 0.5, 80);
    const direction = dx > 0 ? 1 : -1;
    cp1 = { x: start.x + offset * direction, y: start.y };
    cp2 = { x: end.x - offset * direction, y: end.y };
  }

  return `M ${start.x} ${start.y} C ${cp1.x} ${cp1.y}, ${cp2.x} ${cp2.y}, ${end.x} ${end.y}`;
}

/**
 * Build a smooth SVG path through multiple points using Catmull-Rom → cubic Bezier conversion.
 */
function buildSmoothPath(points: { x: number; y: number }[]): string {
  if (points.length < 2) return "";
  if (points.length === 2) return buildCurvePath(points[0], points[1]);

  let d = `M ${points[0].x} ${points[0].y}`;

  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[Math.max(0, i - 1)];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[Math.min(points.length - 1, i + 2)];

    // Catmull-Rom to cubic Bezier: CP1 = P1 + (P2 - P0) / 6, CP2 = P2 - (P3 - P1) / 6
    const cp1x = p1.x + (p2.x - p0.x) / 6;
    const cp1y = p1.y + (p2.y - p0.y) / 6;
    const cp2x = p2.x - (p3.x - p1.x) / 6;
    const cp2y = p2.y - (p3.y - p1.y) / 6;

    d += ` C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${p2.x} ${p2.y}`;
  }

  return d;
}

/**
 * Evaluate a cubic Bezier curve at parameter t ∈ [0,1].
 * Returns the (x, y) point on the curve.
 */
function evaluateCubicBezier(
  p0: { x: number; y: number },
  cp1: { x: number; y: number },
  cp2: { x: number; y: number },
  p3: { x: number; y: number },
  t: number,
): { x: number; y: number } {
  const u = 1 - t;
  const u2 = u * u;
  const u3 = u2 * u;
  const t2 = t * t;
  const t3 = t2 * t;
  return {
    x: u3 * p0.x + 3 * u2 * t * cp1.x + 3 * u * t2 * cp2.x + t3 * p3.x,
    y: u3 * p0.y + 3 * u2 * t * cp1.y + 3 * u * t2 * cp2.y + t3 * p3.y,
  };
}

/**
 * Get the control points for a specific segment of the path.
 * For a 2-point path, uses the buildCurvePath logic.
 * For a multi-point path, uses the Catmull-Rom (buildSmoothPath) logic.
 */
function getSegmentControlPoints(
  allPoints: { x: number; y: number }[],
  segIdx: number,
): { cp1: { x: number; y: number }; cp2: { x: number; y: number } } {
  if (allPoints.length === 2) {
    return getCurveControlPoints(allPoints[0], allPoints[1]);
  }
  return getSmoothSegmentControlPoints(allPoints, segIdx);
}

/**
 * Evaluate the full path at a global t parameter ∈ [0, 1].
 * Maps t onto the correct segment then evaluates the cubic Bezier for that segment.
 */
function evaluatePathAtT(
  allPoints: { x: number; y: number }[],
  t: number,
  _selfLoop?: boolean,
): { x: number; y: number } {
  if (allPoints.length < 2) {
    return allPoints[0] ?? { x: 0, y: 0 };
  }

  // Self-loop with no waypoints (allPoints = [start, end])
  const numSegments = allPoints.length - 1;
  const clampedT = Math.max(0, Math.min(1, t));
  const scaledT = clampedT * numSegments;
  const segIdx = Math.min(Math.floor(scaledT), numSegments - 1);
  const localT = scaledT - segIdx;

  const { cp1, cp2 } = getSegmentControlPoints(allPoints, segIdx);
  return evaluateCubicBezier(
    allPoints[segIdx],
    cp1,
    cp2,
    allPoints[segIdx + 1],
    localT,
  );
}

/**
 * Project a mouse position onto the nearest point on the path.
 * Returns the global t parameter ∈ [0, 1].
 */
function projectOntoPath(
  allPoints: { x: number; y: number }[],
  mousePos: { x: number; y: number },
  _selfLoop?: boolean,
): number {
  if (allPoints.length < 2) return 0;

  const samplesPerSegment = 60;
  let bestT = 0.5;
  let bestDist = Infinity;

  const numSegments = allPoints.length - 1;

  for (let seg = 0; seg < numSegments; seg++) {
    const { cp1, cp2 } = getSegmentControlPoints(allPoints, seg);
    const p1 = allPoints[seg];
    const p2 = allPoints[seg + 1];

    for (let s = 0; s <= samplesPerSegment; s++) {
      const localT = s / samplesPerSegment;
      const pt = evaluateCubicBezier(p1, cp1, cp2, p2, localT);
      const dist = Math.hypot(pt.x - mousePos.x, pt.y - mousePos.y);
      if (dist < bestDist) {
        bestDist = dist;
        bestT = (seg + localT) / numSegments;
      }
    }
  }

  return bestT;
}

/**
 * Compute the control points for a 2-point cubic Bezier (matching buildCurvePath logic).
 */
function getCurveControlPoints(
  start: { x: number; y: number },
  end: { x: number; y: number },
): { cp1: { x: number; y: number }; cp2: { x: number; y: number } } {
  const dx = end.x - start.x;
  const dy = end.y - start.y;

  let cp1: { x: number; y: number };
  let cp2: { x: number; y: number };

  if (Math.abs(dy) > Math.abs(dx) * 0.5) {
    const offset = Math.min(Math.abs(dy) * 0.5, 80);
    const direction = dy > 0 ? 1 : -1;
    cp1 = { x: start.x, y: start.y + offset * direction };
    cp2 = { x: end.x, y: end.y - offset * direction };
  } else {
    const offset = Math.min(Math.abs(dx) * 0.5, 80);
    const direction = dx > 0 ? 1 : -1;
    cp1 = { x: start.x + offset * direction, y: start.y };
    cp2 = { x: end.x - offset * direction, y: end.y };
  }
  return { cp1, cp2 };
}

/**
 * Compute the Catmull-Rom control points for a segment from points[i] to points[i+1],
 * matching the buildSmoothPath logic.
 */
function getSmoothSegmentControlPoints(
  points: { x: number; y: number }[],
  i: number,
): { cp1: { x: number; y: number }; cp2: { x: number; y: number } } {
  const p0 = points[Math.max(0, i - 1)];
  const p1 = points[i];
  const p2 = points[i + 1];
  const p3 = points[Math.min(points.length - 1, i + 2)];

  return {
    cp1: {
      x: p1.x + (p2.x - p0.x) / 6,
      y: p1.y + (p2.y - p0.y) / 6,
    },
    cp2: {
      x: p2.x - (p3.x - p1.x) / 6,
      y: p2.y - (p3.y - p1.y) / 6,
    },
  };
}

/**
 * Compute the point at t=0.5 on the actual curve segment between
 * allPoints[segIdx] and allPoints[segIdx+1].
 *
 * For a 2-point path this uses the same control-point logic as buildCurvePath.
 * For a multi-point path this uses the Catmull-Rom control points from buildSmoothPath.
 */
function curveSegmentMidpoint(
  allPoints: { x: number; y: number }[],
  segIdx: number,
): { x: number; y: number } {
  const p1 = allPoints[segIdx];
  const p2 = allPoints[segIdx + 1];

  if (allPoints.length === 2) {
    // 2-point path — mirrors buildCurvePath
    const { cp1, cp2 } = getCurveControlPoints(p1, p2);
    return evaluateCubicBezier(p1, cp1, cp2, p2, 0.5);
  }

  // Multi-point path — mirrors buildSmoothPath (Catmull-Rom → cubic Bezier)
  const { cp1, cp2 } = getSmoothSegmentControlPoints(allPoints, segIdx);
  return evaluateCubicBezier(p1, cp1, cp2, p2, 0.5);
}

function buildArrowHeadPath(
  from: { x: number; y: number },
  tip: { x: number; y: number },
): {
  path: string;
} {
  const dx = tip.x - from.x;
  const dy = tip.y - from.y;
  const length = Math.hypot(dx, dy) || 1;
  const ux = dx / length;
  const uy = dy / length;
  const baseX = tip.x - ux * ARROW_LENGTH;
  const baseY = tip.y - uy * ARROW_LENGTH;
  const perpX = -uy;
  const perpY = ux;

  return {
    path: `M ${tip.x} ${tip.y} L ${baseX + perpX * ARROW_HALF_WIDTH} ${baseY + perpY * ARROW_HALF_WIDTH} L ${baseX - perpX * ARROW_HALF_WIDTH} ${baseY - perpY * ARROW_HALF_WIDTH} Z`,
  };
}

function getArrowDirectionPoint(
  allPoints: { x: number; y: number }[],
  lookbackPx: number = ARROW_DIRECTION_LOOKBACK_PX,
): { x: number; y: number } {
  if (allPoints.length < 2) {
    return allPoints[0] ?? { x: 0, y: 0 };
  }

  const segIdx = allPoints.length - 2;
  const start = allPoints[segIdx];
  const end = allPoints[segIdx + 1];
  const { cp1, cp2 } = getSegmentControlPoints(allPoints, segIdx);

  let prev = end;
  let traversed = 0;

  for (let i = ARROW_DIRECTION_SAMPLES - 1; i >= 0; i--) {
    const t = i / ARROW_DIRECTION_SAMPLES;
    const pt = evaluateCubicBezier(start, cp1, cp2, end, t);
    traversed += Math.hypot(prev.x - pt.x, prev.y - pt.y);
    if (traversed >= lookbackPx) {
      return pt;
    }
    prev = pt;
  }

  return start;
}

function lerpPoint(
  a: { x: number; y: number },
  b: { x: number; y: number },
  t: number,
): { x: number; y: number } {
  return {
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
  };
}

function splitCubicAtT(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number },
  t: number,
): {
  leftCp1: { x: number; y: number };
  leftCp2: { x: number; y: number };
  point: { x: number; y: number };
} {
  const p01 = lerpPoint(p0, p1, t);
  const p12 = lerpPoint(p1, p2, t);
  const p23 = lerpPoint(p2, p3, t);
  const p012 = lerpPoint(p01, p12, t);
  const p123 = lerpPoint(p12, p23, t);
  const point = lerpPoint(p012, p123, t);

  return {
    leftCp1: p01,
    leftCp2: p012,
    point,
  };
}

function findTrimmedSegmentEnd(
  allPoints: { x: number; y: number }[],
  trimPx: number,
): {
  segIdx: number;
  t: number;
  point: { x: number; y: number };
} {
  const segIdx = allPoints.length - 2;
  const start = allPoints[segIdx];
  const end = allPoints[segIdx + 1];
  const { cp1, cp2 } = getSegmentControlPoints(allPoints, segIdx);

  let prev = end;
  let traversed = 0;

  for (let i = ARROW_DIRECTION_SAMPLES - 1; i >= 0; i--) {
    const t = i / ARROW_DIRECTION_SAMPLES;
    const pt = evaluateCubicBezier(start, cp1, cp2, end, t);
    traversed += Math.hypot(prev.x - pt.x, prev.y - pt.y);
    if (traversed >= trimPx) {
      return { segIdx, t, point: pt };
    }
    prev = pt;
  }

  return { segIdx, t: 0, point: start };
}

function buildTrimmedPath(
  allPoints: { x: number; y: number }[],
  trimPx: number,
): string {
  if (allPoints.length < 2) return "";
  if (trimPx <= 0) {
    return allPoints.length === 2
      ? buildCurvePath(allPoints[0], allPoints[1])
      : buildSmoothPath(allPoints);
  }

  const { segIdx, t } = findTrimmedSegmentEnd(allPoints, trimPx);
  const start = allPoints[segIdx];
  const end = allPoints[segIdx + 1];
  const { cp1, cp2 } = getSegmentControlPoints(allPoints, segIdx);
  const trimmed = splitCubicAtT(start, cp1, cp2, end, t);

  let d = `M ${allPoints[0].x} ${allPoints[0].y}`;

  for (let i = 0; i < segIdx; i++) {
    const p2 = allPoints[i + 1];
    const { cp1: segCp1, cp2: segCp2 } = getSegmentControlPoints(allPoints, i);
    d += ` C ${segCp1.x} ${segCp1.y}, ${segCp2.x} ${segCp2.y}, ${p2.x} ${p2.y}`;
  }

  d += ` C ${trimmed.leftCp1.x} ${trimmed.leftCp1.y}, ${trimmed.leftCp2.x} ${trimmed.leftCp2.y}, ${trimmed.point.x} ${trimmed.point.y}`;

  return d;
}

/** Check whether two SelectedEdgeInfo match the same edge */
function edgeMatches(
  sel: SelectedEdgeInfo | null | undefined,
  edge: WorkflowEdge,
): boolean {
  if (!sel) return false;
  return (
    sel.from === edge.from &&
    sel.to === edge.to &&
    sel.transitionIndex === edge.transitionIndex
  );
}

/** Drag state tracked via ref for performance */
interface DragState {
  type: "waypoint" | "label" | "new-waypoint";
  edgeFrom: string;
  edgeTo: string;
  edgeToName: string;
  transitionIndex: number;
  /** Index within the waypoints array (-1 for label) */
  waypointIndex: number;
  /** The full waypoints array snapshot at drag start (for new-waypoint, already includes the new point) */
  waypointsSnapshot: NodePosition[];
  startMouseX: number;
  startMouseY: number;
  startPointX: number;
  startPointY: number;
  /** Path points at drag start — used for label projection */
  pathPoints?: { x: number; y: number }[];
  /** Whether this edge is a self-loop — used for label projection */
  isSelfLoop?: boolean;
}

function WorkflowEdgesInner({
  edges,
  tasks,
  nodeWidth = NODE_WIDTH,
  nodeHeight = NODE_HEIGHT,
  connectingFrom,
  mousePosition,
  onEdgeClick,
  selectedEdge,
  onWaypointUpdate,
  onLabelPositionUpdate,
  screenToCanvas: screenToCanvasProp,
}: WorkflowEdgesProps) {
  const svgRef = useRef<SVGSVGElement>(null);

  const taskMap = useMemo(() => {
    const map = new Map<string, WorkflowTask>();
    for (const task of tasks) {
      map.set(task.id, task);
    }
    return map;
  }, [tasks]);

  const svgBounds = useMemo(() => {
    if (tasks.length === 0) return { width: 2000, height: 2000 };
    let minX = 0;
    let minY = 0;
    let maxX = 0;
    let maxY = 0;
    for (const task of tasks) {
      minX = Math.min(minX, task.position.x - 120);
      minY = Math.min(minY, task.position.y - 140);
      maxX = Math.max(
        maxX,
        task.position.x + nodeWidth + SELF_LOOP_RIGHT_OFFSET + 40,
      );
      maxY = Math.max(maxY, task.position.y + nodeHeight + 100);
    }
    return {
      width: Math.max(maxX - minX, 2000),
      height: Math.max(maxY - minY, 2000),
    };
  }, [tasks, nodeWidth, nodeHeight]);

  // ---- Drag state ----
  const dragStateRef = useRef<DragState | null>(null);
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);
  const dragPosRef = useRef<{ x: number; y: number } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  /** Current label t-parameter during drag — committed on mouseup */
  const labelDragTRef = useRef<number>(0.5);
  // Tracks which drag is active so we can match it to edge rendering
  const [activeDrag, setActiveDrag] = useState<{
    edgeFrom: string;
    edgeTo: string;
    transitionIndex: number;
    waypointIndex: number;
    type: "waypoint" | "label" | "new-waypoint";
    waypointsSnapshot: NodePosition[];
  } | null>(null);

  // ---- Midpoint hover state ----
  const [hoveredMidpoint, setHoveredMidpoint] = useState<{
    edgeFrom: string;
    edgeTo: string;
    transitionIndex: number;
    segmentIndex: number;
  } | null>(null);

  /** Convert client coordinates to canvas (SVG) coordinates.
   *  Uses the parent-provided screenToCanvas when available (handles zoom/pan),
   *  otherwise falls back to a basic rect-offset calculation. */
  const clientToSvg = useCallback(
    (clientX: number, clientY: number): { x: number; y: number } => {
      if (screenToCanvasProp) {
        return screenToCanvasProp(clientX, clientY);
      }
      const svg = svgRef.current;
      if (!svg) return { x: clientX, y: clientY };
      const rect = svg.getBoundingClientRect();
      const parent = svg.parentElement;
      const scrollLeft = parent?.scrollLeft ?? 0;
      const scrollTop = parent?.scrollTop ?? 0;
      return {
        x: clientX - rect.left + scrollLeft,
        y: clientY - rect.top + scrollTop,
      };
    },
    [screenToCanvasProp],
  );

  // Refs to hold latest callback values (updated via effects)
  const clientToSvgRef = useRef(clientToSvg);
  const onWaypointUpdateRef = useRef(onWaypointUpdate);
  const onLabelPositionUpdateRef = useRef(onLabelPositionUpdate);

  useEffect(() => {
    clientToSvgRef.current = clientToSvg;
  }, [clientToSvg]);
  useEffect(() => {
    onWaypointUpdateRef.current = onWaypointUpdate;
  }, [onWaypointUpdate]);
  useEffect(() => {
    onLabelPositionUpdateRef.current = onLabelPositionUpdate;
  }, [onLabelPositionUpdate]);

  // Effect-based drag listener management
  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const ds = dragStateRef.current;
      if (!ds) return;
      const svgPos = clientToSvgRef.current(e.clientX, e.clientY);

      if (ds.type === "label" && ds.pathPoints) {
        // Project mouse onto path and snap the label to it
        const t = projectOntoPath(ds.pathPoints, svgPos, ds.isSelfLoop);
        labelDragTRef.current = t;
        const onCurve = evaluatePathAtT(ds.pathPoints, t, ds.isSelfLoop);
        setDragPos(onCurve);
        dragPosRef.current = onCurve;
      } else {
        const dx = svgPos.x - ds.startMouseX;
        const dy = svgPos.y - ds.startMouseY;
        const newPos = {
          x: ds.startPointX + dx,
          y: ds.startPointY + dy,
        };
        setDragPos(newPos);
        dragPosRef.current = newPos;
      }
    };

    const handleMouseUp = () => {
      const ds = dragStateRef.current;
      if (!ds) return;

      const currentDragPos = dragPosRef.current;

      if (ds.type === "waypoint" || ds.type === "new-waypoint") {
        const finalWaypoints = [...ds.waypointsSnapshot];
        if (currentDragPos) {
          finalWaypoints[ds.waypointIndex] = {
            x: currentDragPos.x,
            y: currentDragPos.y,
          };
        }
        onWaypointUpdateRef.current?.(
          ds.edgeFrom,
          ds.transitionIndex,
          ds.edgeToName,
          finalWaypoints,
        );
      } else if (ds.type === "label") {
        onLabelPositionUpdateRef.current?.(
          ds.edgeFrom,
          ds.transitionIndex,
          ds.edgeToName,
          labelDragTRef.current,
        );
      }

      dragStateRef.current = null;
      setDragPos(null);
      dragPosRef.current = null;
      setActiveDrag(null);
      setIsDragging(false);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging]);

  /** Start dragging an existing waypoint */
  const startWaypointDrag = useCallback(
    (
      e: React.MouseEvent,
      edge: WorkflowEdge,
      waypointIndex: number,
      currentWaypoints: NodePosition[],
    ) => {
      e.stopPropagation();
      e.preventDefault();

      const svgPos = clientToSvg(e.clientX, e.clientY);
      const wp = currentWaypoints[waypointIndex];

      dragStateRef.current = {
        type: "waypoint",
        edgeFrom: edge.from,
        edgeTo: edge.to,
        edgeToName: edge.toName,
        transitionIndex: edge.transitionIndex,
        waypointIndex,
        waypointsSnapshot: [...currentWaypoints],
        startMouseX: svgPos.x,
        startMouseY: svgPos.y,
        startPointX: wp.x,
        startPointY: wp.y,
      };

      setDragPos({ x: wp.x, y: wp.y });
      dragPosRef.current = { x: wp.x, y: wp.y };
      setActiveDrag({
        edgeFrom: edge.from,
        edgeTo: edge.to,
        transitionIndex: edge.transitionIndex,
        waypointIndex,
        type: "waypoint",
        waypointsSnapshot: [...currentWaypoints],
      });
      setIsDragging(true);
    },
    [clientToSvg],
  );

  /** Start dragging the label along the path */
  const startLabelDrag = useCallback(
    (
      e: React.MouseEvent,
      edge: WorkflowEdge,
      currentLabelPos: { x: number; y: number },
      allPoints: { x: number; y: number }[],
      selfLoop?: boolean,
    ) => {
      e.stopPropagation();
      e.preventDefault();

      const svgPos = clientToSvg(e.clientX, e.clientY);

      // Initialise t from current label position
      const initialT = projectOntoPath(allPoints, currentLabelPos, selfLoop);
      labelDragTRef.current = initialT;

      dragStateRef.current = {
        type: "label",
        edgeFrom: edge.from,
        edgeTo: edge.to,
        edgeToName: edge.toName,
        transitionIndex: edge.transitionIndex,
        waypointIndex: -1,
        waypointsSnapshot: [],
        startMouseX: svgPos.x,
        startMouseY: svgPos.y,
        startPointX: currentLabelPos.x,
        startPointY: currentLabelPos.y,
        pathPoints: allPoints,
        isSelfLoop: selfLoop,
      };

      setDragPos({ x: currentLabelPos.x, y: currentLabelPos.y });
      dragPosRef.current = { x: currentLabelPos.x, y: currentLabelPos.y };
      setActiveDrag({
        edgeFrom: edge.from,
        edgeTo: edge.to,
        transitionIndex: edge.transitionIndex,
        waypointIndex: -1,
        type: "label",
        waypointsSnapshot: [],
      });
      setIsDragging(true);
    },
    [clientToSvg],
  );

  /** Add a new waypoint at a midpoint and begin dragging it */
  const addAndDragWaypoint = useCallback(
    (
      e: React.MouseEvent,
      edge: WorkflowEdge,
      segmentIndex: number,
      allPoints: { x: number; y: number }[],
      currentWaypoints: NodePosition[],
    ) => {
      e.stopPropagation();
      e.preventDefault();

      const mid = curveSegmentMidpoint(allPoints, segmentIndex);
      const newWaypointIdx = segmentIndex;
      const updatedWaypoints = [...currentWaypoints];
      updatedWaypoints.splice(newWaypointIdx, 0, { x: mid.x, y: mid.y });

      const svgPos = clientToSvg(e.clientX, e.clientY);

      dragStateRef.current = {
        type: "new-waypoint",
        edgeFrom: edge.from,
        edgeTo: edge.to,
        edgeToName: edge.toName,
        transitionIndex: edge.transitionIndex,
        waypointIndex: newWaypointIdx,
        waypointsSnapshot: updatedWaypoints,
        startMouseX: svgPos.x,
        startMouseY: svgPos.y,
        startPointX: mid.x,
        startPointY: mid.y,
      };

      setDragPos({ x: mid.x, y: mid.y });
      dragPosRef.current = { x: mid.x, y: mid.y };
      setActiveDrag({
        edgeFrom: edge.from,
        edgeTo: edge.to,
        transitionIndex: edge.transitionIndex,
        waypointIndex: newWaypointIdx,
        type: "new-waypoint",
        waypointsSnapshot: updatedWaypoints,
      });
      setHoveredMidpoint(null);
      setIsDragging(true);
    },
    [clientToSvg],
  );

  /** Handle clicking the midpoint indicator (create waypoint without drag) */
  const handleMidpointClick = useCallback(
    (
      e: React.MouseEvent,
      edge: WorkflowEdge,
      segmentIndex: number,
      allPoints: { x: number; y: number }[],
      currentWaypoints: NodePosition[],
    ) => {
      e.stopPropagation();
      e.preventDefault();

      const mid = curveSegmentMidpoint(allPoints, segmentIndex);
      const newWaypointIdx = segmentIndex;
      const updatedWaypoints = [...currentWaypoints];
      updatedWaypoints.splice(newWaypointIdx, 0, { x: mid.x, y: mid.y });

      onWaypointUpdate?.(
        edge.from,
        edge.transitionIndex,
        edge.toName,
        updatedWaypoints,
      );
      setHoveredMidpoint(null);
    },
    [onWaypointUpdate],
  );

  /** Remove a waypoint on double-click */
  const handleWaypointDoubleClick = useCallback(
    (
      e: React.MouseEvent,
      edge: WorkflowEdge,
      waypointIndex: number,
      currentWaypoints: NodePosition[],
    ) => {
      e.stopPropagation();
      e.preventDefault();
      const updated = [...currentWaypoints];
      updated.splice(waypointIndex, 1);
      onWaypointUpdate?.(edge.from, edge.transitionIndex, edge.toName, updated);
    },
    [onWaypointUpdate],
  );

  /** Reset label to default position (t=0.5) on double-click */
  const handleLabelDoubleClick = useCallback(
    (e: React.MouseEvent, edge: WorkflowEdge) => {
      e.stopPropagation();
      e.preventDefault();
      onLabelPositionUpdate?.(
        edge.from,
        edge.transitionIndex,
        edge.toName,
        undefined,
      );
    },
    [onLabelPositionUpdate],
  );

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
      ref={svgRef}
      className="absolute inset-0 pointer-events-none overflow-visible"
      width={svgBounds.width}
      height={svgBounds.height}
      style={{ zIndex: 1 }}
    >
      <g className="pointer-events-auto">
        {/* Render edges */}
        {edges.map((edge, index) => {
          const fromTask = taskMap.get(edge.from);
          const toTask = taskMap.get(edge.to);
          if (!fromTask || !toTask) return null;
          const isSelfLoopEdge = edge.from === edge.to;

          // Build the current waypoints first so we can pass them into
          // connection-point selection as an approach hint.
          let currentWaypoints: NodePosition[] =
            !isSelfLoopEdge && edge.waypoints ? [...edge.waypoints] : [];
          if (
            !isSelfLoopEdge &&
            activeDrag &&
            activeDrag.edgeFrom === edge.from &&
            activeDrag.edgeTo === edge.to &&
            activeDrag.transitionIndex === edge.transitionIndex &&
            (activeDrag.type === "waypoint" ||
              activeDrag.type === "new-waypoint")
          ) {
            currentWaypoints = [...activeDrag.waypointsSnapshot];
            if (dragPos) {
              currentWaypoints[activeDrag.waypointIndex] = {
                x: dragPos.x,
                y: dragPos.y,
              };
            }
          }

          const { start, end, selfLoop } = getBestConnectionPoints(
            fromTask,
            toTask,
            nodeWidth,
            nodeHeight,
            currentWaypoints.length > 0 ? currentWaypoints : undefined,
          );

          const isSelected = edgeMatches(selectedEdge, edge);

          const selfLoopRoute =
            selfLoop && currentWaypoints.length === 0
              ? buildSelfLoopRoute(fromTask, nodeWidth, nodeHeight)
              : null;

          const color =
            edge.color || EDGE_COLORS[edge.type] || EDGE_COLORS.complete;
          const dash = edge.lineStyle ? LINE_STYLE_DASH[edge.lineStyle] : "";
          const groupOpacity = isSelected ? 1 : 0.75;

          // Label position — evaluate t-parameter on the actual path
          let labelPos: { x: number; y: number };
          const usesDefaultSelfLoopRoute =
            selfLoop && currentWaypoints.length === 0;
          const allPoints = selfLoopRoute ?? [start, ...currentWaypoints, end];
          if (
            activeDrag &&
            activeDrag.type === "label" &&
            activeDrag.edgeFrom === edge.from &&
            activeDrag.edgeTo === edge.to &&
            activeDrag.transitionIndex === edge.transitionIndex &&
            dragPos
          ) {
            // During drag, dragPos is already snapped to the curve
            labelPos = dragPos;
          } else {
            const t =
              edge.labelPosition ?? (usesDefaultSelfLoopRoute ? 0.62 : 0.5);
            labelPos = evaluatePathAtT(allPoints, t, usesDefaultSelfLoopRoute);
          }
          const arrowDirectionPoint = getArrowDirectionPoint(allPoints);
          const arrowHead = buildArrowHeadPath(arrowDirectionPoint, end);
          const pathD = buildTrimmedPath(
            allPoints,
            ARROW_LENGTH - ARROW_SHAFT_OVERLAP_PX,
          );

          const labelText = edge.label || "";
          const labelWidth = Math.max(labelText.length * 5.5 + 12, 48);

          return (
            <g
              key={`edge-${index}-${edge.from}-${edge.to}`}
              opacity={groupOpacity}
            >
              {/* Edge path */}
              <path
                d={pathD}
                fill="none"
                stroke={color}
                strokeWidth={isSelected ? 2.5 : 2}
                strokeDasharray={dash}
                className="transition-opacity"
              />
              <path
                d={arrowHead.path}
                fill={color}
                className="pointer-events-none transition-opacity"
              />

              {/* Selection glow for selected edge */}
              {isSelected && (
                <path
                  d={pathD}
                  fill="none"
                  stroke={color}
                  strokeWidth={6}
                  opacity={0.15}
                  className="pointer-events-none"
                />
              )}

              {/* Wider invisible path for easier clicking */}
              <path
                d={pathD}
                fill="none"
                stroke="transparent"
                strokeWidth={14}
                className="cursor-pointer"
                onClick={(e) => {
                  e.stopPropagation();
                  onEdgeClick?.({
                    taskId: edge.from,
                    targetTaskId: edge.to,
                    transitionIndex: edge.transitionIndex,
                  });
                }}
              />

              {/* Label */}
              {edge.label && (
                <g
                  className={isSelected ? "cursor-grab" : "cursor-default"}
                  onMouseDown={
                    isSelected
                      ? (e) =>
                          startLabelDrag(
                            e,
                            edge,
                            labelPos,
                            allPoints,
                            usesDefaultSelfLoopRoute,
                          )
                      : undefined
                  }
                  onDoubleClick={
                    isSelected
                      ? (e) => handleLabelDoubleClick(e, edge)
                      : undefined
                  }
                  onClick={(e) => {
                    e.stopPropagation();
                    onEdgeClick?.({
                      taskId: edge.from,
                      targetTaskId: edge.to,
                      transitionIndex: edge.transitionIndex,
                    });
                  }}
                >
                  <rect
                    x={labelPos.x - labelWidth / 2}
                    y={labelPos.y - 8}
                    width={labelWidth}
                    height={16}
                    rx={4}
                    fill="white"
                    stroke={isSelected ? color : color}
                    strokeWidth={isSelected ? 1.5 : 0.5}
                    opacity={0.95}
                  />
                  <text
                    x={labelPos.x}
                    y={labelPos.y + 3.5}
                    textAnchor="middle"
                    fontSize={9}
                    fontWeight={isSelected ? 600 : 500}
                    fill={color}
                    className="select-none pointer-events-none"
                  >
                    {labelText.length > 24
                      ? labelText.slice(0, 21) + "..."
                      : labelText}
                  </text>
                  {/* Drag hint icon when selected */}
                  {isSelected && (
                    <text
                      x={labelPos.x + labelWidth / 2 - 10}
                      y={labelPos.y + 3.5}
                      textAnchor="middle"
                      fontSize={8}
                      fill={color}
                      opacity={0.5}
                      className="select-none pointer-events-none"
                    >
                      ⋮⋮
                    </text>
                  )}
                </g>
              )}

              {/* === Selected edge interactive elements === */}
              {isSelected && !isSelfLoopEdge && (
                <>
                  {/* Waypoint handles */}
                  {currentWaypoints.map((wp, wpIdx) => {
                    const isDragging =
                      activeDrag &&
                      activeDrag.edgeFrom === edge.from &&
                      activeDrag.edgeTo === edge.to &&
                      activeDrag.transitionIndex === edge.transitionIndex &&
                      activeDrag.waypointIndex === wpIdx &&
                      (activeDrag.type === "waypoint" ||
                        activeDrag.type === "new-waypoint");

                    return (
                      <g
                        key={`wp-${wpIdx}`}
                        className={
                          isDragging ? "cursor-grabbing" : "cursor-grab"
                        }
                        onMouseDown={(e) =>
                          startWaypointDrag(e, edge, wpIdx, currentWaypoints)
                        }
                        onDoubleClick={(e) =>
                          handleWaypointDoubleClick(
                            e,
                            edge,
                            wpIdx,
                            currentWaypoints,
                          )
                        }
                      >
                        {/* Outer ring on hover/drag */}
                        <circle
                          cx={wp.x}
                          cy={wp.y}
                          r={isDragging ? 10 : 8}
                          fill={color}
                          opacity={isDragging ? 0.15 : 0}
                          className="transition-opacity"
                        >
                          <set
                            attributeName="opacity"
                            to="0.12"
                            begin="mouseover"
                          />
                          <set
                            attributeName="opacity"
                            to="0"
                            begin="mouseout"
                          />
                        </circle>
                        {/* Handle circle */}
                        <circle
                          cx={wp.x}
                          cy={wp.y}
                          r={isDragging ? 6 : 5}
                          fill={color}
                          stroke="white"
                          strokeWidth={2}
                          opacity={1}
                          className="transition-[r]"
                        />
                        {/* Invisible larger hit area */}
                        <circle cx={wp.x} cy={wp.y} r={12} fill="transparent" />
                      </g>
                    );
                  })}

                  {/* Midpoint add-waypoint hover zones */}
                  {allPoints.slice(0, -1).map((pt, segIdx) => {
                    const nextPt = allPoints[segIdx + 1];
                    const mid = curveSegmentMidpoint(allPoints, segIdx);
                    const segDist = Math.hypot(
                      nextPt.x - pt.x,
                      nextPt.y - pt.y,
                    );
                    // Don't show for very short segments
                    if (segDist < 40) return null;

                    const isHovered =
                      hoveredMidpoint !== null &&
                      hoveredMidpoint.edgeFrom === edge.from &&
                      hoveredMidpoint.edgeTo === edge.to &&
                      hoveredMidpoint.transitionIndex ===
                        edge.transitionIndex &&
                      hoveredMidpoint.segmentIndex === segIdx;

                    return (
                      <g
                        key={`mid-${segIdx}`}
                        className="cursor-copy"
                        onMouseEnter={() =>
                          setHoveredMidpoint({
                            edgeFrom: edge.from,
                            edgeTo: edge.to,
                            transitionIndex: edge.transitionIndex,
                            segmentIndex: segIdx,
                          })
                        }
                        onMouseLeave={() => setHoveredMidpoint(null)}
                        onMouseDown={(e) =>
                          addAndDragWaypoint(
                            e,
                            edge,
                            segIdx,
                            allPoints,
                            currentWaypoints,
                          )
                        }
                        onClick={(e) =>
                          handleMidpointClick(
                            e,
                            edge,
                            segIdx,
                            allPoints,
                            currentWaypoints,
                          )
                        }
                      >
                        {/* Invisible hit area along midpoint region */}
                        <circle
                          cx={mid.x}
                          cy={mid.y}
                          r={16}
                          fill="transparent"
                        />
                        {/* Visible indicator - fades in on hover */}
                        <circle
                          cx={mid.x}
                          cy={mid.y}
                          r={isHovered ? 7 : 5}
                          fill="white"
                          stroke={color}
                          strokeWidth={1.5}
                          strokeDasharray={isHovered ? "" : "2,2"}
                          opacity={isHovered ? 1 : 0.7}
                          className="transition-[r,opacity,stroke-dasharray] duration-150"
                        />
                        {/* Plus icon */}
                        <text
                          x={mid.x}
                          y={mid.y + 3.5}
                          textAnchor="middle"
                          fontSize={10}
                          fontWeight={700}
                          fill={color}
                          opacity={isHovered ? 1 : 0.7}
                          className="select-none pointer-events-none transition-opacity duration-150"
                        >
                          +
                        </text>
                      </g>
                    );
                  })}
                </>
              )}
            </g>
          );
        })}
      </g>

      {/* Preview line */}
      {previewLine}
    </svg>
  );
}

const WorkflowEdges = memo(WorkflowEdgesInner);
export default WorkflowEdges;
