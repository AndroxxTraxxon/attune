import { useParams, Link, useNavigate } from "react-router-dom";
import { usePack } from "@/hooks/usePacks";
import PackForm from "@/components/forms/PackForm";

export default function PackEditPage() {
  const { ref } = useParams<{ ref: string }>();
  const navigate = useNavigate();

  const { data: packResponse, isLoading, error } = usePack(ref!);
  const pack = packResponse?.data;

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
        </div>
      </div>
    );
  }

  if (error || !pack) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 rounded-lg p-6">
          <h3 className="text-lg font-semibold text-red-900 mb-2">
            Failed to load pack
          </h3>
          <p className="text-red-700">
            {error instanceof Error ? error.message : "Pack not found"}
          </p>
          <Link
            to="/packs"
            className="inline-block mt-4 text-red-600 hover:text-red-800"
          >
            ← Back to Packs
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6">
      {/* Header */}
      <div className="mb-6">
        <Link
          to={`/packs/${pack?.ref}`}
          className="text-sm text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Pack
        </Link>
        <h1 className="text-3xl font-bold text-gray-900">Edit Pack</h1>
        <p className="mt-2 text-gray-600">
          Update pack configuration: <strong>{pack?.label}</strong>
        </p>
        {pack?.is_standard && (
          <div className="mt-3 p-3 bg-yellow-50 border border-yellow-200 rounded-lg">
            <p className="text-sm text-yellow-800">
              <strong>Note:</strong> This is a standard pack. Only configuration
              values can be edited.
            </p>
          </div>
        )}
      </div>

      {/* Form */}
      {pack && (
        <PackForm
          pack={pack}
          onSuccess={() => navigate(`/packs/${pack.ref}`)}
          onCancel={() => navigate(`/packs/${pack.ref}`)}
        />
      )}
    </div>
  );
}
