import { useState, useMemo, memo } from "react";
import { Link } from "react-router-dom";
import {
  ChevronRight,
  ChevronDown,
  Workflow,
  Loader2,
  CheckCircle2,
  XCircle,
  Clock,
  AlertTriangle,
  Ban,
  CircleDot,
  RotateCcw,
} from "lucide-react";
import { useChildExecutions } from "@/hooks/useExecutions";
import type { ExecutionSummary } from "@/api";

// ─── Helpers ────────────────────────────────────────────────────────────────

function getStatusColor(status: string) {
  switch (status) {
    case "completed":
      return "bg-green-100 text-green-800";
    case "failed":
    case "timeout":
      return "bg-red-100 text-red-800";
    case "running":
      return "bg-blue-100 text-blue-800";
    case "requested":
    case "scheduling":
    case "scheduled":
      return "bg-yellow-100 text-yellow-800";
    case "canceling":
    case "cancelled":
      return "bg-gray-100 text-gray-600";
    default:
      return "bg-gray-100 text-gray-800";
  }
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

// ─── Child execution row (recursive) ────────────────────────────────────────

interface ChildExecutionRowProps {
  execution: ExecutionSummary;
  depth: number;
  selectedExecutionId: number | null;
  onSelectExecution: (id: number) => void;
  workflowActionRefs: Set<string>;
}

/**
 * A single child-execution row inside the accordion. If it has its own
 * children (nested workflow), it can be expanded recursively.
 */
const ChildExecutionRow = memo(function ChildExecutionRow({
  execution,
  depth,
  selectedExecutionId,
  onSelectExecution,
  workflowActionRefs,
}: ChildExecutionRowProps) {
  const isWorkflow = workflowActionRefs.has(execution.action_ref);
  const [expanded, setExpanded] = useState(false);

  // Only fetch children when expanded and this is a workflow action
  const { data, isLoading } = useChildExecutions(
    expanded && isWorkflow ? execution.id : undefined,
  );

  const children = useMemo(() => data?.data ?? [], [data]);
  const hasChildren = expanded && children.length > 0;

  const wt = execution.workflow_task;
  const taskName = wt?.task_name;
  const retryCount = wt?.retry_count ?? 0;
  const maxRetries = wt?.max_retries ?? 0;

  const created = new Date(execution.created);
  const updated = new Date(execution.updated);
  const durationMs =
    wt?.duration_ms ??
    (execution.status === "completed" ||
    execution.status === "failed" ||
    execution.status === "timeout"
      ? updated.getTime() - created.getTime()
      : null);

  const indent = 16 + depth * 24;

  return (
    <>
      <tr
        data-execution-id={execution.id}
        className={`hover:bg-gray-50/80 group border-t border-gray-100 cursor-pointer ${
          selectedExecutionId === execution.id
            ? "bg-blue-50 hover:bg-blue-50"
            : ""
        }`}
        onClick={() => onSelectExecution(execution.id)}
      >
        {/* Task name / expand toggle */}
        <td className="py-3 pr-2" style={{ paddingLeft: indent }}>
          <div className="flex items-center gap-1.5 min-w-0">
            {isWorkflow ? (
              <button
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setExpanded((prev) => !prev);
                }}
                className="flex-shrink-0 p-0.5 rounded hover:bg-gray-200 transition-colors"
                title={expanded ? "Collapse" : "Expand"}
              >
                {isLoading ? (
                  <Loader2 className="h-3.5 w-3.5 text-gray-400 animate-spin" />
                ) : expanded ? (
                  <ChevronDown className="h-3.5 w-3.5 text-gray-400" />
                ) : (
                  <ChevronRight className="h-3.5 w-3.5 text-gray-400" />
                )}
              </button>
            ) : (
              <span className="flex-shrink-0 w-[18px]" />
            )}

            {getStatusIcon(execution.status)}

            {taskName && (
              <span
                className="text-sm font-medium text-gray-700 truncate"
                title={taskName}
              >
                {taskName}
              </span>
            )}

            {wt?.task_index != null && (
              <span className="text-xs text-gray-400 flex-shrink-0">
                [{wt.task_index}]
              </span>
            )}
          </div>
        </td>

        {/* Exec ID */}
        <td className="px-4 py-3 font-mono text-xs">
          <Link
            to={`/executions/${execution.id}`}
            className="text-blue-600 hover:text-blue-800"
            onClick={(e) => e.stopPropagation()}
          >
            #{execution.id}
          </Link>
        </td>

        {/* Action */}
        <td className="px-4 py-3">
          <Link
            to={`/executions/${execution.id}`}
            className="text-sm text-blue-600 hover:text-blue-800 hover:underline truncate block"
            title={execution.action_ref}
            onClick={(e) => e.stopPropagation()}
          >
            {execution.action_ref}
          </Link>
        </td>

        {/* Status */}
        <td className="px-4 py-3">
          <span
            className={`px-2 py-0.5 text-xs rounded-full font-medium ${getStatusColor(execution.status)}`}
          >
            {execution.status}
          </span>
        </td>

        {/* Duration */}
        <td className="px-4 py-3 text-sm text-gray-500">
          {execution.status === "running" ? (
            <span className="text-blue-600 flex items-center gap-1">
              <Loader2 className="h-3 w-3 animate-spin" />
              running
            </span>
          ) : durationMs != null && durationMs > 0 ? (
            formatDuration(durationMs)
          ) : (
            <span className="text-gray-300">&mdash;</span>
          )}
        </td>

        {/* Retry */}
        <td className="px-4 py-3 text-sm text-gray-500">
          {maxRetries > 0 ? (
            <span
              className="inline-flex items-center gap-0.5"
              title={`Attempt ${retryCount + 1} of ${maxRetries + 1}`}
            >
              <RotateCcw className="h-3 w-3" />
              {retryCount}/{maxRetries}
            </span>
          ) : (
            <span className="text-gray-300">&mdash;</span>
          )}
        </td>
      </tr>

      {/* Nested children */}
      {expanded &&
        !isLoading &&
        hasChildren &&
        children.map((child: ExecutionSummary) => (
          <ChildExecutionRow
            key={child.id}
            execution={child}
            depth={depth + 1}
            selectedExecutionId={selectedExecutionId}
            onSelectExecution={onSelectExecution}
            workflowActionRefs={workflowActionRefs}
          />
        ))}
    </>
  );
});

// ─── Top-level workflow row (accordion) ─────────────────────────────────────

interface WorkflowExecutionRowProps {
  execution: ExecutionSummary;
  workflowActionRefs: Set<string>;
  selectedExecutionId: number | null;
  onSelectExecution: (id: number) => void;
}

/**
 * A top-level execution row with an expandable accordion for child tasks.
 */
const WorkflowExecutionRow = memo(function WorkflowExecutionRow({
  execution,
  workflowActionRefs,
  selectedExecutionId,
  onSelectExecution,
}: WorkflowExecutionRowProps) {
  const isWorkflow = workflowActionRefs.has(execution.action_ref);
  const [expanded, setExpanded] = useState(false);

  const { data, isLoading } = useChildExecutions(
    expanded && isWorkflow ? execution.id : undefined,
  );

  const children = useMemo(() => data?.data ?? [], [data]);

  const summary = useMemo(() => {
    const total = children.length;
    const completed = children.filter(
      (t: ExecutionSummary) => t.status === "completed",
    ).length;
    const failed = children.filter(
      (t: ExecutionSummary) => t.status === "failed" || t.status === "timeout",
    ).length;
    const running = children.filter(
      (t: ExecutionSummary) =>
        t.status === "running" ||
        t.status === "requested" ||
        t.status === "scheduling" ||
        t.status === "scheduled",
    ).length;
    return { total, completed, failed, running };
  }, [children]);

  const hasWorkflowChildren = expanded && children.length > 0;

  return (
    <>
      {/* Main execution row */}
      <tr
        data-execution-id={execution.id}
        className={`hover:bg-gray-50 border-b border-gray-200 cursor-pointer ${
          selectedExecutionId === execution.id
            ? "bg-blue-50 hover:bg-blue-50"
            : ""
        }`}
        onClick={() => onSelectExecution(execution.id)}
      >
        <td className="px-6 py-4">
          <div className="flex items-center gap-2">
            {isWorkflow ? (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setExpanded((prev) => !prev);
                }}
                className="flex-shrink-0 p-0.5 rounded hover:bg-gray-200 transition-colors"
                title={
                  expanded ? "Collapse workflow tasks" : "Expand workflow tasks"
                }
              >
                {isLoading ? (
                  <Loader2 className="h-4 w-4 text-gray-400 animate-spin" />
                ) : expanded ? (
                  <ChevronDown className="h-4 w-4 text-gray-500" />
                ) : (
                  <ChevronRight className="h-4 w-4 text-gray-500" />
                )}
              </button>
            ) : (
              <span className="flex-shrink-0 w-[20px]" />
            )}
            <Link
              to={`/executions/${execution.id}`}
              className="text-blue-600 hover:text-blue-800 font-mono text-sm"
              onClick={(e) => e.stopPropagation()}
            >
              #{execution.id}
            </Link>
          </div>
        </td>
        <td className="px-6 py-4">
          <span className="text-sm text-gray-900">{execution.action_ref}</span>
        </td>
        <td className="px-6 py-4">
          {execution.rule_ref ? (
            <span className="text-sm text-gray-700">{execution.rule_ref}</span>
          ) : (
            <span className="text-sm text-gray-400 italic">-</span>
          )}
        </td>
        <td className="px-6 py-4">
          {execution.trigger_ref ? (
            <span className="text-sm text-gray-700">
              {execution.trigger_ref}
            </span>
          ) : (
            <span className="text-sm text-gray-400 italic">-</span>
          )}
        </td>
        <td className="px-6 py-4">
          <span
            className={`px-2 py-1 text-xs rounded ${getStatusColor(execution.status)}`}
          >
            {execution.status}
          </span>
        </td>
        <td className="px-6 py-4 text-sm text-gray-500">
          {new Date(execution.created).toLocaleString()}
        </td>
      </tr>

      {/* Expanded child-task section */}
      {expanded && (
        <tr>
          <td colSpan={6} className="p-0">
            <div className="bg-gray-50 border-b border-gray-200">
              {/* Summary bar */}
              {hasWorkflowChildren && (
                <div className="flex items-center gap-3 px-8 py-2 border-b border-gray-200 bg-gray-100/60">
                  <Workflow className="h-4 w-4 text-indigo-500" />
                  <span className="text-xs font-medium text-gray-600">
                    {summary.total} task{summary.total !== 1 ? "s" : ""}
                  </span>
                  {summary.completed > 0 && (
                    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700">
                      <CheckCircle2 className="h-3 w-3" />
                      {summary.completed}
                    </span>
                  )}
                  {summary.running > 0 && (
                    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-700">
                      <Loader2 className="h-3 w-3 animate-spin" />
                      {summary.running}
                    </span>
                  )}
                  {summary.failed > 0 && (
                    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700">
                      <XCircle className="h-3 w-3" />
                      {summary.failed}
                    </span>
                  )}
                </div>
              )}

              {/* Loading state */}
              {isLoading && (
                <div className="flex items-center gap-2 px-8 py-4">
                  <Loader2 className="h-4 w-4 animate-spin text-gray-400" />
                  <span className="text-sm text-gray-500">
                    Loading workflow tasks...
                  </span>
                </div>
              )}

              {/* No children yet (workflow still starting) */}
              {!isLoading && children.length === 0 && (
                <div className="px-8 py-3 text-sm text-gray-400 italic">
                  No child tasks yet.
                </div>
              )}

              {/* Children table */}
              {hasWorkflowChildren && (
                <table className="w-full">
                  <thead>
                    <tr className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      <th
                        className="py-2 pr-2 text-left"
                        style={{ paddingLeft: 40 }}
                      >
                        Task
                      </th>
                      <th className="px-4 py-2 text-left">ID</th>
                      <th className="px-4 py-2 text-left">Action</th>
                      <th className="px-4 py-2 text-left">Status</th>
                      <th className="px-4 py-2 text-left">Duration</th>
                      <th className="px-4 py-2 text-left">Retry</th>
                    </tr>
                  </thead>
                  <tbody>
                    {children.map((child: ExecutionSummary) => (
                      <ChildExecutionRow
                        key={child.id}
                        execution={child}
                        depth={0}
                        selectedExecutionId={selectedExecutionId}
                        onSelectExecution={onSelectExecution}
                        workflowActionRefs={workflowActionRefs}
                      />
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  );
});

// ─── Main tree table ────────────────────────────────────────────────────────

interface WorkflowExecutionTreeProps {
  executions: ExecutionSummary[];
  isLoading: boolean;
  isFetching: boolean;
  error: Error | null;
  hasActiveFilters: boolean;
  clearFilters: () => void;
  workflowActionRefs: Set<string>;
  selectedExecutionId: number | null;
  onSelectExecution: (id: number) => void;
}

/**
 * Renders the executions list in "By Workflow" mode. Top-level executions
 * are shown with the same columns as the "All" view, but each row is
 * expandable to reveal the workflow's child task executions in an accordion.
 * Nested workflows can be drilled into recursively.
 */
const WorkflowExecutionTree = memo(function WorkflowExecutionTree({
  executions,
  isLoading,
  isFetching,
  error,
  hasActiveFilters,
  clearFilters,
  workflowActionRefs,
  selectedExecutionId,
  onSelectExecution,
}: WorkflowExecutionTreeProps) {
  // Initial load
  if (isLoading && executions.length === 0) {
    return (
      <div className="bg-white shadow rounded-lg">
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
        </div>
      </div>
    );
  }

  // Error with no cached data
  if (error && executions.length === 0) {
    return (
      <div className="bg-white shadow rounded-lg">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {error.message}</p>
        </div>
      </div>
    );
  }

  // Empty
  if (executions.length === 0) {
    return (
      <div className="bg-white p-12 text-center rounded-lg shadow">
        <p>No executions found</p>
        {hasActiveFilters && (
          <button
            onClick={clearFilters}
            className="mt-3 text-sm text-blue-600 hover:text-blue-800"
          >
            Clear filters
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="relative">
      {/* Loading overlay */}
      {isFetching && (
        <div className="absolute inset-0 bg-white/60 z-10 flex items-center justify-center rounded-lg">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
        </div>
      )}

      {/* Non-fatal error banner */}
      {error && (
        <div className="mb-4 bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error refreshing: {error.message}</p>
        </div>
      )}

      <div className="bg-white shadow rounded-lg overflow-hidden">
        <table className="min-w-full">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                ID
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Action
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Rule
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Trigger
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Status
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Created
              </th>
            </tr>
          </thead>
          <tbody className="bg-white">
            {executions.map((exec: ExecutionSummary) => (
              <WorkflowExecutionRow
                key={exec.id}
                execution={exec}
                workflowActionRefs={workflowActionRefs}
                selectedExecutionId={selectedExecutionId}
                onSelectExecution={onSelectExecution}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
});

WorkflowExecutionTree.displayName = "WorkflowExecutionTree";

export default WorkflowExecutionTree;
