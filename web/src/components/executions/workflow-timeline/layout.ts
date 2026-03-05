/**
 * Layout Engine for the Workflow Timeline DAG.
 *
 * Responsible for:
 *   1. Computing the time→pixel x-scale from task time bounds.
 *   2. Assigning tasks to non-overlapping y-lanes (greedy packing).
 *   3. Positioning milestone nodes.
 *   4. Producing the final ComputedLayout consumed by the SVG renderer.
 */

import type {
  TimelineTask,
  TimelineEdge,
  TimelineMilestone,
  TimelineNode,
  ComputedLayout,
  LayoutConfig,
} from "./types";
import { DEFAULT_LAYOUT } from "./types";

// ---------------------------------------------------------------------------
// Time scale helpers
// ---------------------------------------------------------------------------

interface TimeScale {
  /** Minimum time (epoch ms) */
  minMs: number;
  /** Maximum time (epoch ms) */
  maxMs: number;
  /** Available pixel width for the time axis */
  axisWidth: number;
  /** Pixels per millisecond */
  pxPerMs: number;
}

function buildTimeScale(
  tasks: TimelineTask[],
  milestones: TimelineMilestone[],
  chartWidth: number,
  config: LayoutConfig,
): TimeScale {
  // Collect all time values
  const times: number[] = [];
  for (const t of tasks) {
    if (t.startMs != null) times.push(t.startMs);
    if (t.endMs != null) times.push(t.endMs);
  }
  for (const m of milestones) {
    times.push(m.timeMs);
  }

  if (times.length === 0) {
    // Fallback: a 10-second window around now
    const now = Date.now();
    times.push(now - 5000, now + 5000);
  }

  let minMs = Math.min(...times);
  let maxMs = Math.max(...times);

  // Add a small buffer so nodes at the edges aren't right on the border
  const rangeMs = maxMs - minMs;
  const bufferMs = Math.max(rangeMs * 0.04, 200); // at least 200ms buffer
  minMs -= bufferMs;
  maxMs += bufferMs;

  const axisWidth = chartWidth - config.paddingLeft - config.paddingRight;
  const pxPerMs = axisWidth / Math.max(maxMs - minMs, 1);

  return { minMs, maxMs, axisWidth, pxPerMs };
}

/** Convert a timestamp (epoch ms) to an x pixel position */
function timeToPx(ms: number, scale: TimeScale, config: LayoutConfig): number {
  return config.paddingLeft + (ms - scale.minMs) * scale.pxPerMs;
}

// ---------------------------------------------------------------------------
// Lane assignment (greedy packing)
// ---------------------------------------------------------------------------

interface LaneInterval {
  /** Left x pixel (inclusive) */
  left: number;
  /** Right x pixel (inclusive) */
  right: number;
}

/**
 * Assign each task to the first lane where it doesn't overlap with
 * any existing task bar in that lane.
 *
 * Tasks are sorted by startTime (earliest first), then by duration
 * descending (longer bars first) to maximise packing efficiency.
 *
 * After initial packing we optionally reorder lanes so tasks with
 * shared upstream dependencies are adjacent.
 */
function assignLanes(
  tasks: TimelineTask[],
  scale: TimeScale,
  config: LayoutConfig,
): Map<string, number> {
  // Build a sortable list with pixel extents
  type Entry = {
    task: TimelineTask;
    left: number;
    right: number;
  };

  const entries: Entry[] = tasks.map((t) => {
    const left = t.startMs != null ? timeToPx(t.startMs, scale, config) : 0;
    let right =
      t.endMs != null
        ? timeToPx(t.endMs, scale, config)
        : left + config.minBarWidth;
    // Ensure minimum width
    if (right - left < config.minBarWidth) {
      right = left + config.minBarWidth;
    }
    return { task: t, left, right };
  });

  // Sort: by start position, then by width descending (longer bars first)
  entries.sort((a, b) => {
    if (a.left !== b.left) return a.left - b.left;
    return b.right - b.left - (a.right - a.left);
  });

  // Greedy lane packing
  const lanes: LaneInterval[][] = []; // lanes[laneIndex] = list of intervals
  const assignment = new Map<string, number>();

  for (const entry of entries) {
    let placed = false;
    const gap = 4; // minimum px gap between bars in the same lane

    for (let lane = 0; lane < lanes.length; lane++) {
      const intervals = lanes[lane];
      const overlaps = intervals.some(
        (iv) => entry.left < iv.right + gap && entry.right + gap > iv.left,
      );
      if (!overlaps) {
        intervals.push({ left: entry.left, right: entry.right });
        assignment.set(entry.task.id, lane);
        placed = true;
        break;
      }
    }

    if (!placed) {
      // Open a new lane
      lanes.push([{ left: entry.left, right: entry.right }]);
      assignment.set(entry.task.id, lanes.length - 1);
    }
  }

  // --- Optional lane reordering to cluster related tasks ---
  // Build a lane affinity score based on shared upstream dependencies.
  // We do a simple bubble-pass: for each pair of adjacent lanes,
  // if swapping them increases the total number of adjacent upstream-sharing
  // task pairs, do the swap.
  const laneCount = lanes.length;
  if (laneCount > 2) {
    const laneIds: number[] = Array.from({ length: laneCount }, (_, i) => i);

    // Build lane→taskIds mapping
    const tasksByLane = new Map<number, string[]>();
    for (const [taskId, lane] of assignment) {
      const list = tasksByLane.get(lane) ?? [];
      list.push(taskId);
      tasksByLane.set(lane, list);
    }

    // Build a task→upstreams lookup
    const taskUpstreams = new Map<string, Set<string>>();
    for (const t of tasks) {
      taskUpstreams.set(t.id, new Set(t.upstreamIds));
    }

    // Affinity between two lanes: count of task pairs that share upstream deps
    function laneAffinity(laneA: number, laneB: number): number {
      const aTasks = tasksByLane.get(laneA) ?? [];
      const bTasks = tasksByLane.get(laneB) ?? [];
      let score = 0;
      for (const a of aTasks) {
        const aUp = taskUpstreams.get(a);
        if (!aUp || aUp.size === 0) continue;
        for (const b of bTasks) {
          const bUp = taskUpstreams.get(b);
          if (!bUp || bUp.size === 0) continue;
          // Count shared upstreams
          for (const u of aUp) {
            if (bUp.has(u)) {
              score++;
              break; // one shared upstream is enough for this pair
            }
          }
        }
      }
      return score;
    }

    // Simple bubble sort passes (max 3 passes for stability)
    for (let pass = 0; pass < 3; pass++) {
      let swapped = false;
      for (let i = 0; i < laneIds.length - 1; i++) {
        const curr = laneIds[i];
        const next = laneIds[i + 1];

        // Check if swapping improves adjacency with neighbours
        const prev = i > 0 ? laneIds[i - 1] : -1;
        const after = i + 2 < laneIds.length ? laneIds[i + 2] : -1;

        let scoreBefore = 0;
        let scoreAfter = 0;

        if (prev >= 0) {
          scoreBefore += laneAffinity(prev, curr);
          scoreAfter += laneAffinity(prev, next);
        }
        if (after >= 0) {
          scoreBefore += laneAffinity(next, after);
          scoreAfter += laneAffinity(curr, after);
        }
        scoreBefore += laneAffinity(curr, next);
        scoreAfter += laneAffinity(next, curr); // same, symmetric

        if (scoreAfter > scoreBefore) {
          laneIds[i] = next;
          laneIds[i + 1] = curr;
          swapped = true;
        }
      }
      if (!swapped) break;
    }

    // Remap lane assignments to the reordered indices
    const reorderMap = new Map<number, number>();
    for (let newIdx = 0; newIdx < laneIds.length; newIdx++) {
      reorderMap.set(laneIds[newIdx], newIdx);
    }
    for (const [taskId, oldLane] of assignment) {
      assignment.set(taskId, reorderMap.get(oldLane) ?? oldLane);
    }
  }

  return assignment;
}

// ---------------------------------------------------------------------------
// Milestone lane assignment
// ---------------------------------------------------------------------------

/**
 * Position milestones in a lane that centres them vertically relative to
 * the tasks they connect to. Start and end milestones go to a middle lane.
 * Internal merge/fork milestones are placed at the median lane of their
 * connected tasks.
 */
function assignMilestoneLanes(
  milestones: TimelineMilestone[],
  milestoneEdges: TimelineEdge[],
  taskLanes: Map<string, number>,
  laneCount: number,
): Map<string, number> {
  const assignment = new Map<string, number>();
  const midLane = Math.max(0, Math.floor((laneCount - 1) / 2));

  for (const ms of milestones) {
    if (ms.kind === "start" || ms.kind === "end") {
      assignment.set(ms.id, midLane);
      continue;
    }

    // Gather lanes of connected tasks
    const connectedLanes: number[] = [];
    for (const e of milestoneEdges) {
      if (e.from === ms.id) {
        const lane = taskLanes.get(e.to);
        if (lane != null) connectedLanes.push(lane);
      }
      if (e.to === ms.id) {
        const lane = taskLanes.get(e.from);
        if (lane != null) connectedLanes.push(lane);
      }
    }

    if (connectedLanes.length > 0) {
      connectedLanes.sort((a, b) => a - b);
      const median = connectedLanes[Math.floor(connectedLanes.length / 2)];
      assignment.set(ms.id, median);
    } else {
      assignment.set(ms.id, midLane);
    }
  }

  return assignment;
}

// ---------------------------------------------------------------------------
// Build TimelineNode array
// ---------------------------------------------------------------------------

function buildNodes(
  tasks: TimelineTask[],
  milestones: TimelineMilestone[],
  taskLanes: Map<string, number>,
  milestoneLanes: Map<string, number>,
  scale: TimeScale,
  config: LayoutConfig,
): TimelineNode[] {
  const nodes: TimelineNode[] = [];

  // Task nodes
  for (const task of tasks) {
    const lane = taskLanes.get(task.id) ?? 0;
    const left =
      task.startMs != null
        ? timeToPx(task.startMs, scale, config)
        : timeToPx(
            scale.maxMs - (scale.maxMs - scale.minMs) * 0.05,
            scale,
            config,
          );
    let right =
      task.endMs != null
        ? timeToPx(task.endMs, scale, config)
        : left + config.minBarWidth;

    if (right - left < config.minBarWidth) {
      right = left + config.minBarWidth;
    }

    const y =
      config.paddingTop +
      lane * config.laneHeight +
      (config.laneHeight - config.barHeight) / 2;

    nodes.push({
      type: "task",
      id: task.id,
      lane,
      x: left,
      y,
      width: right - left,
      task,
    });
  }

  // Milestone nodes
  for (const ms of milestones) {
    const lane = milestoneLanes.get(ms.id) ?? 0;
    const x = timeToPx(ms.timeMs, scale, config);
    const y =
      config.paddingTop + lane * config.laneHeight + config.laneHeight / 2;

    nodes.push({
      type: "milestone",
      id: ms.id,
      lane,
      x,
      y,
      width: config.milestoneSize,
      milestone: ms,
    });
  }

  return nodes;
}

// ---------------------------------------------------------------------------
// Grid line computation
// ---------------------------------------------------------------------------

export interface GridLine {
  /** X pixel position */
  x: number;
  /** Human-readable label */
  label: string;
  /** Whether this is a major gridline (gets a label) */
  major: boolean;
}

/**
 * Compute vertical gridlines at "nice" time intervals.
 *
 * Picks an interval that gives roughly 6–12 major gridlines across
 * the visible chart width.
 */
export function computeGridLines(
  scale: TimeScale,
  config: LayoutConfig,
): GridLine[] {
  const rangeMs = scale.maxMs - scale.minMs;
  if (rangeMs <= 0) return [];

  // Target ~8 major gridlines
  const targetCount = 8;
  const rawInterval = rangeMs / targetCount;

  // Snap to a "nice" interval
  const niceIntervals = [
    100,
    200,
    500, // sub-second
    1000,
    2000,
    5000, // seconds
    10_000,
    15_000,
    30_000, // tens of seconds
    60_000,
    120_000,
    300_000, // minutes
    600_000,
    900_000,
    1_800_000, // tens of minutes
    3_600_000,
    7_200_000, // hours
    14_400_000,
    28_800_000,
    43_200_000, // multi-hour
    86_400_000, // day
  ];

  let interval = niceIntervals[0];
  for (const ni of niceIntervals) {
    interval = ni;
    if (ni >= rawInterval) break;
  }

  const lines: GridLine[] = [];

  // Start at the first "nice" multiple >= minMs
  const firstTick = Math.ceil(scale.minMs / interval) * interval;

  for (let ms = firstTick; ms <= scale.maxMs; ms += interval) {
    const x = timeToPx(ms, scale, config);
    lines.push({
      x,
      label: formatTimeLabel(ms, interval),
      major: true,
    });

    // Add a minor gridline halfway if the interval is large enough
    if (interval >= 2000) {
      const midMs = ms + interval / 2;
      if (midMs < scale.maxMs) {
        lines.push({
          x: timeToPx(midMs, scale, config),
          label: "",
          major: false,
        });
      }
    }
  }

  return lines;
}

/** Format a timestamp as a short label relative to the chart start */
function formatTimeLabel(ms: number, intervalMs: number): string {
  const date = new Date(ms);

  if (intervalMs >= 86_400_000) {
    // Days — show date
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  }

  if (intervalMs >= 3_600_000) {
    // Hours — show HH:MM
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  if (intervalMs >= 60_000) {
    // Minutes — show HH:MM:SS
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  if (intervalMs >= 1000) {
    // Seconds — show HH:MM:SS
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  // Sub-second — show with milliseconds
  return (
    date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }) +
    "." +
    String(date.getMilliseconds()).padStart(3, "0")
  );
}

// ---------------------------------------------------------------------------
// Public API: computeLayout
// ---------------------------------------------------------------------------

export function computeLayout(
  tasks: TimelineTask[],
  taskEdges: TimelineEdge[],
  milestones: TimelineMilestone[],
  milestoneEdges: TimelineEdge[],
  /** Desired chart width (pixels). The layout will use this for the x-scale. */
  chartWidth: number,
  configOverrides?: Partial<LayoutConfig>,
  /** Direct task→task edge keys that are replaced by milestone-routed paths.
   *  These are filtered out of `taskEdges` to avoid duplicate rendering. */
  suppressedEdgeKeys?: Set<string>,
): ComputedLayout {
  const config: LayoutConfig = { ...DEFAULT_LAYOUT, ...configOverrides };

  // Use a reasonable minimum width
  const effectiveWidth = Math.max(chartWidth, 400);

  // 1. Build time scale
  const scale = buildTimeScale(tasks, milestones, effectiveWidth, config);

  // 2. Assign task lanes
  const taskLanes = assignLanes(tasks, scale, config);

  // Count lanes
  let laneCount = 0;
  for (const lane of taskLanes.values()) {
    laneCount = Math.max(laneCount, lane + 1);
  }
  // Ensure at least 1 lane even if there are no tasks
  laneCount = Math.max(laneCount, 1);

  // 3. Assign milestone lanes
  const milestoneLanes = assignMilestoneLanes(
    milestones,
    milestoneEdges,
    taskLanes,
    laneCount,
  );

  // 4. Build node positions
  const nodes = buildNodes(
    tasks,
    milestones,
    taskLanes,
    milestoneLanes,
    scale,
    config,
  );

  // 5. Merge all edges, filtering out any task edges that have been
  //    replaced by milestone-routed paths (e.g. A→C replaced by A→merge→C).
  const filteredTaskEdges = suppressedEdgeKeys?.size
    ? taskEdges.filter((e) => !suppressedEdgeKeys.has(`${e.from}→${e.to}`))
    : taskEdges;
  const allEdges = [...filteredTaskEdges, ...milestoneEdges];

  // Deduplicate edges (same from→to)
  const edgeSet = new Set<string>();
  const dedupedEdges: TimelineEdge[] = [];
  for (const e of allEdges) {
    const key = `${e.from}→${e.to}`;
    if (!edgeSet.has(key)) {
      edgeSet.add(key);
      dedupedEdges.push(e);
    }
  }

  // 6. Compute total dimensions
  const totalWidth = effectiveWidth;
  const totalHeight =
    config.paddingTop + laneCount * config.laneHeight + config.paddingBottom;

  return {
    nodes,
    edges: dedupedEdges,
    totalWidth,
    totalHeight,
    laneCount,
    minTimeMs: scale.minMs,
    maxTimeMs: scale.maxMs,
    pxPerMs: scale.pxPerMs,
  };
}

// ---------------------------------------------------------------------------
// Bezier edge path generation
// ---------------------------------------------------------------------------

/**
 * Generate an SVG cubic Bezier path string for an edge between two nodes.
 *
 * Edges flow left→right. The control points bend horizontally so curves
 * are smooth and mostly follow the x-axis direction.
 *
 * Anchoring:
 *   - Task nodes: outgoing from right-center, incoming at left-center
 *   - Milestones: connect at center
 */
export function computeEdgePath(
  fromNode: TimelineNode,
  toNode: TimelineNode,
  config: LayoutConfig = DEFAULT_LAYOUT,
): string {
  let x1: number, y1: number, x2: number, y2: number;

  // Source anchor
  if (fromNode.type === "task") {
    x1 = fromNode.x + fromNode.width; // right edge
    y1 = fromNode.y + config.barHeight / 2; // vertical center
  } else {
    x1 = fromNode.x;
    y1 = fromNode.y;
  }

  // Target anchor
  if (toNode.type === "task") {
    x2 = toNode.x; // left edge
    y2 = toNode.y + config.barHeight / 2; // vertical center
  } else {
    x2 = toNode.x;
    y2 = toNode.y;
  }

  // Handle edge case where target is to the left of source (e.g., timing quirks)
  // In that case, draw a slight arc that loops
  const dx = x2 - x1;
  const dy = y2 - y1;

  if (dx < 5) {
    // Target is to the left or very close — use an S-curve that goes
    // slightly below/above and loops back
    const loopOffset = Math.max(30, Math.abs(dx) + 20);
    const yMid = (y1 + y2) / 2 + (dy >= 0 ? 20 : -20);

    return [
      `M ${x1} ${y1}`,
      `C ${x1 + loopOffset} ${y1}, ${x2 - loopOffset} ${yMid}, ${(x1 + x2) / 2} ${yMid}`,
      `C ${(x1 + x2) / 2 + loopOffset} ${yMid}, ${x2 - loopOffset} ${y2}, ${x2} ${y2}`,
    ].join(" ");
  }

  // Normal left→right Bezier
  // Control point offset: 40% of horizontal distance, clamped
  const cpOffset = Math.min(Math.max(dx * 0.4, 20), 120);

  const cx1 = x1 + cpOffset;
  const cy1 = y1;
  const cx2 = x2 - cpOffset;
  const cy2 = y2;

  return `M ${x1} ${y1} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${x2} ${y2}`;
}

// ---------------------------------------------------------------------------
// Export timeToPx for use by the renderer (gridlines etc.)
// ---------------------------------------------------------------------------

export { timeToPx, type TimeScale };
