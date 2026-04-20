import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  Eye,
  Pencil,
  Plus,
  Search,
  Workflow,
} from "lucide-react";
import Pagination from "@/components/executions/Pagination";
import { useQueueStream } from "@/hooks/useQueueStream";
import { useQueues } from "@/hooks/useQueues";
import { getQueueSourceBadge, formatDateTime } from "@/components/queues/queueUtils";

export default function QueuesPage() {
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState("");
  const [enabledFilter, setEnabledFilter] = useState<"all" | "enabled" | "disabled">("all");
  const [managementFilter, setManagementFilter] = useState<"all" | "api" | "pack">("all");
  const pageSize = 20;

  const queryParams = useMemo(() => ({
    page,
    pageSize,
    search: search.trim() || undefined,
    enabled:
      enabledFilter === "all"
        ? undefined
        : enabledFilter === "enabled",
    isAdhoc:
      managementFilter === "all"
        ? undefined
        : managementFilter === "api",
  }), [enabledFilter, managementFilter, page, search]);

  const { data, isLoading, error, isFetching } = useQueues(queryParams);
  useQueueStream();
  const queues = data?.data ?? [];
  const pagination = data?.pagination;
  const total = pagination?.total_items ?? 0;
  const hasActiveFilters =
    search.trim().length > 0 || enabledFilter !== "all" || managementFilter !== "all";

  const clearFilters = () => {
    setSearch("");
    setEnabledFilter("all");
    setManagementFilter("all");
    setPage(1);
  };

  return (
    <div className="p-6 pb-28">
      <div className="mb-6 flex items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Work Queues</h1>
          <p className="mt-2 text-gray-600">
            Browse queue definitions, inspect queue state, and manage API-managed queues.
          </p>
        </div>
        <Link
          to="/queues/new"
          className="inline-flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 transition-colors"
        >
          <Plus className="h-4 w-4" />
          Create Queue
        </Link>
      </div>

      <div className="mb-6 rounded-lg bg-white p-4 shadow">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700">
              <span className="inline-flex items-center gap-2">
                <Search className="h-4 w-4" />
                Search queues
              </span>
            </label>
            <input
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setPage(1);
              }}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
              placeholder="Search by ref, label, or description"
            />
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700">
              Enabled state
            </label>
            <select
              value={enabledFilter}
              onChange={(e) => {
                setEnabledFilter(e.target.value as typeof enabledFilter);
                setPage(1);
              }}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="all">All queues</option>
              <option value="enabled">Enabled only</option>
              <option value="disabled">Disabled only</option>
            </select>
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700">
              Management source
            </label>
            <select
              value={managementFilter}
              onChange={(e) => {
                setManagementFilter(e.target.value as typeof managementFilter);
                setPage(1);
              }}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="all">All queues</option>
              <option value="api">API-managed</option>
              <option value="pack">Pack-managed</option>
            </select>
          </div>
        </div>

        <div className="mt-4 flex items-center justify-between">
          <div className="text-sm text-gray-600">
            {queues.length > 0
              ? `Showing ${queues.length} of ${total} queue${total === 1 ? "" : "s"}`
              : "No queues found"}
            {isFetching && !isLoading ? " • refreshing…" : ""}
          </div>
          {hasActiveFilters && (
            <button
              type="button"
              onClick={clearFilters}
              className="text-sm text-gray-600 hover:text-gray-900"
            >
              Clear filters
            </button>
          )}
        </div>
      </div>

      <div className="overflow-hidden rounded-lg bg-white shadow">
        {isLoading ? (
          <div className="p-12 text-center">
            <div className="inline-block h-8 w-8 animate-spin rounded-full border-b-2 border-blue-600" />
            <p className="mt-4 text-gray-600">Loading queues...</p>
          </div>
        ) : error ? (
          <div className="p-12 text-center">
            <p className="text-red-600">Failed to load queues</p>
            <p className="mt-2 text-sm text-gray-600">
              {error instanceof Error ? error.message : "Unknown error"}
            </p>
          </div>
        ) : queues.length === 0 ? (
          <div className="p-12 text-center">
            <Workflow className="mx-auto h-12 w-12 text-gray-400" />
            <p className="mt-4 text-gray-600">No queues found</p>
            <p className="mt-1 text-sm text-gray-500">
              {hasActiveFilters
                ? "Try adjusting your filters."
                : "Create your first API-managed queue to get started."}
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    Queue
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    Source
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    Status
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    Dispatch action
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">
                    Updated
                  </th>
                  <th className="px-6 py-3 text-right text-xs font-medium uppercase tracking-wider text-gray-500">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 bg-white">
                {queues.map((queue) => {
                  const sourceBadge = getQueueSourceBadge(queue.is_adhoc);
                  return (
                    <tr key={queue.id} className="hover:bg-gray-50">
                      <td className="px-6 py-4">
                        <div className="min-w-0">
                          <Link
                            to={`/queues/${encodeURIComponent(queue.ref)}`}
                            className="block truncate text-sm font-medium text-blue-600 hover:text-blue-800"
                          >
                            {queue.label}
                          </Link>
                          <div className="truncate text-xs font-mono text-gray-500">
                            {queue.ref}
                          </div>
                          {queue.description && (
                            <p className="mt-1 truncate text-sm text-gray-600">
                              {queue.description}
                            </p>
                          )}
                        </div>
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap">
                        <div>
                          <span
                            className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ${sourceBadge.classes}`}
                          >
                            {sourceBadge.label}
                          </span>
                          {queue.pack_ref && (
                            <div className="mt-1 text-xs text-gray-500">
                              Pack: {queue.pack_ref}
                            </div>
                          )}
                        </div>
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap">
                        <span
                          className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ${queue.enabled ? "bg-green-100 text-green-800" : "bg-gray-100 text-gray-700"}`}
                        >
                          {queue.enabled ? "Enabled" : "Disabled"}
                        </span>
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-700 font-mono">
                        {queue.dispatch_action_ref}
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-700">
                        {formatDateTime(queue.updated)}
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-right text-sm">
                        <div className="flex items-center justify-end gap-2">
                          <Link
                            to={`/queues/${encodeURIComponent(queue.ref)}`}
                            className="text-gray-500 hover:text-blue-600"
                            title="View queue"
                          >
                            <Eye className="h-4 w-4" />
                          </Link>
                          {queue.is_adhoc && (
                            <Link
                              to={`/queues/${encodeURIComponent(queue.ref)}/edit`}
                              className="text-gray-500 hover:text-blue-600"
                              title="Edit queue"
                            >
                              <Pencil className="h-4 w-4" />
                            </Link>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <Pagination
        page={page}
        setPage={setPage}
        pageSize={pageSize}
        itemCount={queues.length}
        total={total}
        hasPrevious={pagination?.has_previous}
        hasNext={pagination?.has_next}
        itemLabel="queues"
        floating
      />
    </div>
  );
}
