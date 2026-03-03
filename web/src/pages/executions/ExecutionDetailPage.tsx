import { useParams, Link } from "react-router-dom";

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
import { useExecution } from "@/hooks/useExecutions";
import { useAction } from "@/hooks/useActions";
import { useExecutionStream } from "@/hooks/useExecutionStream";
import { useExecutionHistory } from "@/hooks/useHistory";
import { formatDistanceToNow } from "date-fns";
import { ExecutionStatus } from "@/api";
import { useState, useMemo } from "react";
import { RotateCcw, Loader2 } from "lucide-react";
import ExecuteActionModal from "@/components/common/ExecuteActionModal";
import EntityHistoryPanel from "@/components/common/EntityHistoryPanel";
import WorkflowTasksPanel from "@/components/common/WorkflowTasksPanel";
import ExecutionArtifactsPanel from "@/components/executions/ExecutionArtifactsPanel";
import ExecutionProgressBar from "@/components/executions/ExecutionProgressBar";

const getStatusColor = (status: string) => {
  switch (status) {
    case "succeeded":
    case "completed":
      return "bg-green-100 text-green-800";
    case "failed":
      return "bg-red-100 text-red-800";
    case "running":
      return "bg-blue-100 text-blue-800";
    case "pending":
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
};

/** Map status to a dot color for the timeline. */
const getTimelineDotColor = (status: string) => {
  switch (status) {
    case "completed":
      return "bg-green-500";
    case "failed":
      return "bg-red-500";
    case "running":
      return "bg-blue-500";
    case "requested":
    case "scheduling":
    case "scheduled":
      return "bg-yellow-500";
    case "timeout":
      return "bg-orange-500";
    case "canceling":
    case "cancelled":
      return "bg-gray-400";
    case "abandoned":
      return "bg-red-400";
    default:
      return "bg-gray-400";
  }
};

/** Human-readable label for a status value. */
const getStatusLabel = (status: string) => {
  switch (status) {
    case "requested":
      return "Requested";
    case "scheduling":
      return "Scheduling";
    case "scheduled":
      return "Scheduled";
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "canceling":
      return "Canceling";
    case "cancelled":
      return "Cancelled";
    case "timeout":
      return "Timed Out";
    case "abandoned":
      return "Abandoned";
    default:
      return status.charAt(0).toUpperCase() + status.slice(1);
  }
};

interface TimelineEntry {
  status: string;
  time: string;
  isInitial: boolean;
}

export default function ExecutionDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data: executionData, isLoading, error } = useExecution(Number(id));
  const execution = executionData?.data;

  // Fetch the action so we can get param_schema for the re-run modal
  const { data: actionData } = useAction(execution?.action_ref || "");

  // Determine if this execution is a workflow (action has workflow_def)
  const isWorkflow = !!actionData?.data?.workflow_def;

  const [showRerunModal, setShowRerunModal] = useState(false);

  // Fetch status history for the timeline
  const { data: historyData, isLoading: historyLoading } = useExecutionHistory(
    Number(id),
    { page_size: 100 },
  );

  // Build timeline entries from history records
  const timelineEntries = useMemo<TimelineEntry[]>(() => {
    const records = historyData?.data ?? [];
    const entries: TimelineEntry[] = [];

    for (const record of records) {
      if (record.operation === "INSERT" && record.new_values?.status) {
        entries.push({
          status: String(record.new_values.status),
          time: record.time,
          isInitial: true,
        });
      } else if (
        record.operation === "UPDATE" &&
        record.changed_fields.includes("status") &&
        record.new_values?.status
      ) {
        entries.push({
          status: String(record.new_values.status),
          time: record.time,
          isInitial: false,
        });
      }
    }

    // History comes newest-first; reverse to chronological order
    entries.reverse();
    return entries;
  }, [historyData]);

  // Subscribe to real-time updates for this execution
  const { isConnected } = useExecutionStream({
    executionId: Number(id),
    enabled: !!id,
  });

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
        </div>
      </div>
    );
  }

  if (error || !execution) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>
            Error: {error ? (error as Error).message : "Execution not found"}
          </p>
        </div>
        <Link
          to="/executions"
          className="mt-4 inline-block text-blue-600 hover:text-blue-800"
        >
          ← Back to Executions
        </Link>
      </div>
    );
  }

  const isRunning =
    execution.status === ExecutionStatus.RUNNING ||
    execution.status === ExecutionStatus.SCHEDULING ||
    execution.status === ExecutionStatus.SCHEDULED ||
    execution.status === ExecutionStatus.REQUESTED;

  return (
    <div className="p-6 max-w-7xl mx-auto">
      {/* Header */}
      <div className="mb-6">
        <Link
          to="/executions"
          className="text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Executions
        </Link>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <h1 className="text-3xl font-bold">Execution #{execution.id}</h1>
            {isWorkflow && (
              <span className="px-3 py-1 text-sm rounded-full bg-indigo-100 text-indigo-800">
                Workflow
              </span>
            )}
            <span
              className={`px-3 py-1 text-sm rounded-full ${getStatusColor(execution.status)}`}
            >
              {execution.status}
            </span>
            {isRunning && (
              <div className="flex items-center gap-2 text-sm text-gray-600">
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-blue-600" />
                <span>In Progress</span>
              </div>
            )}
            {isConnected && (
              <div className="flex items-center gap-2 text-xs text-green-600">
                <div className="h-2 w-2 rounded-full bg-green-600 animate-pulse" />
                <span>Live</span>
              </div>
            )}
          </div>
          <button
            onClick={() => setShowRerunModal(true)}
            disabled={!actionData?.data}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            title={
              !actionData?.data
                ? "Loading action details..."
                : "Re-run this action with the same parameters"
            }
          >
            <RotateCcw className="h-4 w-4" />
            Re-Run
          </button>
        </div>
        <p className="text-gray-600 mt-2">
          <Link
            to={`/actions/${execution.action_ref}`}
            className="text-blue-600 hover:text-blue-800"
          >
            {execution.action_ref}
          </Link>
        </p>
        {execution.workflow_task && (
          <p className="text-sm text-indigo-600 mt-1 flex items-center gap-1.5">
            <span className="text-gray-500">Task</span>{" "}
            <span className="font-medium">
              {execution.workflow_task.task_name}
            </span>
            {execution.parent && (
              <>
                <span className="text-gray-500">in workflow</span>
                <Link
                  to={`/executions/${execution.parent}`}
                  className="text-indigo-600 hover:text-indigo-800 font-medium"
                >
                  Execution #{execution.parent}
                </Link>
              </>
            )}
          </p>
        )}
      </div>

      {/* Re-Run Modal */}
      {showRerunModal && actionData?.data && (
        <ExecuteActionModal
          action={actionData.data}
          onClose={() => setShowRerunModal(false)}
          initialParameters={execution.config}
        />
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Status & Timing */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Execution Details</h2>
            <dl className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <dt className="text-sm font-medium text-gray-500">Status</dt>
                <dd className="mt-1">
                  <span
                    className={`px-2 py-1 text-xs rounded ${getStatusColor(execution.status)}`}
                  >
                    {execution.status}
                  </span>
                </dd>
              </div>

              <div>
                <dt className="text-sm font-medium text-gray-500">Created</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(execution.created).toLocaleString()}
                  <span className="text-gray-500 ml-2 text-xs">
                    (
                    {formatDistanceToNow(new Date(execution.created), {
                      addSuffix: true,
                    })}
                    )
                  </span>
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Updated</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(execution.updated).toLocaleString()}
                </dd>
              </div>
              {execution.enforcement && (
                <div>
                  <dt className="text-sm font-medium text-gray-500">
                    Enforcement ID
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900">
                    {execution.enforcement}
                  </dd>
                </div>
              )}
              {execution.parent && (
                <div>
                  <dt className="text-sm font-medium text-gray-500">
                    Parent Execution
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900">
                    <Link
                      to={`/executions/${execution.parent}`}
                      className="text-blue-600 hover:text-blue-800"
                    >
                      #{execution.parent}
                    </Link>
                  </dd>
                </div>
              )}
              {execution.executor && (
                <div>
                  <dt className="text-sm font-medium text-gray-500">
                    Executor ID
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900">
                    {execution.executor}
                  </dd>
                </div>
              )}
            </dl>

            {/* Inline progress bar (visible when execution has progress artifacts) */}
            {isRunning && (
              <ExecutionProgressBar
                executionId={execution.id}
                isRunning={isRunning}
              />
            )}
          </div>

          {/* Config/Parameters */}
          {execution.config && Object.keys(execution.config).length > 0 && (
            <div className="bg-white shadow rounded-lg p-6">
              <h2 className="text-xl font-semibold mb-4">Configuration</h2>
              <pre className="bg-gray-50 p-4 rounded text-sm overflow-x-auto">
                {JSON.stringify(execution.config, null, 2)}
              </pre>
            </div>
          )}

          {/* Result */}
          {execution.result && Object.keys(execution.result).length > 0 && (
            <div className="bg-white shadow rounded-lg p-6">
              <h2 className="text-xl font-semibold mb-4">Result</h2>
              <pre className="bg-gray-50 p-4 rounded text-sm overflow-x-auto">
                {JSON.stringify(execution.result, null, 2)}
              </pre>
            </div>
          )}

          {/* Timeline */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Timeline</h2>

            {historyLoading && (
              <div className="flex items-center justify-center py-6">
                <Loader2 className="h-5 w-5 animate-spin text-gray-400" />
                <span className="ml-2 text-sm text-gray-500">
                  Loading timeline…
                </span>
              </div>
            )}

            {!historyLoading && timelineEntries.length === 0 && (
              /* Fallback: no history data yet — show basic created/current status */
              <div className="space-y-4">
                <div className="flex gap-4">
                  <div className="flex flex-col items-center">
                    <div
                      className={`w-3 h-3 rounded-full ${getTimelineDotColor(execution.status)}`}
                    />
                  </div>
                  <div className="flex-1">
                    <p className="font-medium">
                      {getStatusLabel(execution.status)}
                    </p>
                    <p className="text-sm text-gray-500">
                      {new Date(execution.created).toLocaleString()}
                    </p>
                  </div>
                </div>
              </div>
            )}

            {!historyLoading && timelineEntries.length > 0 && (
              <div className="space-y-0">
                {timelineEntries.map((entry, idx) => {
                  const isLast = idx === timelineEntries.length - 1;
                  const time = new Date(entry.time);
                  const prevTime =
                    idx > 0 ? new Date(timelineEntries[idx - 1].time) : null;
                  const durationMs = prevTime
                    ? time.getTime() - prevTime.getTime()
                    : null;

                  return (
                    <div key={`${entry.status}-${idx}`} className="flex gap-4">
                      <div className="flex flex-col items-center">
                        <div
                          className={`w-3 h-3 rounded-full flex-shrink-0 ${getTimelineDotColor(entry.status)}${
                            isLast && isRunning ? " animate-pulse" : ""
                          }`}
                        />
                        {!isLast && (
                          <div className="w-0.5 flex-1 min-h-[24px] bg-gray-200" />
                        )}
                      </div>
                      <div className={`flex-1 ${!isLast ? "pb-4" : ""}`}>
                        <div className="flex items-center gap-2">
                          <p className="font-medium">
                            {getStatusLabel(entry.status)}
                          </p>
                          <span
                            className={`px-1.5 py-0.5 text-[10px] font-medium rounded ${getStatusColor(entry.status)}`}
                          >
                            {entry.status}
                          </span>
                        </div>
                        <p className="text-sm text-gray-500">
                          {time.toLocaleString()}
                          <span className="text-gray-400 ml-2 text-xs">
                            ({formatDistanceToNow(time, { addSuffix: true })})
                          </span>
                        </p>
                        {durationMs !== null && durationMs > 0 && (
                          <p className="text-xs text-gray-400 mt-0.5">
                            +{formatDuration(durationMs)} since previous
                          </p>
                        )}
                      </div>
                    </div>
                  );
                })}

                {isRunning && (
                  <div className="flex gap-4 pt-4">
                    <div className="flex flex-col items-center">
                      <div className="w-3 h-3 rounded-full bg-blue-500 animate-pulse" />
                    </div>
                    <div className="flex-1">
                      <p className="font-medium text-blue-600">In Progress…</p>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Quick Info */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-lg font-semibold mb-4">Quick Info</h2>
            <div className="space-y-3">
              <div>
                <p className="text-sm text-gray-600">Action</p>
                <Link
                  to={`/actions/${execution.action_ref}`}
                  className="text-sm font-medium text-blue-600 hover:text-blue-800"
                >
                  {execution.action_ref}
                </Link>
              </div>
              {execution.enforcement && (
                <div>
                  <p className="text-sm text-gray-600">Enforcement ID</p>
                  <p className="text-sm font-medium">{execution.enforcement}</p>
                </div>
              )}
            </div>
          </div>

          {/* Quick Actions */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-lg font-semibold mb-4">Quick Actions</h2>
            <div className="space-y-2">
              <button
                onClick={() => setShowRerunModal(true)}
                disabled={!actionData?.data}
                className="block w-full px-4 py-2 text-sm text-center bg-blue-50 hover:bg-blue-100 text-blue-700 rounded disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Re-Run with Same Parameters
              </button>
              <Link
                to={`/actions/${execution.action_ref}`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View Action
              </Link>
              <Link
                to={`/executions?action_ref=${execution.action_ref}`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View All Executions
              </Link>
            </div>
          </div>
        </div>
      </div>

      {/* Workflow Tasks (shown only for workflow executions) */}
      {isWorkflow && (
        <div className="mt-6">
          <WorkflowTasksPanel parentExecutionId={execution.id} />
        </div>
      )}

      {/* Artifacts */}
      <div className="mt-6">
        <ExecutionArtifactsPanel
          executionId={execution.id}
          isRunning={isRunning}
        />
      </div>

      {/* Change History */}
      <div className="mt-6">
        <EntityHistoryPanel
          entityType="execution"
          entityId={execution.id}
          title="Execution History"
        />
      </div>
    </div>
  );
}
