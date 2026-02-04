import { useParams, Link, useNavigate } from "react-router-dom";
import { useRule } from "@/hooks/useRules";
import RuleForm from "@/components/forms/RuleForm";

export default function RuleEditPage() {
  const { ref } = useParams<{ ref: string }>();
  const navigate = useNavigate();

  const { data: ruleData, isLoading, error } = useRule(ref || "");
  const rule = ruleData?.data;

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
        </div>
      </div>
    );
  }

  if (error || !rule) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 rounded-lg p-6">
          <h3 className="text-lg font-semibold text-red-900 mb-2">
            Failed to load rule
          </h3>
          <p className="text-red-700">
            {error instanceof Error ? error.message : "Rule not found"}
          </p>
          <Link
            to="/rules"
            className="inline-block mt-4 text-red-600 hover:text-red-800"
          >
            ← Back to Rules
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
          to={`/rules/${rule.ref}`}
          className="text-sm text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Rule
        </Link>
        <h1 className="text-3xl font-bold text-gray-900">Edit Rule</h1>
        <p className="mt-2 text-gray-600">
          Update the automation rule: <strong>{rule.label}</strong>
        </p>
      </div>

      {/* Form */}
      <RuleForm
        rule={rule}
        onSuccess={() => navigate(`/rules/${rule.ref}`)}
        onCancel={() => navigate(`/rules/${rule.ref}`)}
      />
    </div>
  );
}
