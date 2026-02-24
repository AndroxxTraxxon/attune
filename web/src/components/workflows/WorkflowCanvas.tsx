import { useState, useCallback, useRef, useMemo } from "react";
import TaskNode from "./TaskNode";
import type { TransitionPreset } from "./TaskNode";
import WorkflowEdges from "./WorkflowEdges";
import type { EdgeHoverInfo } from "./WorkflowEdges";
import type {
  WorkflowTask,
  PaletteAction,
  WorkflowEdge,
} from "@/types/workflow";
import {
  deriveEdges,
  generateUniqueTaskName,
  generateTaskId,
  PRESET_LABELS,
} from "@/types/workflow";
import { Plus } from "lucide-react";

interface WorkflowCanvasProps {
  tasks: WorkflowTask[];
  selectedTaskId: string | null;
  availableActions: PaletteAction[];
  onSelectTask: (taskId: string | null) => void;
  onUpdateTask: (taskId: string, updates: Partial<WorkflowTask>) => void;
  onDeleteTask: (taskId: string) => void;
  onAddTask: (task: WorkflowTask) => void;
  onSetConnection: (
    fromTaskId: string,
    preset: TransitionPreset,
    toTaskName: string,
  ) => void;
  onEdgeHover?: (info: EdgeHoverInfo | null) => void;
}

/** Label color mapping for the connecting banner */
const PRESET_BANNER_COLORS: Record<TransitionPreset, string> = {
  succeeded: "text-green-200 font-bold",
  failed: "text-red-200 font-bold",
  always: "text-gray-200 font-bold",
};

export default function WorkflowCanvas({
  tasks,
  selectedTaskId,
  onSelectTask,
  onUpdateTask,
  onDeleteTask,
  onAddTask,
  onSetConnection,
  onEdgeHover,
}: WorkflowCanvasProps) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const [connectingFrom, setConnectingFrom] = useState<{
    taskId: string;
    preset: TransitionPreset;
  } | null>(null);
  const [mousePosition, setMousePosition] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const allTaskNames = useMemo(() => tasks.map((t) => t.name), [tasks]);

  const edges: WorkflowEdge[] = useMemo(() => deriveEdges(tasks), [tasks]);

  const handleCanvasClick = useCallback(
    (e: React.MouseEvent) => {
      // Only deselect if clicking the canvas background
      if (
        e.target === canvasRef.current ||
        (e.target as HTMLElement).dataset.canvasBg === "true"
      ) {
        if (connectingFrom) {
          setConnectingFrom(null);
          setMousePosition(null);
        } else {
          onSelectTask(null);
        }
      }
    },
    [onSelectTask, connectingFrom],
  );

  const handleCanvasMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (connectingFrom && canvasRef.current) {
        const rect = canvasRef.current.getBoundingClientRect();
        const scrollLeft = canvasRef.current.scrollLeft;
        const scrollTop = canvasRef.current.scrollTop;
        setMousePosition({
          x: e.clientX - rect.left + scrollLeft,
          y: e.clientY - rect.top + scrollTop,
        });
      }
    },
    [connectingFrom],
  );

  const handleCanvasMouseUp = useCallback(() => {
    // If we're connecting and mouseup happens on the canvas (not on a node),
    // cancel the connection
    if (connectingFrom) {
      setConnectingFrom(null);
      setMousePosition(null);
    }
  }, [connectingFrom]);

  const handlePositionChange = useCallback(
    (taskId: string, position: { x: number; y: number }) => {
      onUpdateTask(taskId, { position });
    },
    [onUpdateTask],
  );

  const handleStartConnection = useCallback(
    (taskId: string, preset: TransitionPreset) => {
      setConnectingFrom({ taskId, preset });
    },
    [],
  );

  const handleCompleteConnection = useCallback(
    (targetTaskId: string) => {
      if (!connectingFrom) return;

      const targetTask = tasks.find((t) => t.id === targetTaskId);
      if (!targetTask) return;

      onSetConnection(
        connectingFrom.taskId,
        connectingFrom.preset,
        targetTask.name,
      );
      setConnectingFrom(null);
      setMousePosition(null);
    },
    [connectingFrom, tasks, onSetConnection],
  );

  const handleAddEmptyTask = useCallback(() => {
    const name = generateUniqueTaskName(tasks);
    // Position new tasks below existing ones
    let maxY = 0;
    for (const task of tasks) {
      if (task.position.y > maxY) {
        maxY = task.position.y;
      }
    }
    const newTask: WorkflowTask = {
      id: generateTaskId(),
      name,
      action: "",
      input: {},
      position: {
        x: 300,
        y: tasks.length === 0 ? 60 : maxY + 160,
      },
    };
    onAddTask(newTask);
    onSelectTask(newTask.id);
  }, [tasks, onAddTask, onSelectTask]);

  // Calculate minimum canvas dimensions based on node positions
  const canvasDimensions = useMemo(() => {
    let maxX = 800;
    let maxY = 600;
    for (const task of tasks) {
      maxX = Math.max(maxX, task.position.x + 340);
      maxY = Math.max(maxY, task.position.y + 220);
    }
    return { width: maxX, height: maxY };
  }, [tasks]);

  return (
    <div
      className="flex-1 overflow-auto bg-gray-100 relative"
      ref={canvasRef}
      onClick={handleCanvasClick}
      onMouseMove={handleCanvasMouseMove}
      onMouseUp={handleCanvasMouseUp}
    >
      {/* Grid background */}
      <div
        data-canvas-bg="true"
        className="absolute inset-0"
        style={{
          minWidth: canvasDimensions.width,
          minHeight: canvasDimensions.height,
          backgroundImage: `
            linear-gradient(to right, rgba(0,0,0,0.03) 1px, transparent 1px),
            linear-gradient(to bottom, rgba(0,0,0,0.03) 1px, transparent 1px)
          `,
          backgroundSize: "20px 20px",
        }}
      />

      {/* Connecting mode indicator */}
      {connectingFrom && (
        <div className="sticky top-0 left-0 right-0 z-50 flex justify-center pointer-events-none">
          <div className="mt-3 px-4 py-2 bg-purple-600 text-white text-sm font-medium rounded-full shadow-lg pointer-events-auto">
            Drag to a task to connect as{" "}
            <span className={PRESET_BANNER_COLORS[connectingFrom.preset]}>
              {PRESET_LABELS[connectingFrom.preset]}
            </span>{" "}
            transition — or release to cancel
          </div>
        </div>
      )}

      {/* Edge rendering layer */}
      <WorkflowEdges
        edges={edges}
        tasks={tasks}
        connectingFrom={connectingFrom}
        mousePosition={mousePosition}
        onEdgeHover={onEdgeHover}
      />

      {/* Task nodes */}
      {tasks.map((task) => (
        <TaskNode
          key={task.id}
          task={task}
          isSelected={task.id === selectedTaskId}
          allTaskNames={allTaskNames}
          onSelect={onSelectTask}
          onDelete={onDeleteTask}
          onPositionChange={handlePositionChange}
          onStartConnection={handleStartConnection}
          connectingFrom={connectingFrom}
          onCompleteConnection={handleCompleteConnection}
        />
      ))}

      {/* Empty state / Add task button */}
      {tasks.length === 0 ? (
        <div
          className="absolute inset-0 flex items-center justify-center pointer-events-none"
          style={{
            minWidth: canvasDimensions.width,
            minHeight: canvasDimensions.height,
          }}
        >
          <div className="text-center pointer-events-auto">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-200 flex items-center justify-center">
              <Plus className="w-8 h-8 text-gray-400" />
            </div>
            <h3 className="text-lg font-medium text-gray-600 mb-2">
              Empty Workflow
            </h3>
            <p className="text-sm text-gray-400 mb-4 max-w-xs">
              Add tasks from the action palette on the left, or click the button
              below to add a blank task.
            </p>
            <button
              onClick={handleAddEmptyTask}
              className="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors shadow-sm"
            >
              <Plus className="w-4 h-4 inline-block mr-1.5 -mt-0.5" />
              Add First Task
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={handleAddEmptyTask}
          className="fixed bottom-6 right-6 z-40 w-12 h-12 bg-blue-600 text-white rounded-full shadow-lg hover:bg-blue-700 transition-colors flex items-center justify-center"
          title="Add a new task"
        >
          <Plus className="w-6 h-6" />
        </button>
      )}
    </div>
  );
}
