import { Link } from "react-router-dom";
import { Info, Workflow, Radio, PencilRuler } from "lucide-react";
import PackForm from "@/components/forms/PackForm";

export default function PackCreatePage() {
  return (
    <div className="p-6 max-w-5xl mx-auto">
      {/* Header */}
      <div className="mb-6">
        <Link
          to="/packs"
          className="text-sm text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Packs
        </Link>
        <h1 className="text-3xl font-bold text-gray-900">Create Empty Pack</h1>
        <p className="mt-2 text-gray-600">
          Create an empty pack for ad-hoc rules, workflows, webhook triggers,
          and custom actions
        </p>
      </div>

      {/* Info Box */}
      <div className="mb-6 bg-blue-50 border border-blue-200 rounded-lg p-5">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
          <div>
            <h3 className="text-sm font-semibold text-blue-900 mb-2">
              When to Use Empty Packs
            </h3>
            <p className="text-sm text-blue-800 mb-3">
              Empty packs provide a namespace for ad-hoc automation content that
              doesn't require filesystem-based pack structure. Perfect for:
            </p>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <div className="flex items-start gap-2">
                <PencilRuler className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-800">
                  <strong>Ad-hoc Rules</strong>
                  <p className="text-xs text-blue-700 mt-0.5">
                    Quick automation rules created via the UI
                  </p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <Workflow className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-800">
                  <strong>Custom Workflows</strong>
                  <p className="text-xs text-blue-700 mt-0.5">
                    Workflow actions defined in the database
                  </p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <Radio className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-800">
                  <strong>Webhook Triggers</strong>
                  <p className="text-xs text-blue-700 mt-0.5">
                    Webhook endpoints for external integrations
                  </p>
                </div>
              </div>
            </div>
            <p className="text-xs text-blue-700 mt-3">
              <strong>Tip:</strong> Use remote installation for existing pack
              directories that are available from git, archives, or configured
              registries.
            </p>
          </div>
        </div>
      </div>

      {/* Form */}
      <PackForm />
    </div>
  );
}
