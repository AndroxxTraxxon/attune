import { useAuth } from "@/contexts/AuthContext";
import { usePacks } from "@/hooks/usePacks";
import { useActions } from "@/hooks/useActions";
import { useRules } from "@/hooks/useRules";
import { useExecutions } from "@/hooks/useExecutions";
import { useExecutionStream } from "@/hooks/useExecutionStream";
import { useDashboardAnalytics } from "@/hooks/useAnalytics";
import { Link } from "react-router-dom";
import { ExecutionStatus } from "@/api";
import { useMemo, useState } from "react";
import AnalyticsDashboard from "@/components/common/AnalyticsWidgets";
import type { TimeRangeHours } from "@/components/common/AnalyticsWidgets";

export default function DashboardPage() {
  const { user } = useAuth();

  // Fetch metrics data
  const { data: packsData, isLoading: packsLoading } = usePacks({
    page: 1,
    pageSize: 1,
  });
  const { data: actionsData, isLoading: actionsLoading } = useActions({
    page: 1,
    pageSize: 1,
  });
  const { data: rulesData, isLoading: rulesLoading } = useRules({
    page: 1,
    pageSize: 1,
    enabled: true,
  });
  const { data: executionsData, isLoading: executionsLoading } = useExecutions({
    page: 1,
    pageSize: 20,
  });
  const { data: runningExecutions } = useExecutions({
    page: 1,
    pageSize: 1,
    status: ExecutionStatus.RUNNING,
  });

  // Subscribe to real-time execution updates
  // The hook automatically invalidates queries when updates arrive
  const { isConnected } = useExecutionStream();

  // Analytics time range state and data
  const [analyticsHours, setAnalyticsHours] = useState<TimeRangeHours>(24);
  const {
    data: analyticsData,
    isLoading: analyticsLoading,
    error: analyticsError,
  } = useDashboardAnalytics({ hours: analyticsHours });

  // Calculate metrics
  const totalPacks = packsData?.pagination?.total_items || 0;
  const totalActions = actionsData?.pagination?.total_items || 0;
  const activeRules = rulesData?.pagination?.total_items || 0;
  const runningCount = runningExecutions?.pagination?.total_items || 0;

  // Calculate status distribution
  const statusDistribution = useMemo(() => {
    if (!executionsData?.data) return {};

    const distribution: Record<ExecutionStatus, number> = {
      [ExecutionStatus.REQUESTED]: 0,
      [ExecutionStatus.SCHEDULING]: 0,
      [ExecutionStatus.SCHEDULED]: 0,
      [ExecutionStatus.RUNNING]: 0,
      [ExecutionStatus.COMPLETED]: 0,
      [ExecutionStatus.FAILED]: 0,
      [ExecutionStatus.CANCELING]: 0,
      [ExecutionStatus.CANCELLED]: 0,
      [ExecutionStatus.TIMEOUT]: 0,
      [ExecutionStatus.ABANDONED]: 0,
    };

    executionsData.data.forEach((execution) => {
      distribution[execution.status] =
        (distribution[execution.status] || 0) + 1;
    });

    return distribution;
  }, [executionsData]);

  // Calculate success rate
  const successRate = useMemo(() => {
    if (!executionsData?.data || executionsData.data.length === 0) return 0;

    const completed = executionsData.data.filter(
      (e) =>
        e.status === ExecutionStatus.COMPLETED ||
        e.status === ExecutionStatus.FAILED ||
        e.status === ExecutionStatus.TIMEOUT,
    );

    if (completed.length === 0) return 0;

    const succeeded = completed.filter(
      (e) => e.status === ExecutionStatus.COMPLETED,
    ).length;
    return Math.round((succeeded / completed.length) * 100);
  }, [executionsData]);

  // Format timestamp
  const formatTime = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return "just now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return date.toLocaleDateString();
  };

  // Get status color
  const getStatusColor = (status: ExecutionStatus) => {
    switch (status) {
      case ExecutionStatus.COMPLETED:
        return "text-green-600 bg-green-50";
      case ExecutionStatus.FAILED:
      case ExecutionStatus.TIMEOUT:
        return "text-red-600 bg-red-50";
      case ExecutionStatus.RUNNING:
        return "text-blue-600 bg-blue-50";
      case ExecutionStatus.REQUESTED:
      case ExecutionStatus.SCHEDULING:
      case ExecutionStatus.SCHEDULED:
        return "text-yellow-600 bg-yellow-50";
      case ExecutionStatus.CANCELLED:
      case ExecutionStatus.CANCELING:
        return "text-gray-600 bg-gray-50";
      case ExecutionStatus.ABANDONED:
        return "text-purple-600 bg-purple-50";
      default:
        return "text-gray-600 bg-gray-50";
    }
  };

  // Get status display name
  const getStatusDisplay = (status: ExecutionStatus): string => {
    return status;
  };

  const isLoading =
    packsLoading || actionsLoading || rulesLoading || executionsLoading;

  return (
    <div className="p-6">
      {/* Header */}
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-gray-900">Dashboard</h1>
        <div className="flex items-center gap-2 mt-2">
          <p className="text-gray-600">Welcome back, {user?.login || "User"}</p>
          {isConnected && (
            <span className="inline-flex items-center gap-1 text-xs text-green-600">
              <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse"></span>
              Live
            </span>
          )}
        </div>
      </div>

      {/* Metrics Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
        <Link
          to="/packs"
          className="bg-white rounded-lg shadow p-6 hover:shadow-md transition-shadow"
        >
          <p className="text-sm font-medium text-gray-600">Total Packs</p>
          <p className="text-3xl font-bold text-gray-900 mt-1">
            {isLoading ? "—" : totalPacks}
          </p>
          <p className="text-xs text-gray-500 mt-2">View all packs →</p>
        </Link>

        <Link
          to="/rules"
          className="bg-white rounded-lg shadow p-6 hover:shadow-md transition-shadow"
        >
          <p className="text-sm font-medium text-gray-600">Active Rules</p>
          <p className="text-3xl font-bold text-gray-900 mt-1">
            {isLoading ? "—" : activeRules}
          </p>
          <p className="text-xs text-gray-500 mt-2">Manage rules →</p>
        </Link>

        <Link
          to="/executions"
          className="bg-white rounded-lg shadow p-6 hover:shadow-md transition-shadow"
        >
          <p className="text-sm font-medium text-gray-600">
            Running Executions
          </p>
          <p className="text-3xl font-bold text-blue-600 mt-1">
            {isLoading ? "—" : runningCount}
          </p>
          <p className="text-xs text-gray-500 mt-2">View executions →</p>
        </Link>

        <Link
          to="/actions"
          className="bg-white rounded-lg shadow p-6 hover:shadow-md transition-shadow"
        >
          <p className="text-sm font-medium text-gray-600">Total Actions</p>
          <p className="text-3xl font-bold text-gray-900 mt-1">
            {isLoading ? "—" : totalActions}
          </p>
          <p className="text-xs text-gray-500 mt-2">Browse actions →</p>
        </Link>
      </div>

      {/* Status Overview & Recent Activity */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Status Distribution */}
        <div className="bg-white rounded-lg shadow p-6">
          <h2 className="text-lg font-semibold text-gray-900 mb-4">
            Execution Status
          </h2>
          {isLoading ? (
            <p className="text-gray-500 text-center py-8">Loading...</p>
          ) : executionsData?.data && executionsData.data.length > 0 ? (
            <div className="space-y-3">
              {Object.entries(statusDistribution).map(([status, count]) => {
                const countNum = typeof count === "number" ? count : 0;
                if (countNum === 0) return null;
                const percentage = executionsData?.data
                  ? Math.round((countNum / executionsData.data.length) * 100)
                  : 0;

                return (
                  <div key={status}>
                    <div className="flex items-center justify-between text-sm mb-1">
                      <span className="text-gray-700">{status}</span>
                      <span className="font-medium text-gray-900">
                        {countNum}
                      </span>
                    </div>
                    <div className="w-full bg-gray-200 rounded-full h-2">
                      <div
                        className={`h-2 rounded-full ${
                          status === ExecutionStatus.COMPLETED
                            ? "bg-green-500"
                            : status === ExecutionStatus.FAILED ||
                                status === ExecutionStatus.TIMEOUT
                              ? "bg-red-500"
                              : status === ExecutionStatus.RUNNING
                                ? "bg-blue-500"
                                : "bg-gray-400"
                        }`}
                        style={{ width: `${percentage}%` }}
                      ></div>
                    </div>
                  </div>
                );
              })}

              {/* Success Rate */}
              <div className="pt-3 mt-3 border-t border-gray-200">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-gray-700">Success Rate</span>
                  <span className="text-lg font-bold text-gray-900">
                    {successRate}%
                  </span>
                </div>
              </div>
            </div>
          ) : (
            <p className="text-gray-500 text-center py-8">No executions yet</p>
          )}
        </div>

        {/* Recent Activity */}
        <div className="bg-white rounded-lg shadow p-6 lg:col-span-2">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-gray-900">
              Recent Activity
            </h2>
            <Link
              to="/executions"
              className="text-sm text-blue-600 hover:text-blue-700"
            >
              View all →
            </Link>
          </div>

          {isLoading ? (
            <p className="text-gray-500 text-center py-8">Loading...</p>
          ) : executionsData?.data && executionsData.data.length > 0 ? (
            <div className="space-y-3 max-h-96 overflow-y-auto">
              {executionsData.data.map((execution) => (
                <Link
                  key={execution.id}
                  to={`/executions/${execution.id}`}
                  className="block p-3 rounded-lg border border-gray-200 hover:border-blue-300 hover:bg-blue-50 transition-colors"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-gray-900 truncate">
                          {execution.action_ref}
                        </span>
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${getStatusColor(
                            execution.status,
                          )}`}
                        >
                          {getStatusDisplay(execution.status)}
                        </span>
                      </div>
                      <div className="flex items-center gap-3 mt-1 text-xs text-gray-500">
                        <span>ID: {execution.id}</span>
                        <span>•</span>
                        <span>{formatTime(execution.created)}</span>
                      </div>
                    </div>
                  </div>
                </Link>
              ))}
            </div>
          ) : (
            <p className="text-gray-500 text-center py-8">No recent activity</p>
          )}
        </div>
      </div>

      {/* Analytics Section */}
      <div className="mt-8">
        <AnalyticsDashboard
          data={analyticsData}
          isLoading={analyticsLoading}
          error={analyticsError as Error | null}
          hours={analyticsHours}
          onHoursChange={setAnalyticsHours}
        />
      </div>
    </div>
  );
}
