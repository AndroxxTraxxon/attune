import { Link, useSearchParams } from "react-router-dom";
import { useExecutions } from "@/hooks/useExecutions";
import { useExecutionStream } from "@/hooks/useExecutionStream";
import { ExecutionStatus } from "@/api";
import { useState, useMemo, memo, useCallback, useEffect } from "react";
import { Search, X } from "lucide-react";
import MultiSelect from "@/components/common/MultiSelect";

// Memoized filter input component to prevent re-render on WebSocket updates
const FilterInput = memo(
  ({
    label,
    value,
    onChange,
    placeholder,
  }: {
    label: string;
    value: string;
    onChange: (value: string) => void;
    placeholder: string;
  }) => (
    <div>
      <label className="block text-sm font-medium text-gray-700 mb-1">
        {label}
      </label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
      />
    </div>
  ),
);

FilterInput.displayName = "FilterInput";

// Status options moved outside component to prevent recreation
const STATUS_OPTIONS = [
  { value: ExecutionStatus.REQUESTED, label: "Requested" },
  { value: ExecutionStatus.SCHEDULING, label: "Scheduling" },
  { value: ExecutionStatus.SCHEDULED, label: "Scheduled" },
  { value: ExecutionStatus.RUNNING, label: "Running" },
  { value: ExecutionStatus.COMPLETED, label: "Completed" },
  { value: ExecutionStatus.FAILED, label: "Failed" },
  { value: ExecutionStatus.CANCELING, label: "Canceling" },
  { value: ExecutionStatus.CANCELLED, label: "Cancelled" },
  { value: ExecutionStatus.TIMEOUT, label: "Timeout" },
  { value: ExecutionStatus.ABANDONED, label: "Abandoned" },
];

export default function ExecutionsPage() {
  const [searchParams] = useSearchParams();

  // Initialize filters from URL query parameters
  const [page, setPage] = useState(1);
  const pageSize = 50;
  const [searchFilters, setSearchFilters] = useState({
    pack: searchParams.get("pack_name") || "",
    rule: searchParams.get("rule_ref") || "",
    action: searchParams.get("action_ref") || "",
    trigger: searchParams.get("trigger_ref") || "",
    executor: searchParams.get("executor") || "",
  });
  const [selectedStatuses, setSelectedStatuses] = useState<string[]>(() => {
    const status = searchParams.get("status");
    return status ? [status] : [];
  });

  // Debounced filter state for API calls
  const [debouncedFilters, setDebouncedFilters] = useState(searchFilters);
  const [debouncedStatuses, setDebouncedStatuses] = useState(selectedStatuses);

  // Debounce filter changes (500ms delay)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedFilters(searchFilters);
    }, 500);

    return () => clearTimeout(timer);
  }, [searchFilters]);

  // Debounce status changes (300ms delay - shorter since it's a selection)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedStatuses(selectedStatuses);
    }, 300);

    return () => clearTimeout(timer);
  }, [selectedStatuses]);

  const queryParams = useMemo(() => {
    const params: any = { page, pageSize };
    if (debouncedFilters.pack) params.packName = debouncedFilters.pack;
    if (debouncedFilters.rule) params.ruleRef = debouncedFilters.rule;
    if (debouncedFilters.action) params.actionRef = debouncedFilters.action;
    if (debouncedFilters.trigger) params.triggerRef = debouncedFilters.trigger;
    if (debouncedFilters.executor)
      params.executor = parseInt(debouncedFilters.executor, 10);

    // Include status filter if exactly one status is selected
    // API only supports single status, so we use the first one for filtering
    // and show all results if multiple are selected
    if (debouncedStatuses.length === 1) {
      params.status = debouncedStatuses[0] as ExecutionStatus;
    }

    return params;
  }, [page, pageSize, debouncedFilters, debouncedStatuses]);

  const { data, isLoading, error } = useExecutions(queryParams);

  // Subscribe to real-time updates for all executions
  const { isConnected } = useExecutionStream({ enabled: true });

  const executions = data?.data || [];
  const total = data?.pagination?.total_items || 0;
  const totalPages = Math.ceil(total / pageSize);

  // Client-side filtering for multiple status selection (when > 1 selected)
  const filteredExecutions = useMemo(() => {
    // If no statuses selected or only one (already filtered by API), show all
    if (debouncedStatuses.length <= 1) {
      return executions;
    }
    // If multiple statuses selected, filter client-side
    return executions.filter((exec: any) =>
      debouncedStatuses.includes(exec.status),
    );
  }, [executions, debouncedStatuses]);

  const handleFilterChange = useCallback((field: string, value: string) => {
    setSearchFilters((prev) => ({ ...prev, [field]: value }));
    setPage(1); // Reset to first page on filter change
  }, []);

  const clearFilters = useCallback(() => {
    setSearchFilters({
      pack: "",
      rule: "",
      action: "",
      trigger: "",
      executor: "",
    });
    setSelectedStatuses([]);
    setPage(1); // Reset to first page
  }, []);

  const hasActiveFilters =
    Object.values(searchFilters).some((v) => v !== "") ||
    selectedStatuses.length > 0;

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {(error as Error).message}</p>
        </div>
      </div>
    );
  }

  const getStatusColor = (status: ExecutionStatus) => {
    switch (status) {
      case ExecutionStatus.COMPLETED:
        return "bg-green-100 text-green-800";
      case ExecutionStatus.FAILED:
      case ExecutionStatus.TIMEOUT:
        return "bg-red-100 text-red-800";
      case ExecutionStatus.RUNNING:
        return "bg-blue-100 text-blue-800";
      case ExecutionStatus.SCHEDULED:
      case ExecutionStatus.SCHEDULING:
      case ExecutionStatus.REQUESTED:
        return "bg-yellow-100 text-yellow-800";
      default:
        return "bg-gray-100 text-gray-800";
    }
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-3xl font-bold">Executions</h1>
          {isLoading && hasActiveFilters && (
            <p className="text-sm text-gray-500 mt-1">
              Searching executions...
            </p>
          )}
        </div>
        {isConnected && (
          <div className="flex items-center gap-2 text-sm text-green-600">
            <div className="h-2 w-2 rounded-full bg-green-600 animate-pulse" />
            <span>Live Updates</span>
          </div>
        )}
      </div>

      {/* Search Filters */}
      <div className="bg-white shadow rounded-lg p-4 mb-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Search className="h-5 w-5 text-gray-400" />
            <h2 className="text-lg font-semibold">Filter Executions</h2>
          </div>
          {hasActiveFilters && (
            <button
              onClick={clearFilters}
              className="flex items-center gap-1 text-sm text-gray-600 hover:text-gray-900"
            >
              <X className="h-4 w-4" />
              Clear Filters
            </button>
          )}
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-4">
          <FilterInput
            label="Pack"
            value={searchFilters.pack}
            onChange={(value) => handleFilterChange("pack", value)}
            placeholder="e.g., core"
          />
          <FilterInput
            label="Rule"
            value={searchFilters.rule}
            onChange={(value) => handleFilterChange("rule", value)}
            placeholder="e.g., core.on_timer"
          />
          <FilterInput
            label="Action"
            value={searchFilters.action}
            onChange={(value) => handleFilterChange("action", value)}
            placeholder="e.g., core.echo"
          />
          <FilterInput
            label="Trigger"
            value={searchFilters.trigger}
            onChange={(value) => handleFilterChange("trigger", value)}
            placeholder="e.g., core.timer"
          />
          <FilterInput
            label="Executor ID"
            value={searchFilters.executor}
            onChange={(value) => handleFilterChange("executor", value)}
            placeholder="e.g., 1"
          />
          <div>
            <MultiSelect
              label="Status"
              options={STATUS_OPTIONS}
              value={selectedStatuses}
              onChange={setSelectedStatuses}
              placeholder="All Statuses"
            />
          </div>
        </div>
      </div>

      {filteredExecutions.length === 0 ? (
        <div className="bg-white p-12 text-center rounded-lg shadow">
          <p>
            {executions.length === 0
              ? "No executions found"
              : "No executions match the selected filters"}
          </p>
          {executions.length > 0 && hasActiveFilters && (
            <button
              onClick={clearFilters}
              className="mt-3 text-sm text-blue-600 hover:text-blue-800"
            >
              Clear filters
            </button>
          )}
        </div>
      ) : (
        <>
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
              <tbody className="bg-white divide-y divide-gray-200">
                {filteredExecutions.map((exec: any) => (
                  <tr key={exec.id} className="hover:bg-gray-50">
                    <td className="px-6 py-4 font-mono text-sm">
                      <Link
                        to={`/executions/${exec.id}`}
                        className="text-blue-600 hover:text-blue-800"
                      >
                        #{exec.id}
                      </Link>
                    </td>
                    <td className="px-6 py-4">
                      <span className="text-sm text-gray-900">
                        {exec.action_ref}
                      </span>
                    </td>
                    <td className="px-6 py-4">
                      {exec.rule_ref ? (
                        <span className="text-sm text-gray-700">
                          {exec.rule_ref}
                        </span>
                      ) : (
                        <span className="text-sm text-gray-400 italic">-</span>
                      )}
                    </td>
                    <td className="px-6 py-4">
                      {exec.trigger_ref ? (
                        <span className="text-sm text-gray-700">
                          {exec.trigger_ref}
                        </span>
                      ) : (
                        <span className="text-sm text-gray-400 italic">-</span>
                      )}
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`px-2 py-1 text-xs rounded ${getStatusColor(exec.status)}`}
                      >
                        {exec.status}
                      </span>
                    </td>
                    <td className="px-6 py-4 text-sm text-gray-500">
                      {new Date(exec.created).toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {totalPages > 1 && (
            <div className="bg-gray-50 px-6 py-4 flex items-center justify-between border-t border-gray-200">
              <div className="flex-1 flex justify-between sm:hidden">
                <button
                  onClick={() => setPage(page - 1)}
                  disabled={page === 1}
                  className="relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Previous
                </button>
                <button
                  onClick={() => setPage(page + 1)}
                  disabled={page === totalPages}
                  className="ml-3 relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Next
                </button>
              </div>
              <div className="hidden sm:flex-1 sm:flex sm:items-center sm:justify-between">
                <div>
                  <p className="text-sm text-gray-700">
                    Showing{" "}
                    <span className="font-medium">
                      {(page - 1) * pageSize + 1}
                    </span>{" "}
                    to
                    <span className="font-medium">
                      {Math.min(page * pageSize, total)}
                    </span>{" "}
                    of &nbsp;
                    <span className="font-medium">{total}</span> executions
                  </p>
                </div>
                <div>
                  <nav className="relative z-0 inline-flex rounded-md shadow-sm -space-x-px">
                    <button
                      onClick={() => setPage(page - 1)}
                      disabled={page === 1}
                      className="relative inline-flex items-center px-2 py-2 rounded-l-md border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      Previous
                    </button>
                    <button
                      onClick={() => setPage(page + 1)}
                      disabled={page === totalPages}
                      className="relative inline-flex items-center px-2 py-2 rounded-r-md border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      Next
                    </button>
                  </nav>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
