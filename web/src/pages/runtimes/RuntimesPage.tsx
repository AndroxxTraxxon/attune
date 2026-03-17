import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { Code2, Pencil, Plus, Search, Trash2, X } from "lucide-react";
import type { RuntimeSummary } from "@/api";
import RuntimeForm from "@/components/forms/RuntimeForm";
import {
  useDeleteRuntime,
  useRuntime,
  useRuntimes,
} from "@/hooks/useRuntimes";

function formatJson(value: unknown): string {
  return JSON.stringify(value ?? null, null, 2);
}

export default function RuntimesPage() {
  const { ref } = useParams<{ ref?: string }>();
  const navigate = useNavigate();
  const { data, isLoading, error } = useRuntimes();
  const [searchQuery, setSearchQuery] = useState("");
  const runtimes = useMemo(() => data?.data || [], [data?.data]);

  const filteredRuntimes = useMemo(() => {
    if (!searchQuery.trim()) {
      return runtimes;
    }

    const query = searchQuery.toLowerCase();
    return runtimes.filter((runtime: RuntimeSummary) => {
      return (
        runtime.name.toLowerCase().includes(query) ||
        runtime.ref.toLowerCase().includes(query) ||
        runtime.pack_ref?.toLowerCase().includes(query) ||
        runtime.description?.toLowerCase().includes(query)
      );
    });
  }, [runtimes, searchQuery]);

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

  return (
    <div className="flex h-full">
      <div className="w-96 border-r border-gray-200 overflow-y-auto bg-gray-50">
        <div className="p-4 border-b border-gray-200 bg-white sticky top-0 z-10">
          <div className="flex items-center justify-between mb-1">
            <div>
              <h1 className="text-2xl font-bold">Runtimes</h1>
              <p className="text-sm text-gray-600 mt-1">
                {filteredRuntimes.length} of {runtimes.length} runtimes
              </p>
            </div>
            <button
              onClick={() => navigate("/runtimes/new")}
              className="inline-flex items-center px-3 py-1.5 bg-blue-600 text-white text-sm rounded-lg hover:bg-blue-700"
            >
              <Plus className="h-4 w-4 mr-1" />
              New Runtime
            </button>
          </div>

          <div className="mt-3 relative">
            <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
              <Search className="h-4 w-4 text-gray-400" />
            </div>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search runtimes..."
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

        <div className="p-2 space-y-1">
          {filteredRuntimes.length === 0 ? (
            <div className="bg-white p-8 text-center rounded-lg shadow-sm m-2">
              <p className="text-gray-500">
                {runtimes.length === 0
                  ? "No runtimes found"
                  : "No runtimes match your search"}
              </p>
            </div>
          ) : (
            filteredRuntimes.map((runtime: RuntimeSummary) => (
              <Link
                key={runtime.id}
                to={`/runtimes/${encodeURIComponent(runtime.ref)}`}
                className={`block p-3 rounded-lg transition-colors ${
                  ref === runtime.ref
                    ? "bg-blue-50 border-2 border-blue-500"
                    : "bg-white border-2 border-transparent hover:bg-gray-100 hover:border-gray-300"
                }`}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="font-medium text-sm text-gray-900 truncate">
                    {runtime.name}
                  </div>
                  <span className="text-[11px] px-2 py-0.5 rounded-full bg-gray-100 text-gray-600">
                    {runtime.pack_ref ?? "system"}
                  </span>
                </div>
                <div className="font-mono text-xs text-gray-500 mt-1 truncate">
                  {runtime.ref}
                </div>
                {runtime.description && (
                  <div className="text-xs text-gray-400 mt-1 line-clamp-2">
                    {runtime.description}
                  </div>
                )}
              </Link>
            ))
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {ref === "new" ? (
          <RuntimeForm />
        ) : ref ? (
          <RuntimeDetail runtimeRef={ref} />
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center text-gray-500">
              <Code2 className="mx-auto h-12 w-12 text-gray-400" />
              <h3 className="mt-2 text-sm font-medium text-gray-900">
                No runtime selected
              </h3>
              <p className="mt-1 text-sm text-gray-500">
                Select a runtime from the list to inspect or update it
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function RuntimeDetail({ runtimeRef }: { runtimeRef: string }) {
  const navigate = useNavigate();
  const { data, isLoading, error } = useRuntime(runtimeRef);
  const deleteRuntime = useDeleteRuntime();
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    setIsEditing(false);
  }, [runtimeRef]);

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
        </div>
      </div>
    );
  }

  if (error || !data?.data) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {error ? (error as Error).message : "Runtime not found"}</p>
        </div>
      </div>
    );
  }

  if (isEditing) {
    return (
      <RuntimeForm
        initialData={data.data}
        isEditing={true}
        onCancel={() => setIsEditing(false)}
      />
    );
  }

  const runtime = data.data;

  const handleDelete = async () => {
    if (!window.confirm(`Delete runtime "${runtime.ref}"?`)) {
      return;
    }

    await deleteRuntime.mutateAsync(runtime.ref);
    navigate("/runtimes");
  };

  return (
    <div className="p-6 max-w-6xl space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <h2 className="text-3xl font-bold text-gray-900">{runtime.name}</h2>
            <span className="text-xs px-2 py-1 rounded-full bg-gray-100 text-gray-700">
              {runtime.pack_ref ?? "system"}
            </span>
          </div>
          <p className="mt-2 font-mono text-sm text-gray-500">{runtime.ref}</p>
          {runtime.description && (
            <p className="mt-3 text-sm text-gray-700">{runtime.description}</p>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setIsEditing(true)}
            className="inline-flex items-center px-3 py-2 border border-gray-300 rounded-lg text-sm hover:bg-gray-50"
          >
            <Pencil className="h-4 w-4 mr-2" />
            Edit
          </button>
          <button
            onClick={handleDelete}
            className="inline-flex items-center px-3 py-2 border border-red-300 text-red-700 rounded-lg text-sm hover:bg-red-50"
          >
            <Trash2 className="h-4 w-4 mr-2" />
            Delete
          </button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <InfoCard label="Pack" value={runtime.pack_ref ?? "None"} />
        <InfoCard label="Created" value={new Date(runtime.created).toLocaleString()} />
        <InfoCard label="Updated" value={new Date(runtime.updated).toLocaleString()} />
      </div>

      <JsonCard title="Distributions" value={runtime.distributions} />
      <JsonCard title="Installation" value={runtime.installation} />
      <JsonCard title="Execution Config" value={runtime.execution_config} />
    </div>
  );
}

function InfoCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-white rounded-lg shadow p-5">
      <div className="text-xs font-medium uppercase tracking-wide text-gray-500">
        {label}
      </div>
      <div className="mt-2 text-sm text-gray-900 break-all">{value}</div>
    </div>
  );
}

function JsonCard({ title, value }: { title: string; value: unknown }) {
  return (
    <div className="bg-white rounded-lg shadow overflow-hidden">
      <div className="px-5 py-3 border-b border-gray-200">
        <h3 className="text-sm font-semibold text-gray-900">{title}</h3>
      </div>
      <pre className="p-5 text-xs text-gray-800 overflow-x-auto bg-gray-50">
        {formatJson(value)}
      </pre>
    </div>
  );
}
