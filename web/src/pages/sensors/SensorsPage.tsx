import { Link, useParams } from "react-router-dom";
import { useSensors, useSensor, useDeleteSensor } from "@/hooks/useSensors";
import { useState, useMemo } from "react";
import type { SensorSummary } from "@/api";
import { ChevronDown, ChevronRight, Search, X } from "lucide-react";

export default function SensorsPage() {
  const { ref } = useParams<{ ref?: string }>();
  const { data, isLoading, error } = useSensors({});
  const sensors = useMemo(() => data?.items || [], [data?.items]);
  const [collapsedPacks, setCollapsedPacks] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");

  // Filter sensors based on search query
  const filteredSensors = useMemo(() => {
    if (!searchQuery.trim()) return sensors;
    const query = searchQuery.toLowerCase();
    return sensors.filter((sensor: SensorSummary) => {
      return (
        sensor.label?.toLowerCase().includes(query) ||
        sensor.ref?.toLowerCase().includes(query) ||
        sensor.description?.toLowerCase().includes(query) ||
        sensor.pack_ref?.toLowerCase().includes(query)
      );
    });
  }, [sensors, searchQuery]);

  // Group filtered sensors by pack
  const sensorsByPack = useMemo(() => {
    const grouped = new Map<string, SensorSummary[]>();
    filteredSensors.forEach((sensor: SensorSummary) => {
      const packRef = sensor.pack_ref || "unknown";
      if (!grouped.has(packRef)) {
        grouped.set(packRef, []);
      }
      grouped.get(packRef)!.push(sensor);
    });
    // Sort packs alphabetically
    return new Map(
      [...grouped.entries()].sort((a, b) => a[0].localeCompare(b[0])),
    );
  }, [filteredSensors]);

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
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {(error as Error).message}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Left sidebar - Sensors List */}
      <div className="w-96 border-r border-gray-200 overflow-y-auto bg-gray-50">
        <div className="p-4 border-b border-gray-200 bg-white sticky top-0 z-10">
          <h1 className="text-2xl font-bold">Sensors</h1>
          <p className="text-sm text-gray-600 mt-1">
            {filteredSensors.length} of {sensors.length} sensors
          </p>

          {/* Search Bar */}
          <div className="mt-3 relative">
            <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
              <Search className="h-4 w-4 text-gray-400" />
            </div>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search sensors..."
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
          {sensors.length === 0 ? (
            <div className="bg-white p-8 text-center rounded-lg shadow-sm m-2">
              <p className="text-gray-500">No sensors found</p>
            </div>
          ) : filteredSensors.length === 0 ? (
            <div className="bg-white p-8 text-center rounded-lg shadow-sm m-2">
              <p className="text-gray-500">No sensors match your search</p>
              <button
                onClick={() => setSearchQuery("")}
                className="mt-2 text-sm text-blue-600 hover:text-blue-800"
              >
                Clear search
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {Array.from(sensorsByPack.entries()).map(
                ([packRef, packSensors]) => {
                  const isCollapsed = collapsedPacks.has(packRef);
                  return (
                    <div
                      key={packRef}
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
                          {packSensors.length}
                        </span>
                      </button>

                      {/* Sensors List */}
                      {!isCollapsed && (
                        <div className="p-1">
                          {packSensors.map((sensor: SensorSummary) => (
                            <Link
                              key={sensor.id}
                              to={`/sensors/${sensor.ref}`}
                              className={`block p-3 rounded transition-colors ${
                                ref === sensor.ref
                                  ? "bg-blue-50 border-2 border-blue-500"
                                  : "border-2 border-transparent hover:bg-gray-50"
                              }`}
                            >
                              <div className="flex items-center justify-between">
                                <div className="font-medium text-sm text-gray-900 truncate">
                                  {sensor.label}
                                </div>
                                <span
                                  className={`ml-2 inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                                    sensor.enabled
                                      ? "bg-green-100 text-green-800"
                                      : "bg-gray-100 text-gray-800"
                                  }`}
                                >
                                  {sensor.enabled ? "Enabled" : "Disabled"}
                                </span>
                              </div>
                              <div className="font-mono text-xs text-gray-500 mt-1 truncate">
                                {sensor.ref}
                              </div>
                              {sensor.description && (
                                <div className="text-xs text-gray-400 mt-1 line-clamp-2">
                                  {sensor.description}
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

      {/* Right panel - Sensor Detail or Empty State */}
      <div className="flex-1 overflow-y-auto">
        {ref ? (
          <SensorDetail sensorRef={ref} />
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
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                />
              </svg>
              <h3 className="mt-2 text-sm font-medium text-gray-900">
                No sensor selected
              </h3>
              <p className="mt-1 text-sm text-gray-500">
                Select a sensor from the list to view its details
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function SensorDetail({ sensorRef }: { sensorRef: string }) {
  const { data: sensor, isLoading, error } = useSensor(sensorRef);
  const deleteSensor = useDeleteSensor();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const handleDelete = async () => {
    try {
      await deleteSensor.mutateAsync(sensorRef);
      window.location.href = "/sensors";
    } catch (err) {
      console.error("Failed to delete sensor:", err);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
      </div>
    );
  }

  if (error || !sensor) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          <p>Error: {error ? (error as Error).message : "Sensor not found"}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-7xl mx-auto">
      {/* Header */}
      <div className="mb-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <h1 className="text-3xl font-bold">
              <span className="text-gray-500">{sensor.data?.pack_ref}.</span>
              {sensor.data?.label}
            </h1>
            <span
              className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-medium ${
                sensor.data?.enabled
                  ? "bg-green-100 text-green-800"
                  : "bg-gray-100 text-gray-800"
              }`}
            >
              {sensor.data?.enabled ? "Enabled" : "Disabled"}
            </span>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setShowDeleteConfirm(true)}
              disabled={deleteSensor.isPending}
              className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
            >
              Delete
            </button>
          </div>
        </div>
      </div>

      {/* Delete Confirmation Modal */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md">
            <h3 className="text-xl font-bold mb-4">Confirm Delete</h3>
            <p className="mb-6">
              Are you sure you want to delete sensor{" "}
              <strong>
                {sensor.data?.pack_ref}.{sensor.data?.label}
              </strong>
              ?
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

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Info Card */}
        <div className="lg:col-span-2 space-y-6">
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Sensor Information</h2>
            <dl className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <dt className="text-sm font-medium text-gray-500">Reference</dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono">
                  {sensor.data?.ref}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Label</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {sensor.data?.label}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Pack</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  <Link
                    to={`/packs/${sensor.data?.pack_ref}`}
                    className="text-blue-600 hover:text-blue-800"
                  >
                    {sensor.data?.pack_ref}
                  </Link>
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">
                  Entry Point
                </dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono">
                  {sensor.data?.entrypoint}
                </dd>
              </div>
              <div className="sm:col-span-2">
                <dt className="text-sm font-medium text-gray-500">
                  Description
                </dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {sensor.data?.description || "No description provided"}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Status</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {sensor.data?.enabled ? "Enabled" : "Disabled"}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">
                  Sensor Ref
                </dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono">
                  {sensor.data?.ref}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Created</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(sensor.data?.created || "").toLocaleString()}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500">Updated</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  {new Date(sensor.data?.updated || "").toLocaleString()}
                </dd>
              </div>
            </dl>
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Quick Actions */}
          <div className="bg-white shadow rounded-lg p-6">
            <h2 className="text-lg font-semibold mb-4">Quick Actions</h2>
            <div className="space-y-2">
              <Link
                to={`/packs/${sensor.data?.pack_ref}`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View Pack
              </Link>
              <Link
                to={`/triggers`}
                className="block w-full px-4 py-2 text-sm text-center bg-gray-100 hover:bg-gray-200 rounded"
              >
                View Triggers
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
