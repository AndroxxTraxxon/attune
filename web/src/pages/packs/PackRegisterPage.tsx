import { useState } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useRegisterPack } from "@/hooks/usePackTests";
import { AlertCircle, CheckCircle, Loader2, FolderOpen } from "lucide-react";

export default function PackRegisterPage() {
  const navigate = useNavigate();
  const registerPack = useRegisterPack();

  const [formData, setFormData] = useState({
    path: "",
    force: false,
    skipTests: false,
  });

  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccess(null);

    if (!formData.path) {
      setError("Pack path is required");
      return;
    }

    try {
      const result = await registerPack.mutateAsync({
        path: formData.path,
        force: formData.force,
        skipTests: formData.skipTests,
      });

      const packRef = result.data.pack.ref;
      setSuccess(
        `Pack '${result.data.pack.label}' registered successfully! ${
          result.data.tests_skipped
            ? "Tests were skipped."
            : result.data.test_result
              ? `Tests ${result.data.test_result.status}: ${result.data.test_result.passed}/${result.data.test_result.total_tests} passed.`
              : ""
        }`,
      );

      // Redirect to pack details after 2 seconds
      setTimeout(() => {
        navigate(`/packs/${packRef}`);
      }, 2000);
    } catch (err) {
      setError((err as Error).message || "Failed to register pack");
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, type, checked, value } = e.target;
    setFormData((prev) => ({
      ...prev,
      [name]: type === "checkbox" ? checked : value,
    }));
  };

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <div className="mb-6">
        <Link
          to="/packs"
          className="text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Packs
        </Link>
        <h1 className="text-3xl font-bold">Register Pack from Filesystem</h1>
        <p className="mt-2 text-gray-600">
          Load an existing pack directory with actions, sensors, and metadata
        </p>
      </div>

      {/* Info Box */}
      <div className="mb-6 bg-green-50 border border-green-200 rounded-lg p-5">
        <div className="flex items-start gap-3">
          <FolderOpen className="w-5 h-5 text-green-600 flex-shrink-0 mt-0.5" />
          <div>
            <h3 className="text-sm font-semibold text-green-900 mb-2">
              Filesystem-Based Pack Registration
            </h3>
            <p className="text-sm text-green-800 mb-2">
              This option registers a pack from a local directory containing:
            </p>
            <ul className="text-sm text-green-800 space-y-1 list-disc list-inside ml-2">
              <li>
                <strong>pack.yaml</strong> - Pack metadata and configuration
              </li>
              <li>
                <strong>actions/</strong> - Executable action scripts (Python,
                Shell, Node.js)
              </li>
              <li>
                <strong>sensors/</strong> - Sensor monitoring scripts (optional)
              </li>
              <li>
                <strong>rules/</strong> - Pre-defined automation rules
                (optional)
              </li>
              <li>
                <strong>workflows/</strong> - Workflow definitions (optional)
              </li>
            </ul>
            <div className="mt-3 pt-3 border-t border-green-300">
              <p className="text-xs text-green-700">
                <strong>Need an empty pack instead?</strong> If you're creating
                ad-hoc rules, workflows, or webhooks without filesystem-based
                actions,{" "}
                <Link
                  to="/packs/new"
                  className="underline hover:text-green-900 font-medium"
                >
                  create an empty pack
                </Link>{" "}
                instead.
              </p>
            </div>
          </div>
        </div>
      </div>

      {error && (
        <div className="mb-6 bg-red-50 border border-red-200 rounded-lg p-4 flex items-start gap-3">
          <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
          <div className="flex-1">
            <h3 className="text-sm font-semibold text-red-800">Error</h3>
            <p className="text-sm text-red-700 mt-1">{error}</p>
          </div>
        </div>
      )}

      {success && (
        <div className="mb-6 bg-green-50 border border-green-200 rounded-lg p-4 flex items-start gap-3">
          <CheckCircle className="w-5 h-5 text-green-600 flex-shrink-0 mt-0.5" />
          <div className="flex-1">
            <h3 className="text-sm font-semibold text-green-800">Success</h3>
            <p className="text-sm text-green-700 mt-1">{success}</p>
            <p className="text-xs text-green-600 mt-2">
              Redirecting to pack details...
            </p>
          </div>
        </div>
      )}

      <div className="bg-white shadow rounded-lg">
        <form onSubmit={handleSubmit} className="p-6 space-y-6">
          {/* Pack Path */}
          <div>
            <label
              htmlFor="path"
              className="block text-sm font-medium text-gray-700 mb-2"
            >
              Pack Directory Path <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              id="path"
              name="path"
              value={formData.path}
              onChange={handleChange}
              placeholder="/path/to/pack/directory"
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              required
            />
            <p className="mt-2 text-sm text-gray-500">
              Absolute path to the pack directory containing pack.yaml
            </p>
          </div>

          {/* Test Options */}
          <div className="border-t border-gray-200 pt-6">
            <h3 className="text-lg font-semibold text-gray-900 mb-4">
              Test Options
            </h3>

            <div className="space-y-4">
              {/* Skip Tests */}
              <div className="flex items-start">
                <div className="flex items-center h-5">
                  <input
                    type="checkbox"
                    id="skipTests"
                    name="skipTests"
                    checked={formData.skipTests}
                    onChange={handleChange}
                    className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                  />
                </div>
                <div className="ml-3">
                  <label
                    htmlFor="skipTests"
                    className="text-sm font-medium text-gray-700"
                  >
                    Skip Tests
                  </label>
                  <p className="text-sm text-gray-500">
                    Skip running tests during registration. Useful for
                    development or when tests are not yet ready.
                  </p>
                </div>
              </div>

              {/* Force Registration */}
              <div className="flex items-start">
                <div className="flex items-center h-5">
                  <input
                    type="checkbox"
                    id="force"
                    name="force"
                    checked={formData.force}
                    onChange={handleChange}
                    className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                  />
                </div>
                <div className="ml-3">
                  <label
                    htmlFor="force"
                    className="text-sm font-medium text-gray-700"
                  >
                    Force Registration
                  </label>
                  <p className="text-sm text-gray-500">
                    Proceed with registration even if pack exists or tests fail.
                    This will replace any existing pack with the same reference.
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* Info Box */}
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
            <h4 className="text-sm font-semibold text-blue-800 mb-2">
              Registration Process
            </h4>
            <ul className="text-sm text-blue-700 space-y-1 list-disc list-inside">
              <li>Pack metadata is read from pack.yaml</li>
              <li>Pack is registered in the database</li>
              <li>Workflows are automatically synced</li>
              {!formData.skipTests && (
                <li className="font-medium">
                  Tests are executed and must pass (unless forced)
                </li>
              )}
              {formData.skipTests && (
                <li className="text-blue-600 font-medium">
                  Tests will be skipped - no validation
                </li>
              )}
            </ul>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-3 pt-6 border-t border-gray-200">
            <button
              type="submit"
              disabled={registerPack.isPending || !!success}
              className="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium flex items-center gap-2"
            >
              {registerPack.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Registering...
                </>
              ) : (
                "Register Pack"
              )}
            </button>
            <Link
              to="/packs"
              className="px-6 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 transition-colors font-medium"
            >
              Cancel
            </Link>
          </div>
        </form>
      </div>

      {/* Help Section */}
      <div className="mt-6 bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-3">Need Help?</h3>
        <div className="space-y-3 text-sm text-gray-600">
          <div>
            <strong className="text-gray-900">Pack Directory:</strong> Must
            contain a valid pack.yaml file with pack metadata
          </div>
          <div>
            <strong className="text-gray-900">Testing:</strong> If tests are
            configured in pack.yaml, they will run automatically unless skipped
          </div>
          <div>
            <strong className="text-gray-900">Force Mode:</strong> Use this to
            replace existing packs or proceed despite test failures
          </div>
        </div>
      </div>
    </div>
  );
}
