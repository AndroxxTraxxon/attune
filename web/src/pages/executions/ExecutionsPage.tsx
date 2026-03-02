import { Link, useSearchParams } from "react-router-dom";
import { useExecutions } from "@/hooks/useExecutions";
import { useExecutionStream } from "@/hooks/useExecutionStream";
import { ExecutionStatus } from "@/api";
import { useState, useMemo, memo, useCallback, useEffect } from "react";
import { Search, X, List, GitBranch } from "lucide-react";
import MultiSelect from "@/components/common/MultiSelect";
import AutocompleteInput from "@/components/common/AutocompleteInput";
import {
  useFilterSuggestions,
  useMergedSuggestions,
} from "@/hooks/useFilterSuggestions";
import WorkflowExecutionTree from "@/components/executions/WorkflowExecutionTree";
import ExecutionPreviewPanel from "@/components/executions/ExecutionPreviewPanel";

type ViewMode = "all" | "workflow";

const VIEW_MODE_STORAGE_KEY = "attune:executions:viewMode";

// Memoized filter input component for non-ref fields (e.g. Executor ID)
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

// Memoized results table component - only re-renders when query data changes,
// NOT when the user types in filter inputs.
const ExecutionsResultsTable = memo(
  ({
    executions,
    isLoading,
    isFetching,
    error,
    hasActiveFilters,
    clearFilters,
    page,
    setPage,
    pageSize,
    total,
    selectedExecutionId,
    onSelectExecution,
  }: {
    executions: any[];
    isLoading: boolean;
    isFetching: boolean;
    error: Error | null;
    hasActiveFilters: boolean;
    clearFilters: () => void;
    page: number;
    setPage: (page: number) => void;
    pageSize: number;
    total: number;
    selectedExecutionId: number | null;
    onSelectExecution: (id: number) => void;
  }) => {
    const totalPages = Math.ceil(total / pageSize);

    // Initial load (no cached data yet)
    if (isLoading && executions.length === 0) {
      return (
        <div className="bg-white shadow rounded-lg">
          <div className="flex items-center justify-center h-64">
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
          </div>
        </div>
      );
    }

    // Error with no cached data to show
    if (error && executions.length === 0) {
      return (
        <div className="bg-white shadow rounded-lg">
          <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
            <p>Error: {error.message}</p>
          </div>
        </div>
      );
    }

    // Empty results
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
        {/* Inline loading overlay - shows on top of previous results while fetching */}
        {isFetching && (
          <div className="absolute inset-0 bg-white/60 z-10 flex items-center justify-center rounded-lg">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
          </div>
        )}

        {/* Non-fatal error banner (data still shown from cache) */}
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
            <tbody className="bg-white divide-y divide-gray-200">
              {executions.map((exec: any) => (
                <tr
                  key={exec.id}
                  data-execution-id={exec.id}
                  className={`hover:bg-gray-50 cursor-pointer ${
                    selectedExecutionId === exec.id
                      ? "bg-blue-50 hover:bg-blue-50"
                      : ""
                  }`}
                  onClick={() => onSelectExecution(exec.id)}
                >
                  <td className="px-6 py-4 font-mono text-sm">
                    <Link
                      to={`/executions/${exec.id}`}
                      className="text-blue-600 hover:text-blue-800"
                      onClick={(e) => e.stopPropagation()}
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

        {/* Pagination */}
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
                  to{" "}
                  <span className="font-medium">
                    {Math.min(page * pageSize, total)}
                  </span>{" "}
                  of <span className="font-medium">{total}</span> executions
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
      </div>
    );
  },
);

ExecutionsResultsTable.displayName = "ExecutionsResultsTable";

export default function ExecutionsPage() {
  const [searchParams] = useSearchParams();

  // --- View mode toggle ---
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    const stored = localStorage.getItem(VIEW_MODE_STORAGE_KEY);
    if (stored === "all" || stored === "workflow") return stored;
    const param = searchParams.get("view");
    if (param === "all" || param === "workflow") return param;
    return "all";
  });

  // --- Filter input state (updates immediately on keystroke) ---
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

  // --- Debounced filter state (drives API calls, updates after delay) ---
  const [debouncedFilters, setDebouncedFilters] = useState(searchFilters);
  const [debouncedStatuses, setDebouncedStatuses] = useState(selectedStatuses);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedFilters(searchFilters);
    }, 500);
    return () => clearTimeout(timer);
  }, [searchFilters]);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedStatuses(selectedStatuses);
    }, 300);
    return () => clearTimeout(timer);
  }, [selectedStatuses]);

  // --- Autocomplete suggestions ---
  const baseSuggestions = useFilterSuggestions();

  // --- Build query params from debounced state ---
  const queryParams = useMemo(() => {
    const params: any = { page, pageSize };
    if (debouncedFilters.pack) params.packName = debouncedFilters.pack;
    if (debouncedFilters.rule) params.ruleRef = debouncedFilters.rule;
    if (debouncedFilters.action) params.actionRef = debouncedFilters.action;
    if (debouncedFilters.trigger) params.triggerRef = debouncedFilters.trigger;
    if (debouncedFilters.executor)
      params.executor = parseInt(debouncedFilters.executor, 10);
    if (debouncedStatuses.length === 1) {
      params.status = debouncedStatuses[0] as ExecutionStatus;
    }
    if (viewMode === "workflow") {
      params.topLevelOnly = true;
    }
    return params;
  }, [page, pageSize, debouncedFilters, debouncedStatuses, viewMode]);

  const { data, isLoading, isFetching, error } = useExecutions(queryParams);
  const { isConnected } = useExecutionStream({ enabled: true });

  const executions = useMemo(() => data?.data || [], [data]);
  const total = data?.pagination?.total_items || 0;

  // Derive refs from currently-loaded execution data (no setState needed)
  const loadedRefs = useMemo(() => {
    const packs = new Set<string>();
    const rules = new Set<string>();
    const actions = new Set<string>();
    const triggers = new Set<string>();

    for (const exec of executions) {
      if (exec.action_ref) {
        const pack = (exec.action_ref as string).split(".")[0];
        if (pack) packs.add(pack);
        actions.add(exec.action_ref as string);
      }
      if (exec.rule_ref) rules.add(exec.rule_ref as string);
      if (exec.trigger_ref) triggers.add(exec.trigger_ref as string);
    }

    return {
      packs: [...packs],
      rules: [...rules],
      actions: [...actions],
      triggers: [...triggers],
    };
  }, [executions]);

  // Merge base entity suggestions + loaded data refs
  const packSuggestions = useMergedSuggestions(
    baseSuggestions.packNames,
    loadedRefs.packs,
  );
  const ruleSuggestions = useMergedSuggestions(
    baseSuggestions.ruleRefs,
    loadedRefs.rules,
  );
  const actionSuggestions = useMergedSuggestions(
    baseSuggestions.actionRefs,
    loadedRefs.actions,
  );
  const triggerSuggestions = useMergedSuggestions(
    baseSuggestions.triggerRefs,
    loadedRefs.triggers,
  );

  // Client-side filtering for multiple status selection (when > 1 selected)
  const filteredExecutions = useMemo(() => {
    if (debouncedStatuses.length <= 1) return executions;
    return executions.filter((exec: any) =>
      debouncedStatuses.includes(exec.status),
    );
  }, [executions, debouncedStatuses]);

  const handleFilterChange = useCallback((field: string, value: string) => {
    setSearchFilters((prev) => ({ ...prev, [field]: value }));
    setPage(1);
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
    setPage(1);
  }, []);

  const hasActiveFilters =
    Object.values(searchFilters).some((v) => v !== "") ||
    selectedStatuses.length > 0;

  const [selectedExecutionId, setSelectedExecutionId] = useState<number | null>(
    null,
  );

  const handleSelectExecution = useCallback((id: number) => {
    setSelectedExecutionId((prev) => (prev === id ? null : id));
  }, []);

  const handleClosePreview = useCallback(() => {
    setSelectedExecutionId(null);
  }, []);

  const handleViewModeChange = useCallback((mode: ViewMode) => {
    setViewMode(mode);
    localStorage.setItem(VIEW_MODE_STORAGE_KEY, mode);
    setPage(1);
  }, []);

  // --- Keyboard arrow-key navigation for execution list ---
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;

      // Don't interfere with inputs, selects, textareas
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;

      const list = filteredExecutions;
      if (!list || list.length === 0) return;

      e.preventDefault();

      setSelectedExecutionId((prevId) => {
        if (prevId == null) {
          // Nothing selected — pick first or last depending on direction
          const nextId =
            e.key === "ArrowDown" ? list[0].id : list[list.length - 1].id;
          requestAnimationFrame(() => {
            document
              .querySelector(`[data-execution-id="${nextId}"]`)
              ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
          });
          return nextId;
        }

        const currentIndex = list.findIndex((ex: any) => ex.id === prevId);
        if (currentIndex === -1) {
          const nextId = list[0].id;
          requestAnimationFrame(() => {
            document
              .querySelector(`[data-execution-id="${nextId}"]`)
              ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
          });
          return nextId;
        }

        let nextIndex: number;
        if (e.key === "ArrowDown") {
          nextIndex =
            currentIndex < list.length - 1 ? currentIndex + 1 : currentIndex;
        } else {
          nextIndex = currentIndex > 0 ? currentIndex - 1 : currentIndex;
        }

        const nextId = list[nextIndex].id;
        requestAnimationFrame(() => {
          document
            .querySelector(`[data-execution-id="${nextId}"]`)
            ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
        });
        return nextId;
      });
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [filteredExecutions]);

  return (
    <div className="flex h-full">
      {/* Main content area */}
      <div
        className={`flex-1 min-w-0 overflow-y-auto p-6 ${selectedExecutionId ? "mr-0" : ""}`}
      >
        {/* Header - always visible */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold">Executions</h1>
            {isConnected && (
              <div className="flex items-center gap-1.5 text-xs text-green-600 bg-green-50 border border-green-200 rounded-full px-2.5 py-1">
                <div className="h-1.5 w-1.5 rounded-full bg-green-500 animate-pulse" />
                <span>Live</span>
              </div>
            )}
            {isFetching && hasActiveFilters && (
              <p className="text-sm text-gray-500">Searching executions...</p>
            )}
          </div>
          <div className="flex items-center gap-4">
            {/* View mode toggle */}
            <div className="inline-flex rounded-lg border border-gray-300 bg-white shadow-sm">
              <button
                onClick={() => handleViewModeChange("all")}
                className={`inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-l-lg transition-colors ${
                  viewMode === "all"
                    ? "bg-blue-600 text-white"
                    : "text-gray-600 hover:bg-gray-50"
                }`}
              >
                <List className="h-4 w-4" />
                All
              </button>
              <button
                onClick={() => handleViewModeChange("workflow")}
                className={`inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-r-lg transition-colors ${
                  viewMode === "workflow"
                    ? "bg-blue-600 text-white"
                    : "text-gray-600 hover:bg-gray-50"
                }`}
              >
                <GitBranch className="h-4 w-4" />
                By Workflow
              </button>
            </div>
          </div>
        </div>

        {/* Filter section - always mounted, never unmounts during loading */}
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
            <AutocompleteInput
              label="Pack"
              value={searchFilters.pack}
              onChange={(value) => handleFilterChange("pack", value)}
              suggestions={packSuggestions}
              placeholder="e.g., core"
            />
            <AutocompleteInput
              label="Rule"
              value={searchFilters.rule}
              onChange={(value) => handleFilterChange("rule", value)}
              suggestions={ruleSuggestions}
              placeholder="e.g., core.on_timer"
            />
            <AutocompleteInput
              label="Action"
              value={searchFilters.action}
              onChange={(value) => handleFilterChange("action", value)}
              suggestions={actionSuggestions}
              placeholder="e.g., core.echo"
            />
            <AutocompleteInput
              label="Trigger"
              value={searchFilters.trigger}
              onChange={(value) => handleFilterChange("trigger", value)}
              suggestions={triggerSuggestions}
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

        {/* Results section - isolated from filter state, only depends on query results */}
        {viewMode === "all" ? (
          <ExecutionsResultsTable
            executions={filteredExecutions}
            isLoading={isLoading}
            isFetching={isFetching}
            error={error as Error | null}
            hasActiveFilters={hasActiveFilters}
            clearFilters={clearFilters}
            page={page}
            setPage={setPage}
            pageSize={pageSize}
            total={total}
            selectedExecutionId={selectedExecutionId}
            onSelectExecution={handleSelectExecution}
          />
        ) : (
          <WorkflowExecutionTree
            executions={filteredExecutions}
            isLoading={isLoading}
            isFetching={isFetching}
            error={error as Error | null}
            hasActiveFilters={hasActiveFilters}
            clearFilters={clearFilters}
            page={page}
            setPage={setPage}
            pageSize={pageSize}
            total={total}
            workflowActionRefs={baseSuggestions.workflowActionRefs}
            selectedExecutionId={selectedExecutionId}
            onSelectExecution={handleSelectExecution}
          />
        )}
      </div>

      {/* Right-side preview panel */}
      {selectedExecutionId && (
        <div className="w-[400px] flex-shrink-0 h-full">
          <ExecutionPreviewPanel
            executionId={selectedExecutionId}
            onClose={handleClosePreview}
          />
        </div>
      )}
    </div>
  );
}
