import { memo, useCallback, useRef, useState } from "react";
import { Trash2, Settings, GripVertical } from "lucide-react";
import type { WorkflowTask, TransitionPreset } from "@/types/workflow";
import {
  PRESET_LABELS,
  PRESET_WHEN,
  classifyTransitionWhen,
} from "@/types/workflow";

export type { TransitionPreset };

interface TaskNodeProps {
  task: WorkflowTask;
  isSelected: boolean;
  allTaskNames: string[];
  onSelect: (taskId: string) => void;
  onDelete: (taskId: string) => void;
  onPositionChange: (
    taskId: string,
    position: { x: number; y: number },
  ) => void;
  onStartConnection: (taskId: string, preset: TransitionPreset) => void;
  connectingFrom: { taskId: string; preset: TransitionPreset } | null;
  onCompleteConnection: (targetTaskId: string) => void;
}

/** Handle visual configuration for each transition preset */
const HANDLE_CONFIG: {
  preset: TransitionPreset;
  color: string;
  hoverColor: string;
  activeColor: string;
  ringColor: string;
}[] = [
  {
    preset: "succeeded",
    color: "#22c55e",
    hoverColor: "#16a34a",
    activeColor: "#15803d",
    ringColor: "rgba(34, 197, 94, 0.3)",
  },
  {
    preset: "failed",
    color: "#ef4444",
    hoverColor: "#dc2626",
    activeColor: "#b91c1c",
    ringColor: "rgba(239, 68, 68, 0.3)",
  },
  {
    preset: "always",
    color: "#6b7280",
    hoverColor: "#4b5563",
    activeColor: "#374151",
    ringColor: "rgba(107, 114, 128, 0.3)",
  },
];

/**
 * Check if a task has an active transition matching a given preset.
 */
function hasActiveTransition(
  task: WorkflowTask,
  preset: TransitionPreset,
): boolean {
  if (!task.next) return false;
  const whenExpr = PRESET_WHEN[preset];
  return task.next.some((t) => {
    if (whenExpr === undefined) return t.when === undefined;
    return (
      t.when?.toLowerCase().replace(/\s+/g, "") ===
      whenExpr.toLowerCase().replace(/\s+/g, "")
    );
  });
}

/**
 * Compute a short summary of outgoing transitions for the node body.
 */
function transitionSummary(task: WorkflowTask): string | null {
  if (!task.next || task.next.length === 0) return null;
  const totalTargets = task.next.reduce(
    (sum, t) => sum + (t.do?.length ?? 0),
    0,
  );
  if (
    totalTargets === 0 &&
    task.next.some((t) => t.publish && t.publish.length > 0)
  ) {
    return `${task.next.length} transition${task.next.length !== 1 ? "s" : ""} (publish only)`;
  }
  if (totalTargets === 0) return null;
  return `${totalTargets} target${totalTargets !== 1 ? "s" : ""} via ${task.next.length} transition${task.next.length !== 1 ? "s" : ""}`;
}

function TaskNodeInner({
  task,
  isSelected,
  onSelect,
  onDelete,
  onPositionChange,
  onStartConnection,
  connectingFrom,
  onCompleteConnection,
}: TaskNodeProps) {
  const nodeRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [hoveredHandle, setHoveredHandle] = useState<TransitionPreset | null>(
    null,
  );
  const [isInputHandleHovered, setIsInputHandleHovered] = useState(false);
  const dragOffset = useRef({ x: 0, y: 0 });

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest("[data-action-button]")) return;
      if (target.closest("[data-handle]")) return;

      e.stopPropagation();
      setIsDragging(true);
      dragOffset.current = {
        x: e.clientX - task.position.x,
        y: e.clientY - task.position.y,
      };

      const handleMouseMove = (moveEvent: MouseEvent) => {
        const newX = moveEvent.clientX - dragOffset.current.x;
        const newY = moveEvent.clientY - dragOffset.current.y;
        onPositionChange(task.id, {
          x: Math.max(0, newX),
          y: Math.max(0, newY),
        });
      };

      const handleMouseUp = () => {
        setIsDragging(false);
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [task.id, task.position.x, task.position.y, onPositionChange],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (connectingFrom && connectingFrom.taskId !== task.id) {
        onCompleteConnection(task.id);
      } else if (!connectingFrom) {
        onSelect(task.id);
      }
    },
    [task.id, onSelect, connectingFrom, onCompleteConnection],
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onDelete(task.id);
    },
    [task.id, onDelete],
  );

  const handleHandleMouseDown = useCallback(
    (e: React.MouseEvent, preset: TransitionPreset) => {
      e.stopPropagation();
      e.preventDefault();
      onStartConnection(task.id, preset);
    },
    [task.id, onStartConnection],
  );

  const handleInputHandleMouseUp = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (connectingFrom && connectingFrom.taskId !== task.id) {
        onCompleteConnection(task.id);
      }
    },
    [task.id, connectingFrom, onCompleteConnection],
  );

  const isConnectionTarget =
    connectingFrom !== null && connectingFrom.taskId !== task.id;

  const borderColor = isSelected
    ? "border-blue-500 ring-2 ring-blue-200"
    : isConnectionTarget
      ? "border-purple-400 ring-2 ring-purple-200"
      : "border-gray-300 hover:border-gray-400";

  const hasAction = task.action && task.action.length > 0;
  const summary = transitionSummary(task);

  // Count custom transitions (those not matching any preset)
  const customTransitionCount = (task.next || []).filter((t) => {
    const ct = classifyTransitionWhen(t.when);
    return ct === "custom";
  }).length;

  return (
    <div
      ref={nodeRef}
      className={`absolute select-none ${isDragging ? "cursor-grabbing z-50" : "cursor-grab z-10"}`}
      style={{
        left: task.position.x,
        top: task.position.y,
        width: 240,
      }}
      onMouseDown={handleMouseDown}
      onClick={handleClick}
    >
      {/* Input handle (top center) — drop target */}
      <div
        data-handle
        className="absolute left-1/2 -translate-x-1/2 -top-[7px] z-20"
        onMouseUp={handleInputHandleMouseUp}
        onMouseEnter={() => setIsInputHandleHovered(true)}
        onMouseLeave={() => setIsInputHandleHovered(false)}
      >
        <div
          className="transition-all duration-150 rounded-full border-2 border-white shadow-sm"
          style={{
            width:
              isConnectionTarget && isInputHandleHovered
                ? 16
                : isConnectionTarget
                  ? 14
                  : 10,
            height:
              isConnectionTarget && isInputHandleHovered
                ? 16
                : isConnectionTarget
                  ? 14
                  : 10,
            backgroundColor:
              isConnectionTarget && isInputHandleHovered
                ? "#8b5cf6"
                : isConnectionTarget
                  ? "#a78bfa"
                  : "#9ca3af",
            boxShadow:
              isConnectionTarget && isInputHandleHovered
                ? "0 0 0 4px rgba(139, 92, 246, 0.3), 0 1px 3px rgba(0,0,0,0.2)"
                : isConnectionTarget
                  ? "0 0 0 3px rgba(167, 139, 250, 0.3), 0 1px 2px rgba(0,0,0,0.15)"
                  : "0 1px 2px rgba(0,0,0,0.1)",
            cursor: isConnectionTarget ? "pointer" : "default",
          }}
        />
      </div>

      <div
        className={`bg-white rounded-lg border-2 shadow-sm transition-colors ${borderColor}`}
      >
        {/* Header */}
        <div className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-t-md bg-blue-500 bg-opacity-10 border-b border-gray-100">
          <GripVertical className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="font-semibold text-xs text-gray-900 truncate">
              {task.name}
            </div>
          </div>
        </div>

        {/* Body */}
        <div className="px-2.5 py-2">
          {hasAction ? (
            <div className="font-mono text-[11px] text-gray-600 truncate">
              {task.action}
            </div>
          ) : (
            <div className="text-[11px] text-orange-500 italic">
              No action assigned
            </div>
          )}

          {/* Input summary */}
          {Object.keys(task.input).length > 0 && (
            <div className="mt-1.5 text-[10px] text-gray-400">
              {Object.keys(task.input).length} input
              {Object.keys(task.input).length !== 1 ? "s" : ""}
            </div>
          )}

          {/* Transition summary */}
          {summary && (
            <div className="mt-1 text-[10px] text-gray-400">{summary}</div>
          )}

          {/* Delay badge */}
          {task.delay && (
            <div className="mt-1 inline-block px-1.5 py-0.5 bg-yellow-50 border border-yellow-200 rounded text-[10px] text-yellow-700 truncate max-w-full">
              delay: {task.delay}s
            </div>
          )}

          {/* With-items badge */}
          {task.with_items && (
            <div className="mt-1 inline-block px-1.5 py-0.5 bg-indigo-50 border border-indigo-200 rounded text-[10px] text-indigo-700 truncate max-w-full">
              with_items
            </div>
          )}

          {/* Retry badge */}
          {task.retry && (
            <div className="mt-1 inline-block px-1.5 py-0.5 bg-orange-50 border border-orange-200 rounded text-[10px] text-orange-700 ml-1">
              retry: {task.retry.count}×
            </div>
          )}

          {/* Custom transitions badge */}
          {customTransitionCount > 0 && (
            <div className="mt-1 inline-block px-1.5 py-0.5 bg-violet-50 border border-violet-200 rounded text-[10px] text-violet-700 ml-1">
              {customTransitionCount} custom transition
              {customTransitionCount !== 1 ? "s" : ""}
            </div>
          )}
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-end px-2 py-1.5 border-t border-gray-100 bg-gray-50 rounded-b-md">
          <div className="flex gap-1">
            <button
              data-action-button
              onClick={(e) => {
                e.stopPropagation();
                onSelect(task.id);
              }}
              className="p-1 rounded hover:bg-blue-100 text-gray-400 hover:text-blue-600 transition-colors"
              title="Configure task"
            >
              <Settings className="w-3 h-3" />
            </button>
            <button
              data-action-button
              onClick={handleDelete}
              className="p-1 rounded hover:bg-red-100 text-gray-400 hover:text-red-600 transition-colors"
              title="Delete task"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        </div>

        {/* Connection target overlay */}
        {isConnectionTarget && (
          <div className="absolute inset-0 rounded-lg bg-purple-100 bg-opacity-20 pointer-events-none flex items-center justify-center">
            <div className="text-xs font-medium text-purple-600 bg-white px-2 py-1 rounded shadow-sm">
              Drop to connect
            </div>
          </div>
        )}
      </div>

      {/* Output handles (bottom) — drag sources */}
      <div
        className="flex items-center justify-center gap-3 -mt-[7px] relative z-20"
        data-handle
      >
        {HANDLE_CONFIG.map((handle) => {
          const isActive = hasActiveTransition(task, handle.preset);
          const isHovered = hoveredHandle === handle.preset;
          const isCurrentlyDragging =
            connectingFrom?.taskId === task.id &&
            connectingFrom?.preset === handle.preset;

          return (
            <div
              key={handle.preset}
              className="relative group"
              onMouseEnter={() => setHoveredHandle(handle.preset)}
              onMouseLeave={() => setHoveredHandle(null)}
            >
              <div
                data-handle
                onMouseDown={(e) => handleHandleMouseDown(e, handle.preset)}
                className="transition-all duration-150 rounded-full border-2 border-white cursor-crosshair"
                style={{
                  width: isHovered || isCurrentlyDragging ? 14 : 10,
                  height: isHovered || isCurrentlyDragging ? 14 : 10,
                  backgroundColor: isCurrentlyDragging
                    ? handle.activeColor
                    : isHovered
                      ? handle.hoverColor
                      : isActive
                        ? handle.color
                        : `${handle.color}80`,
                  boxShadow: isCurrentlyDragging
                    ? `0 0 0 4px ${handle.ringColor}, 0 1px 3px rgba(0,0,0,0.2)`
                    : isHovered
                      ? `0 0 0 3px ${handle.ringColor}, 0 1px 2px rgba(0,0,0,0.15)`
                      : "0 1px 2px rgba(0,0,0,0.1)",
                }}
              />
              {/* Tooltip */}
              <div
                className={`absolute left-1/2 -translate-x-1/2 top-full mt-1.5 px-2 py-1 bg-gray-900 text-white text-[10px] font-medium rounded shadow-lg whitespace-nowrap pointer-events-none transition-opacity duration-150 ${
                  isHovered ? "opacity-100" : "opacity-0"
                }`}
              >
                {PRESET_LABELS[handle.preset]}
                <div className="absolute left-1/2 -translate-x-1/2 -top-1 w-2 h-2 bg-gray-900 rotate-45" />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

const TaskNode = memo(TaskNodeInner);
export default TaskNode;
