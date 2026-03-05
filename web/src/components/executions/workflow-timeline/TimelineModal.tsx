/**
 * TimelineModal — Full-screen modal for the Workflow Timeline DAG.
 *
 * Opens as a portal overlay with:
 *   - A much larger vertical layout (more lane height, bigger bars)
 *   - A timescale zoom slider that re-computes the layout at wider widths
 *   - Horizontal scroll for zoomed-in views
 *   - All the same interactions as the inline renderer (hover, click, double-click)
 *   - Escape key / close button to dismiss
 */

import { useState, useRef, useCallback, useMemo, useEffect } from "react";
import { createPortal } from "react-dom";
import { X, ZoomIn, ZoomOut, RotateCcw, GitBranch } from "lucide-react";

import type {
  TimelineTask,
  TimelineEdge,
  TimelineMilestone,
  LayoutConfig,
  ComputedLayout,
} from "./types";
import { DEFAULT_LAYOUT } from "./types";
import { computeLayout } from "./layout";
import TimelineRenderer from "./TimelineRenderer";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface TimelineModalProps {
  /** Whether the modal is open */
  isOpen: boolean;
  /** Callback to close the modal */
  onClose: () => void;
  /** Timeline tasks */
  tasks: TimelineTask[];
  /** Structural dependency edges between tasks */
  taskEdges: TimelineEdge[];
  /** Synthetic milestone nodes */
  milestones: TimelineMilestone[];
  /** Edges connecting milestones */
  milestoneEdges: TimelineEdge[];
  /** Direct task→task edge keys replaced by milestone-routed paths */
  suppressedEdgeKeys?: Set<string>;
  /** Callback when a task is double-clicked (navigate to execution) */
  onTaskClick?: (task: TimelineTask) => void;
  /** Summary stats for the header */
  summary: {
    total: number;
    completed: number;
    failed: number;
    running: number;
    other: number;
    durationMs: number | null;
  };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** The modal layout uses more generous spacing */
const MODAL_LAYOUT: LayoutConfig = {
  ...DEFAULT_LAYOUT,
  laneHeight: 44,
  barHeight: 28,
  lanePadding: 8,
  milestoneSize: 12,
  paddingTop: 44,
  paddingBottom: 24,
  paddingLeft: 24,
  paddingRight: 24,
  minBarWidth: 12,
};

const MIN_ZOOM = 1;
const MAX_ZOOM = 8;
const ZOOM_STEP = 0.25;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function TimelineModal({
  isOpen,
  onClose,
  tasks,
  taskEdges,
  milestones,
  milestoneEdges,
  suppressedEdgeKeys,
  onTaskClick,
  summary,
}: TimelineModalProps) {
  const [zoom, setZoom] = useState(1);
  const scrollRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(1200);

  // ---- Observe container width ----
  useEffect(() => {
    if (!isOpen) return;
    const el = containerRef.current;
    if (!el) return;

    // Initial measurement
    setContainerWidth(el.clientWidth);

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.width > 0) {
          setContainerWidth(entry.contentRect.width);
        }
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [isOpen]);

  // ---- Keyboard handling (Escape to close) ----
  useEffect(() => {
    if (!isOpen) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [isOpen, onClose]);

  // ---- Prevent body scroll when modal is open ----
  useEffect(() => {
    if (!isOpen) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prev;
    };
  }, [isOpen]);

  // ---- Adjust layout config based on task count ----
  const layoutConfig: LayoutConfig = useMemo(() => {
    const taskCount = tasks.length;
    if (taskCount > 80) {
      return {
        ...MODAL_LAYOUT,
        laneHeight: 32,
        barHeight: 20,
        lanePadding: 6,
      };
    }
    if (taskCount > 40) {
      return {
        ...MODAL_LAYOUT,
        laneHeight: 38,
        barHeight: 24,
        lanePadding: 7,
      };
    }
    return MODAL_LAYOUT;
  }, [tasks.length]);

  // ---- Compute layout at the zoomed width ----
  const layout: ComputedLayout | null = useMemo(() => {
    if (tasks.length === 0) return null;
    // Zoom stretches the timeline horizontally
    const effectiveWidth = Math.max(containerWidth * zoom, 600);
    return computeLayout(
      tasks,
      taskEdges,
      milestones,
      milestoneEdges,
      effectiveWidth,
      layoutConfig,
      suppressedEdgeKeys,
    );
  }, [
    tasks,
    taskEdges,
    milestones,
    milestoneEdges,
    containerWidth,
    zoom,
    layoutConfig,
    suppressedEdgeKeys,
  ]);

  // ---- Zoom handlers ----
  const handleZoomIn = useCallback(() => {
    setZoom((z) => Math.min(MAX_ZOOM, z + ZOOM_STEP));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom((z) => Math.max(MIN_ZOOM, z - ZOOM_STEP));
  }, []);

  const handleZoomReset = useCallback(() => {
    setZoom(1);
    if (scrollRef.current) {
      scrollRef.current.scrollLeft = 0;
    }
  }, []);

  const handleZoomSlider = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setZoom(parseFloat(e.target.value));
    },
    [],
  );

  // ---- Wheel zoom on the timeline area ----
  const handleWheel = useCallback((e: React.WheelEvent) => {
    // Only zoom on Ctrl+wheel or meta+wheel to avoid interfering with normal scroll
    if (!e.ctrlKey && !e.metaKey) return;

    e.preventDefault();
    const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    setZoom((z) => {
      const newZoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, z + delta));
      return newZoom;
    });
  }, []);

  if (!isOpen) return null;

  const content = (
    <div
      className="fixed inset-0 z-50 flex flex-col"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.6)" }}
      onClick={(e) => {
        // Close on backdrop click
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* Modal container */}
      <div className="flex flex-col m-4 md:m-6 lg:m-8 bg-white rounded-xl shadow-2xl overflow-hidden flex-1 min-h-0">
        {/* ---- Header ---- */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 bg-gray-50/80 flex-shrink-0">
          <div className="flex items-center gap-3">
            <GitBranch className="h-4 w-4 text-indigo-500" />
            <h2 className="text-sm font-semibold text-gray-800">
              Workflow Timeline
            </h2>
            <span className="text-xs text-gray-400">
              {summary.total} task{summary.total !== 1 ? "s" : ""}
              {summary.durationMs != null && (
                <> · {formatDurationShort(summary.durationMs)}</>
              )}
            </span>

            {/* Summary badges */}
            <div className="flex items-center gap-1.5 ml-2">
              {summary.completed > 0 && (
                <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-medium bg-green-100 text-green-700">
                  {summary.completed} ✓
                </span>
              )}
              {summary.running > 0 && (
                <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-medium bg-blue-100 text-blue-700">
                  {summary.running} ⟳
                </span>
              )}
              {summary.failed > 0 && (
                <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-medium bg-red-100 text-red-700">
                  {summary.failed} ✗
                </span>
              )}
              {summary.other > 0 && (
                <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-medium bg-gray-100 text-gray-500">
                  {summary.other}
                </span>
              )}
            </div>
          </div>

          {/* Right: zoom controls + close */}
          <div className="flex items-center gap-3">
            {/* Zoom controls */}
            <div className="flex items-center gap-2 bg-white border border-gray-200 rounded-lg px-2.5 py-1.5 shadow-sm">
              <button
                onClick={handleZoomOut}
                disabled={zoom <= MIN_ZOOM}
                className="p-0.5 text-gray-500 hover:text-gray-800 disabled:text-gray-300 disabled:cursor-not-allowed"
                title="Zoom out"
              >
                <ZoomOut className="h-3.5 w-3.5" />
              </button>

              <input
                type="range"
                min={MIN_ZOOM}
                max={MAX_ZOOM}
                step={ZOOM_STEP}
                value={zoom}
                onChange={handleZoomSlider}
                className="w-24 h-1 accent-indigo-500 cursor-pointer"
                title={`Timescale: ${Math.round(zoom * 100)}%`}
              />

              <button
                onClick={handleZoomIn}
                disabled={zoom >= MAX_ZOOM}
                className="p-0.5 text-gray-500 hover:text-gray-800 disabled:text-gray-300 disabled:cursor-not-allowed"
                title="Zoom in"
              >
                <ZoomIn className="h-3.5 w-3.5" />
              </button>

              <span className="text-xs text-gray-500 font-mono tabular-nums w-10 text-center">
                {Math.round(zoom * 100)}%
              </span>

              {zoom !== 1 && (
                <button
                  onClick={handleZoomReset}
                  className="p-0.5 text-gray-400 hover:text-gray-700"
                  title="Reset zoom"
                >
                  <RotateCcw className="h-3 w-3" />
                </button>
              )}
            </div>

            {/* Close button */}
            <button
              onClick={onClose}
              className="p-1.5 text-gray-400 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
              title="Close (Esc)"
            >
              <X className="h-5 w-5" />
            </button>
          </div>
        </div>

        {/* ---- Legend ---- */}
        <div className="flex items-center gap-3 px-5 py-2 text-[10px] text-gray-400 border-b border-gray-100 flex-shrink-0">
          <LegendItem color="#22c55e" label="Completed" />
          <LegendItem color="#3b82f6" label="Running" />
          <LegendItem color="#ef4444" label="Failed" dashed />
          <LegendItem color="#f97316" label="Timeout" dotted />
          <LegendItem color="#9ca3af" label="Pending" />
          <span className="ml-2 text-gray-300">|</span>
          <EdgeLegendItem color="#22c55e" label="Succeeded" />
          <EdgeLegendItem color="#ef4444" label="Failed" dashed />
          <EdgeLegendItem color="#9ca3af" label="Always" />
          <span className="ml-auto text-gray-300">
            Ctrl+scroll to zoom · Click task to highlight path · Double-click to
            view
          </span>
        </div>

        {/* ---- Timeline body ---- */}
        <div
          ref={containerRef}
          className="flex-1 min-h-0 overflow-auto"
          onWheel={handleWheel}
        >
          {layout ? (
            <div ref={scrollRef} className="min-h-full">
              <TimelineRenderer
                layout={layout}
                tasks={tasks}
                config={layoutConfig}
                onTaskClick={onTaskClick}
                idPrefix="modal-"
              />
            </div>
          ) : (
            <div className="flex items-center justify-center h-full">
              <span className="text-sm text-gray-400">No tasks to display</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(content, document.body);
}

// ---------------------------------------------------------------------------
// Legend sub-components (duplicated from WorkflowTimelineDAG to keep modal
// self-contained — these are tiny presentational helpers)
// ---------------------------------------------------------------------------

function LegendItem({
  color,
  label,
  dashed,
  dotted,
}: {
  color: string;
  label: string;
  dashed?: boolean;
  dotted?: boolean;
}) {
  return (
    <span className="flex items-center gap-1">
      <span
        className="inline-block w-5 h-2.5 rounded-sm"
        style={{
          backgroundColor: color,
          opacity: 0.7,
          border: dashed
            ? `1px dashed ${color}`
            : dotted
              ? `1px dotted ${color}`
              : undefined,
        }}
      />
      <span>{label}</span>
    </span>
  );
}

function EdgeLegendItem({
  color,
  label,
  dashed,
}: {
  color: string;
  label: string;
  dashed?: boolean;
}) {
  return (
    <span className="flex items-center gap-1">
      <svg width="16" height="8" viewBox="0 0 16 8">
        <line
          x1="0"
          y1="4"
          x2="16"
          y2="4"
          stroke={color}
          strokeWidth="1.5"
          strokeDasharray={dashed ? "3 2" : undefined}
          opacity="0.7"
        />
        <polygon points="12,1 16,4 12,7" fill={color} opacity="0.6" />
      </svg>
      <span>{label}</span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDurationShort(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(1)}s`;
  const mins = Math.floor(secs / 60);
  const remainSecs = Math.round(secs % 60);
  if (mins < 60) return `${mins}m ${remainSecs}s`;
  const hrs = Math.floor(mins / 60);
  const remainMins = mins % 60;
  return `${hrs}h ${remainMins}m`;
}
