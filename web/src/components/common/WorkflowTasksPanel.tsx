import { useState, useMemo } from "react";
import { Link } from "react-router-dom";
import { formatDistanceToNow } from "date-fns";
import {
  ChevronDown,
  ChevronRight,
  Workflow,
  CheckCircle2,
  XCircle,
  Clock,
  Loader2,
  AlertTriangle,
  Ban,
  CircleDot,
  RotateCcw,
} from "lucide-react";
import { useChildExecutions } from "@/hooks/useExecutions";

interface WorkflowTasksPanelProps {
  /** The parent (workflow) execution ID */
  parentExecutionId: number;
  /** Whether the panel starts collapsed (default: false — open by default for workflows) */
  defaultCollapsed?: boolean;
}

/** Format a duration in ms to a human-readable string. */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(1)}s`;
  const mins = Math.floor(secs / 60);
  const remainSecs = Math.round(secs % 60);
  if (mins < 60) return `${mins}m ${remainSecs}s`;
  const hrs = Math.floor(mins / 60);
  const remainMins = mins % 60;
  return `${hrs}h ${remainMins}m`;
}

function getStatusIcon(status: string) {
  switch (status) {
    case "completed":
      return <CheckCircle2 className="h-4 w-4 text-green-500" />;
    case "failed":
      return <XCircle className="h-4 w-4 text-red-500" />;
    case "running":
      return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
    case "requested":
    case "scheduling":
    case "scheduled":
      return <Clock className="h-4 w-4 text-yellow-500" />;
    case "timeout":
      return <AlertTriangle className="h-4 w-4 text-orange-500" />;
    case "canceling":
    case "cancelled":
      return <Ban className="h-4 w-4 text-gray-400" />;
    case "abandoned":
      return <AlertTriangle className="h-4 w-4 text-red-400" />;
    default:
      return <CircleDot className="h-4 w-4 text-gray-400" />;
  }
}

function getStatusBadgeClasses(status: string): string {
  switch (status) {
    case "completed":
      return "bg-green-100 text-green-800";
    case "failed":
      return "bg-red-100 text-red-800";
    case "running":
      return "bg-blue-100 text-blue-800";
    case "requested":
    case "scheduling":
    case "scheduled":
      return "bg-yellow-100 text-yellow-800";
    case "timeout":
      return "bg-orange-100 text-orange-800";
    case "canceling":
    case "cancelled":
      return "bg-gray-100 text-gray-800";
    case "abandoned":
      return "bg-red-100 text-red-600";
    default:
      return "bg-gray-100 text-gray-800";
  }
}

/**
 * Panel that displays workflow task (child) executions for a parent
 * workflow execution. Shows each task's name, action, status, and timing.
 */
export default function WorkflowTasksPanel({
  parentExecutionId,
  defaultCollapsed = false,
}: WorkflowTasksPanelProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);
  const { data, isLoading, error } = useChildExecutions(parentExecutionId);

  const tasks = useMemo(() => {
    if (!data?.data) return [];
    return data.data;
  }, [data]);

  const summary = useMemo(() => {
    const total = tasks.length;
    const completed = tasks.filter((t) => t.status === "completed").length;
    const failed = tasks.filter((t) => t.status === "failed").length;
    const running = tasks.filter(
      (t) =>
        t.status === "running" ||
        t.status === "requested" ||
        t.status === "scheduling" ||
        t.status === "scheduled",
    ).length;
    const other = total - completed - failed - running;
    return { total, completed, failed, running, other };
  }, [tasks]);

  if (!isLoading && tasks.length === 0 && !error) {
    // No child tasks — nothing to show
    return null;
  }

  return (
    <div className="bg-white shadow rounded-lg">
      {/* Header */}
      <button
        onClick={() => setIsCollapsed(!isCollapsed)}
        className="w-full flex items-center justify-between p-6 text-left hover:bg-gray-50 rounded-lg transition-colors"
      >
        <div className="flex items-center gap-3">
          {isCollapsed ? (
            <ChevronRight className="h-5 w-5 text-gray-400" />
          ) : (
            <ChevronDown className="h-5 w-5 text-gray-400" />
          )}
          <Workflow className="h-5 w-5 text-indigo-500" />
          <h2 className="text-xl font-semibold">Workflow Tasks</h2>
          {!isLoading && (
            <span className="text-sm text-gray-500">
              ({summary.total} task{summary.total !== 1 ? "s" : ""})
            </span>
          )}
        </div>

        {/* Summary badges */}
        {!isCollapsed || !isLoading ? (
          <div className="flex items-center gap-2">
            {summary.completed > 0 && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                <CheckCircle2 className="h-3 w-3" />
                {summary.completed}
              </span>
            )}
            {summary.running > 0 && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
                <Loader2 className="h-3 w-3 animate-spin" />
                {summary.running}
              </span>
            )}
            {summary.failed > 0 && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">
                <XCircle className="h-3 w-3" />
                {summary.failed}
              </span>
            )}
            {summary.other > 0 && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700">
                {summary.other}
              </span>
            )}
          </div>
        ) : null}
      </button>

      {/* Content */}
      {!isCollapsed && (
        <div className="px-6 pb-6">
          {isLoading && (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-5 w-5 animate-spin text-gray-400" />
              <span className="ml-2 text-sm text-gray-500">
                Loading workflow tasks…
              </span>
            </div>
          )}

          {error && (
            <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded text-sm">
              Error loading workflow tasks:{" "}
              {error instanceof Error ? error.message : "Unknown error"}
            </div>
          )}

          {!isLoading && !error && tasks.length > 0 && (
            <div className="space-y-2">
              {/* Column headers */}
              <div className="grid grid-cols-12 gap-3 px-3 py-2 text-xs font-medium text-gray-500 uppercase tracking-wider border-b border-gray-100">
                <div className="col-span-1">#</div>
                <div className="col-span-3">Task</div>
                <div className="col-span-3">Action</div>
                <div className="col-span-2">Status</div>
                <div className="col-span-2">Duration</div>
                <div className="col-span-1">Retry</div>
              </div>

              {/* Task rows */}
              {tasks.map((task, idx) => {
                const wt = task.workflow_task;
                const taskName = wt?.task_name ?? `Task ${idx + 1}`;
                const retryCount = wt?.retry_count ?? 0;
                const maxRetries = wt?.max_retries ?? 0;
                const timedOut = wt?.timed_out ?? false;

                // Compute duration from created → updated (best available)
                const created = new Date(task.created);
                const updated = new Date(task.updated);
                const durationMs =
                  wt?.duration_ms ??
                  (task.status === "completed" ||
                  task.status === "failed" ||
                  task.status === "timeout"
                    ? updated.getTime() - created.getTime()
                    : null);

                return (
                  <Link
                    key={task.id}
                    to={`/executions/${task.id}`}
                    className="grid grid-cols-12 gap-3 px-3 py-3 rounded-lg hover:bg-gray-50 transition-colors items-center group"
                  >
                    {/* Index */}
                    <div className="col-span-1 text-sm text-gray-400 font-mono">
                      {idx + 1}
                    </div>

                    {/* Task name */}
                    <div className="col-span-3 flex items-center gap-2 min-w-0">
                      {getStatusIcon(task.status)}
                      <span
                        className="text-sm font-medium text-gray-900 truncate group-hover:text-blue-600"
                        title={taskName}
                      >
                        {taskName}
                      </span>
                      {wt?.task_index != null && (
                        <span className="text-xs text-gray-400 flex-shrink-0">
                          [{wt.task_index}]
                        </span>
                      )}
                    </div>

                    {/* Action ref */}
                    <div className="col-span-3 min-w-0">
                      <span
                        className="text-sm text-gray-600 truncate block"
                        title={task.action_ref}
                      >
                        {task.action_ref}
                      </span>
                    </div>

                    {/* Status badge */}
                    <div className="col-span-2 flex items-center gap-1.5">
                      <span
                        className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${getStatusBadgeClasses(task.status)}`}
                      >
                        {task.status}
                      </span>
                      {timedOut && (
                        <span title="Timed out">
                          <AlertTriangle className="h-3.5 w-3.5 text-orange-500" />
                        </span>
                      )}
                    </div>

                    {/* Duration */}
                    <div className="col-span-2 text-sm text-gray-500">
                      {task.status === "running" ? (
                        <span className="text-blue-600">
                          {formatDistanceToNow(created, { addSuffix: false })}…
                        </span>
                      ) : durationMs != null && durationMs > 0 ? (
                        formatDuration(durationMs)
                      ) : (
                        <span className="text-gray-300">—</span>
                      )}
                    </div>

                    {/* Retry info */}
                    <div className="col-span-1 text-sm text-gray-500">
                      {maxRetries > 0 ? (
                        <span
                          className="inline-flex items-center gap-0.5"
                          title={`Attempt ${retryCount + 1} of ${maxRetries + 1}`}
                        >
                          <RotateCcw className="h-3 w-3" />
                          {retryCount}/{maxRetries}
                        </span>
                      ) : (
                        <span className="text-gray-300">—</span>
                      )}
                    </div>
                  </Link>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
