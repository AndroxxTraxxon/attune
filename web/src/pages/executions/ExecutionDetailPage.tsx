import { useParams, Link } from "react-router-dom";
import { useExecution } from "@/hooks/useExecutions";
import { useExecutionStream } from "@/hooks/useExecutionStream";
import { formatDistanceToNow } from "date-fns";
import { ExecutionStatus } from "@/api";

const getStatusColor = (status: string) => {
  switch (status) {
    case "succeeded":
      return "bg-green-100 text-green-800";
    case "failed":
      return "bg-red-100 text-red-800";
    case "running":
      return "bg-blue-100 text-blue-800";
    case "pending":
    case "scheduled":
      return "bg-yellow-100 text-yellow-800";
    case "timeout":
      return "bg-orange-100 text-orange-800";
    case "canceled":
      return "bg-gray-100 text-gray-800";
    case "paused":
      return "bg-purple-100 text-purple-800";
    default:
      return "bg-gray-100 text-gray-800";
  }
};

export default function ExecutionDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data: executionData, isLoading, error } = useExecution(Number(id));
  const execution = executionData?.data;

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
        </div>
        <p className="text-gray-600 mt-2">
          <Link
            to={`/actions/${execution.action_ref}`}
            className="text-blue-600 hover:text-blue-800"
          >
            {execution.action_ref}
          </Link>
        </p>
      </div>

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
            <div className="space-y-4">
              <div className="flex gap-4">
                <div className="flex flex-col items-center">
                  <div className="w-3 h-3 rounded-full bg-blue-500" />
                  {!isRunning && <div className="w-0.5 h-full bg-gray-300" />}
                </div>
                <div className="flex-1 pb-4">
                  <p className="font-medium">Execution Created</p>
                  <p className="text-sm text-gray-500">
                    {new Date(execution.created).toLocaleString()}
                  </p>
                </div>
              </div>

              {execution.status === ExecutionStatus.COMPLETED && (
                <div className="flex gap-4">
                  <div className="flex flex-col items-center">
                    <div className="w-3 h-3 rounded-full bg-green-500" />
                  </div>
                  <div className="flex-1">
                    <p className="font-medium">Execution Completed</p>
                    <p className="text-sm text-gray-500">
                      {new Date(execution.updated).toLocaleString()}
                    </p>
                  </div>
                </div>
              )}

              {execution.status === ExecutionStatus.FAILED && (
                <div className="flex gap-4">
                  <div className="flex flex-col items-center">
                    <div className="w-3 h-3 rounded-full bg-red-500" />
                  </div>
                  <div className="flex-1">
                    <p className="font-medium">Execution Failed</p>
                    <p className="text-sm text-gray-500">
                      {new Date(execution.updated).toLocaleString()}
                    </p>
                  </div>
                </div>
              )}

              {isRunning && (
                <div className="flex gap-4">
                  <div className="flex flex-col items-center">
                    <div className="w-3 h-3 rounded-full bg-blue-500 animate-pulse" />
                  </div>
                  <div className="flex-1">
                    <p className="font-medium text-blue-600">In Progress...</p>
                  </div>
                </div>
              )}
            </div>
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
    </div>
  );
}
