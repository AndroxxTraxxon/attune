import { Link, useParams, useNavigate, useSearchParams } from "react-router-dom";
import {
  useActions,
  useAction,
  useDeleteAction,
  useUpdateAction,
} from "@/hooks/useActions";
import { useExecutions } from "@/hooks/useExecutions";
import { usePermissionSets } from "@/hooks/usePermissions";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
  ActionResponse,
  ActionSummary,
  ExecutionSummary,
  PermissionSetSummary,
} from "@/api";
import type { ParamSchemaProperty } from "@/components/common/ParamSchemaForm";
import {
  ChevronDown,
  ChevronRight,
  Search,
  X,
  Play,
  Plus,
  GitBranch,
  Pencil,
} from "lucide-react";
import ExecuteActionModal from "@/components/common/ExecuteActionModal";
import ErrorDisplay from "@/components/common/ErrorDisplay";
import { extractProperties } from "@/components/common/ParamSchemaForm";
import { STANDARD_EXECUTION_ACCESS_REF } from "@/lib/permissions";

export default function ActionsPage() {
  const { ref } = useParams<{ ref?: string }>();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { data, isLoading, error } = useActions();
  const actions = useMemo(() => data?.items || [], [data?.items]);
  const [collapsedPacks, setCollapsedPacks] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const sidebarRef = useRef<HTMLDivElement | null>(null);
  const headerRef = useRef<HTMLDivElement | null>(null);
  const packSectionRefs = useRef<Record<string, HTMLDivElement | null>>({});

  // Filter actions based on search query
  const filteredActions = useMemo(() => {
    if (!searchQuery.trim()) return actions;
    const query = searchQuery.toLowerCase();
    return actions.filter((action: ActionSummary) => {
      return (
        action.label?.toLowerCase().includes(query) ||
        action.ref?.toLowerCase().includes(query) ||
        action.description?.toLowerCase().includes(query) ||
        action.pack_ref?.toLowerCase().includes(query)
      );
    });
  }, [actions, searchQuery]);

  // Group filtered actions by pack
  const actionsByPack = useMemo(() => {
    const grouped = new Map<string, ActionSummary[]>();
    filteredActions.forEach((action: ActionSummary) => {
      const packRef = action.pack_ref;
      if (!grouped.has(packRef)) {
        grouped.set(packRef, []);
      }
      grouped.get(packRef)!.push(action);
    });
    // Sort packs alphabetically
    return new Map(
      [...grouped.entries()].sort((a, b) => a[0].localeCompare(b[0])),
    );
  }, [filteredActions]);

  const requestedPack = searchParams.get("pack")?.trim() || "";
  const focusedPack = useMemo(() => {
    if (!requestedPack) {
      return null;
    }

    return actionsByPack.has(requestedPack) ? requestedPack : null;
  }, [actionsByPack, requestedPack]);

  const orderedPackEntries = useMemo(() => {
    const entries = Array.from(actionsByPack.entries());
    if (!focusedPack) {
      return entries;
    }

    return entries.sort(([left], [right]) => {
      if (left === focusedPack) {
        return -1;
      }
      if (right === focusedPack) {
        return 1;
      }
      return left.localeCompare(right);
    });
  }, [actionsByPack, focusedPack]);

  useEffect(() => {
    if (!focusedPack) {
      return;
    }

    const target = packSectionRefs.current[focusedPack];
    const container = sidebarRef.current;
    if (!target || !container) {
      return;
    }

    const stickyHeaderHeight = headerRef.current?.offsetHeight ?? 0;
    const targetTop =
      target.offsetTop - stickyHeaderHeight - 8;

    container.scrollTo({
      top: Math.max(0, targetTop),
      behavior: "auto",
    });
  }, [focusedPack, orderedPackEntries.length]);

  const togglePack = (packRef: string) => {
    setCollapsedPacks((prev) => {
      const next = new Set(prev);
      if (next.has(packRef)) {
        next.delete(packRef);
      } else {
        next.add(packRef);
      }
      return next;
    });
  };

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
        <ErrorDisplay error={error} title="Failed to load actions" />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Left sidebar - Actions List */}
      <div ref={sidebarRef} className="w-96 border-r border-gray-200 overflow-y-auto bg-gray-50">
        <div ref={headerRef} className="p-4 border-b border-gray-200 bg-white sticky top-0 z-10">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-2xl font-bold">Actions</h1>
              <p className="text-sm text-gray-600 mt-1">
                {filteredActions.length} of {actions.length} actions
                {focusedPack ? ` • Focused pack: ${focusedPack}` : ""}
              </p>
            </div>
            <button
              onClick={() => navigate("/actions/workflows/new")}
              className="flex items-center gap-1.5 px-3 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors shadow-sm"
              title="Create a new workflow action"
            >
              <Plus className="w-4 h-4" />
              Workflow
            </button>
          </div>

          {/* Search Bar */}
          <div className="mt-3 relative">
            <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
              <Search className="h-4 w-4 text-gray-400" />
            </div>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search actions..."
              className="block w-full pl-10 pr-10 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery("")}
                className="absolute inset-y-0 right-0 pr-3 flex items-center"
              >
                <X className="h-4 w-4 text-gray-400 hover:text-gray-600" />
              </button>
            )}
          </div>
        </div>
        <div className="p-2">
          {actions.length === 0 ? (
            <div className="bg-white p-8 text-center rounded-lg shadow-sm m-2">
              <p className="text-gray-500">No actions found</p>
            </div>
          ) : filteredActions.length === 0 ? (
            <div className="bg-white p-8 text-center rounded-lg shadow-sm m-2">
              <p className="text-gray-500">No actions match your search</p>
              <button
                onClick={() => setSearchQuery("")}
                className="mt-2 text-sm text-blue-600 hover:text-blue-800"
              >
                Clear search
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {orderedPackEntries.map(
                ([packRef, packActions]) => {
                  const isCollapsed =
                    focusedPack !== null && packRef !== focusedPack
                      ? true
                      : collapsedPacks.has(packRef);
                  return (
                    <div
                      key={packRef}
                      ref={(element) => {
                        packSectionRefs.current[packRef] = element;
                      }}
                      className="bg-white rounded-lg shadow-sm overflow-hidden"
                    >
                      {/* Pack Header */}
                      <button
                        onClick={() => togglePack(packRef)}
                        className="w-full px-3 py-2 flex items-center justify-between hover:bg-gray-50 transition-colors border-b border-gray-200"
                      >
                        <div className="flex items-center gap-2">
                          {isCollapsed ? (
                            <ChevronRight className="w-4 h-4 text-gray-500" />
                          ) : (
                            <ChevronDown className="w-4 h-4 text-gray-500" />
                          )}
                          <span className="font-semibold text-sm text-gray-900">
                            {packRef}
                          </span>
                        </div>
                        <span className="text-xs text-gray-500 bg-gray-100 px-2 py-0.5 rounded">
                          {packActions.length}
                        </span>
                      </button>

                      {/* Actions List */}
                      {!isCollapsed && (
                        <div className="p-1">
                          {packActions.map((action: ActionSummary) => (
                            <Link
                              key={action.id}
                              to={`/actions/${action.ref}`}
                              className={`block p-3 rounded transition-colors ${
                                ref === action.ref
                                  ? "bg-blue-50 border-2 border-blue-500"
                                  : "border-2 border-transparent hover:bg-gray-50"
                              }`}
                            >
                              <div className="font-medium text-sm text-gray-900 truncate flex items-center gap-1.5">
                                {action.workflow_def && (
                                  <span title="Workflow">
                                    <GitBranch className="w-3.5 h-3.5 text-purple-500 flex-shrink-0" />
                                  </span>
                                )}
                                {action.label}
                              </div>
                              <div className="font-mono text-xs text-gray-500 mt-1 truncate">
                                {action.ref}
                              </div>
                              {action.description && (
                                <div className="text-xs text-gray-400 mt-1 line-clamp-2">
                                  {action.description}
                                </div>
                              )}
                            </Link>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                },
              )}
            </div>
          )}
        </div>
      </div>

      {/* Right panel - Action Detail or Empty State */}
      <div className="flex-1 overflow-y-auto">
        {ref ? (
          <ActionDetail actionRef={ref} />
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center text-gray-500">
              <svg
                className="mx-auto h-12 w-12 text-gray-400"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M13 10V3L4 14h7v7l9-11h-7z"
                />
              </svg>
              <h3 className="mt-2 text-sm font-medium text-gray-900">
                No action selected
              </h3>
              <p className="mt-1 text-sm text-gray-500">
                Select an action from the list to view its details
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ActionDetail({ actionRef }: { actionRef: string }) {
  const navigate = useNavigate();
  const { data: action, isLoading, error } = useAction(actionRef);
  const { data: executionsData } = useExecutions({
    actionRef: actionRef,
    pageSize: 10,
  });
  const deleteAction = useDeleteAction();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showExecuteModal, setShowExecuteModal] = useState(false);

  const handleDelete = async () => {
    try {
      await deleteAction.mutateAsync(actionRef);
      // Navigate back to actions list without selection
      window.location.href = "/actions";
    } catch (err) {
      console.error("Failed to delete action:", err);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
      </div>
    );
  }

  if (error || !action) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {error ? (error as Error).message : "Action not found"}</p>
        </div>
      </div>
    );
  }

  const executions = executionsData?.items || [];
  const actionDetails = action.data as ActionResponse;
  const paramSchema = action.data?.param_schema || {};
  const properties = extractProperties(paramSchema);
  const paramEntries = Object.entries(properties);
  const outSchema = action.data?.out_schema || {};
  const outProperties = extractProperties(outSchema);
  const outEntries = Object.entries(outProperties);

  return (
    <div className="p-6 max-w-7xl mx-auto">
      {/* Header */}
      <div className="mb-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <h1 className="text-3xl font-bold">
              <span className="text-gray-500">{action.data?.pack_ref}.</span>
              {action.data?.label}
            </h1>
          </div>
          <div className="flex gap-2">
            {action.data?.workflow_def && (
              <button
                onClick={() =>
                  navigate(`/actions/workflows/${action.data!.ref}/edit`)
                }
                className="px-4 py-2 bg-purple-600 text-white rounded hover:bg-purple-700 flex items-center gap-2"
              >
                <Pencil className="h-4 w-4" />
                Edit Workflow
              </button>
            )}
            <button
              onClick={() => setShowExecuteModal(true)}
              className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 flex items-center gap-2"
            >
              <Play className="h-4 w-4" />
              Execute
            </button>
            {/* Only show delete button for ad-hoc actions (not from pack installation) */}
            {action.data?.is_adhoc && (
              <button
                onClick={() => setShowDeleteConfirm(true)}
                disabled={deleteAction.isPending}
                className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
              >
                Delete
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Delete Confirmation Modal */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md">
            <h3 className="text-xl font-bold mb-4">Confirm Delete</h3>
            <p className="mb-6">
              Are you sure you want to delete action{" "}
              <strong>
                {action.data?.pack_ref}.{action.data?.label}
              </strong>
              ? This will also delete all associated executions.
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setShowDeleteConfirm(false)}
                className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300"
              >
                Cancel
              </button>
              <button
                onClick={handleDelete}
                className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Execute Action Modal */}
      {showExecuteModal && (
        <ExecuteActionModal
          action={action.data!}
          onClose={() => setShowExecuteModal(false)}
        />
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Info Card */}
        <div className="lg:col-span-2 space-y-6">
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Action Information</h2>
            <dl className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <dt className="text-sm font-medium text-gray-500">Reference</dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono">
                  {action.data?.ref}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Label</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {action.data?.label}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Pack</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  <Link
                    to={`/packs/${action.data?.pack_ref}`}
                    className="text-blue-600 hover:text-blue-800"
                  >
                    {action.data?.pack_ref}
                  </Link>
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">
                  Entry Point
                </dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono">
                  {action.data?.entrypoint}
                </dd>
              </div>
              <div className="sm:col-span-2">
                <dt className="text-sm font-medium text-gray-500">
                  Description
                </dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {action.data?.description || "No description provided"}
                </dd>
              </div>
              {action.data?.runtime && (
                <div>
                  <dt className="text-sm font-medium text-gray-500">Runtime</dt>
                  <dd className="mt-1 text-sm text-gray-900">
                    Runtime #{action.data.runtime}
                  </dd>
                </div>
              )}
              <div>
                <dt className="text-sm font-medium text-gray-500">Created</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(action.data?.created || "").toLocaleString()}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Updated</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(action.data?.updated || "").toLocaleString()}
                </dd>
              </div>
            </dl>

            {paramEntries.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-medium text-gray-900 mb-3">
                  Parameters
                </h3>
                <div className="space-y-3">
                  {paramEntries.map(
                    ([key, param]: [string, ParamSchemaProperty]) => (
                      <div
                        key={key}
                        className="border border-gray-200 rounded p-3"
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2">
                              <span className="font-mono font-semibold text-sm">
                                {key}
                              </span>
                              {param?.required && (
                                <span className="text-xs px-2 py-0.5 bg-red-100 text-red-700 rounded">
                                  Required
                                </span>
                              )}
                              {param?.secret && (
                                <span className="text-xs px-2 py-0.5 bg-yellow-100 text-yellow-700 rounded">
                                  Secret
                                </span>
                              )}
                              <span className="text-xs px-2 py-0.5 bg-gray-100 text-gray-700 rounded">
                                {param?.type || "any"}
                              </span>
                            </div>
                            {param?.description && (
                              <p className="text-sm text-gray-600 mt-1">
                                {param.description}
                              </p>
                            )}
                            {param?.default !== undefined && (
                              <p className="text-xs text-gray-500 mt-1">
                                Default:{" "}
                                <code className="bg-gray-100 px-1 rounded">
                                  {JSON.stringify(param.default)}
                                </code>
                              </p>
                            )}
                            {param?.enum && param.enum.length > 0 && (
                              <p className="text-xs text-gray-500 mt-1">
                                Values:{" "}
                                {param.enum
                                  .map((v: string) => `"${v}"`)
                                  .join(", ")}
                              </p>
                            )}
                          </div>
                        </div>
                      </div>
                    ),
                  )}
                </div>
              </div>
            )}

            {outEntries.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-medium text-gray-900 mb-3">
                  Output Schema
                </h3>
                <div className="space-y-3">
                  {outEntries.map(
                    ([key, param]: [string, ParamSchemaProperty]) => (
                      <div
                        key={key}
                        className="border border-gray-200 rounded p-3"
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2">
                              <span className="font-mono font-semibold text-sm">
                                {key}
                              </span>
                              {param?.required && (
                                <span className="text-xs px-2 py-0.5 bg-red-100 text-red-700 rounded">
                                  Required
                                </span>
                              )}
                              {param?.secret && (
                                <span className="text-xs px-2 py-0.5 bg-yellow-100 text-yellow-700 rounded">
                                  Secret
                                </span>
                              )}
                              <span className="text-xs px-2 py-0.5 bg-gray-100 text-gray-700 rounded">
                                {param?.type || "any"}
                              </span>
                            </div>
                            {param?.description && (
                              <p className="text-sm text-gray-600 mt-1">
                                {param.description}
                              </p>
                            )}
                            {param?.default !== undefined && (
                              <p className="text-xs text-gray-500 mt-1">
                                Default:{" "}
                                <code className="bg-gray-100 px-1 rounded">
                                  {JSON.stringify(param.default)}
                                </code>
                              </p>
                            )}
                            {param?.enum && param.enum.length > 0 && (
                              <p className="text-xs text-gray-500 mt-1">
                                Values:{" "}
                                {param.enum
                                  .map((v: string) => `"${v}"`)
                                  .join(", ")}
                              </p>
                            )}
                          </div>
                        </div>
                      </div>
                    ),
                  )}
                </div>
              </div>
            )}
          </div>

          <DefaultExecutionPermissionsCard action={actionDetails} />

          {/* Recent Executions */}
          <div className="bg-white shadow rounded-lg p-6">
            <div className="flex justify-between items-center mb-4">
              <h2 className="text-xl font-semibold">
                Recent Executions ({executions.length})
              </h2>
              <Link
                to={`/executions?action_ref=${action.data?.ref}`}
                className="text-sm text-blue-600 hover:text-blue-800"
              >
                View All →
              </Link>
            </div>
            {executions.length === 0 ? (
              <p className="text-gray-500 text-center py-8">
                No executions yet
              </p>
            ) : (
              <div className="space-y-2">
                {executions.map((execution: ExecutionSummary) => (
                  <Link
                    key={execution.id}
                    to={`/executions/${execution.id}`}
                    className="block p-3 border border-gray-200 rounded hover:bg-gray-50 transition-colors"
                  >
                    <div className="flex justify-between items-center">
                      <div className="flex items-center gap-3">
                        <span className="text-sm font-mono text-gray-600">
                          #{execution.id}
                        </span>
                        <span
                          className={`px-2 py-1 text-xs rounded ${
                            execution.status === "completed"
                              ? "bg-green-100 text-green-800"
                              : execution.status === "failed"
                                ? "bg-red-100 text-red-800"
                                : execution.status === "running"
                                  ? "bg-blue-100 text-blue-800"
                                  : "bg-gray-100 text-gray-800"
                          }`}
                        >
                          {execution.status}
                        </span>
                      </div>
                      <span className="text-xs text-gray-500">
                        {new Date(execution.created).toLocaleString()}
                      </span>
                    </div>
                  </Link>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Quick Stats */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-lg font-semibold mb-4">Statistics</h2>
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-600">Total Executions</span>
                <span className="text-lg font-semibold">
                  {executionsData?.pagination?.total_items || 0}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-600">Recent</span>
                <span className="text-lg font-semibold">
                  {executions.length}
                </span>
              </div>
            </div>
          </div>

          {/* Quick Actions */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-lg font-semibold mb-4">Quick Actions</h2>
            <div className="space-y-2">
              <Link
                to={`/packs/${action.data?.pack_ref}`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View Pack
              </Link>
              <Link
                to={`/rules?action=${action.data?.ref}`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View Rules
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function normalizePermissionSetRefs(input: string): string[] {
  const seen = new Set<string>();
  return input
    .split(/[\s,]+/)
    .map((value) => value.trim())
    .filter(Boolean)
    .filter((value) => {
      if (seen.has(value)) {
        return false;
      }
      seen.add(value);
      return true;
    });
}

function formatPermissionSetRefs(refs: string[] | undefined): string {
  return refs?.join("\n") ?? "";
}

function PermissionSetRefChips({ refs }: { refs: string[] }) {
  if (refs.length === 0) {
    return (
      <p className="text-sm text-gray-500">
        No permission set refs configured.
      </p>
    );
  }

  return (
    <div className="flex flex-wrap gap-2">
      {refs.map((ref) => (
        ref === STANDARD_EXECUTION_ACCESS_REF ? (
          <span
            key={ref}
            className="font-mono text-xs px-2 py-1 rounded bg-green-50 text-green-700"
            title="Standard action/pack-scoped keys and artifacts access"
          >
            {ref}
          </span>
        ) : (
          <Link
            key={ref}
            to={`/access-control/permission-sets/${ref}`}
            className="font-mono text-xs px-2 py-1 rounded bg-blue-50 text-blue-700 hover:bg-blue-100"
            title={`View permission set ${ref}`}
          >
            {ref}
          </Link>
        )
      ))}
    </div>
  );
}

function DefaultExecutionPermissionsCard({ action }: { action: ActionResponse }) {
  const currentRefs = action.default_execution_permission_set_refs ?? [];
  const updateAction = useUpdateAction();
  const { data: permissionSets, isLoading: permissionSetsLoading } =
    usePermissionSets();
  const [draftRefs, setDraftRefs] = useState(formatPermissionSetRefs(currentRefs));
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    setDraftRefs(formatPermissionSetRefs(action.default_execution_permission_set_refs));
    setErrorMessage(null);
  }, [action.ref, action.default_execution_permission_set_refs]);

  const normalizedDraftRefs = useMemo(
    () => normalizePermissionSetRefs(draftRefs),
    [draftRefs],
  );
  const knownPermissionSetRefs = useMemo(
    () => new Set((permissionSets ?? []).map((set: PermissionSetSummary) => set.ref)),
    [permissionSets],
  );
  const unknownRefs =
    permissionSets && !permissionSetsLoading
      ? normalizedDraftRefs.filter(
          (ref) =>
            ref !== STANDARD_EXECUTION_ACCESS_REF &&
            !knownPermissionSetRefs.has(ref),
        )
      : [];
  const hasChanges =
    normalizedDraftRefs.join("\n") !== formatPermissionSetRefs(currentRefs);

  const save = async () => {
    setErrorMessage(null);
    try {
      await updateAction.mutateAsync({
        ref: action.ref,
        data: {
          label: action.label,
          description: action.description ?? null,
          entrypoint: action.entrypoint,
          runtime: action.runtime ?? null,
          required_worker_runtimes: action.required_worker_runtimes ?? {},
          param_schema: action.param_schema ?? null,
          out_schema: action.out_schema ?? null,
          accesses_mcp: action.accesses_mcp,
          default_execution_permission_set_refs: normalizedDraftRefs,
        },
      });
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : "Failed to save permission refs");
    }
  };

  return (
    <div className="bg-white shadow rounded-lg p-6">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div>
          <h2 className="text-xl font-semibold">Default Execution Token Access</h2>
          <p className="text-sm text-gray-600 mt-1">
            These permission set refs are applied to executions when the caller
            does not explicitly override token access. Leave empty for no
            execution-scoped API token by default.
          </p>
        </div>
      </div>

      <div className="mb-4">
        <dt className="text-sm font-medium text-gray-500 mb-2">
          Current permission set refs
        </dt>
        <dd>
          <PermissionSetRefChips refs={currentRefs} />
        </dd>
      </div>

      <label className="block text-sm font-medium text-gray-700 mb-2">
        Configure refs
      </label>
      <textarea
        value={draftRefs}
        onChange={(event) => setDraftRefs(event.target.value)}
        rows={Math.max(3, Math.min(8, normalizedDraftRefs.length + 1))}
        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
        placeholder="standard&#10;core.agent_reader&#10;my_pack.agent_scope"
      />
      <p className="text-xs text-gray-500 mt-2">
        Enter permission set refs separated by commas, spaces, or new lines.
        Use <span className="font-mono">standard</span> for the action/pack-scoped keys
        and artifacts access built into execution tokens. Refs are stored as
        metadata refs, not database IDs.
      </p>

      {unknownRefs.length > 0 && (
        <div className="mt-3 rounded-md border border-yellow-200 bg-yellow-50 px-3 py-2 text-sm text-yellow-800">
          Unknown permission set refs:{" "}
          <span className="font-mono">{unknownRefs.join(", ")}</span>. Saving
          will let the API validate whether they can be delegated.
        </div>
      )}

      {errorMessage && (
        <div className="mt-3 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
          {errorMessage}
        </div>
      )}

      <div className="mt-4 flex items-center gap-2">
        <button
          type="button"
          onClick={save}
          disabled={!hasChanges || updateAction.isPending}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
        >
          {updateAction.isPending ? "Saving..." : "Save token access"}
        </button>
        <button
          type="button"
          onClick={() => setDraftRefs("")}
          disabled={updateAction.isPending || normalizedDraftRefs.length === 0}
          className="px-4 py-2 bg-gray-100 text-gray-700 rounded hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        >
          Clear
        </button>
      </div>
    </div>
  );
}
