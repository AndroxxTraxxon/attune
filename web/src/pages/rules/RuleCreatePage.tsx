import { Link } from "react-router-dom";
import RuleForm from "@/components/forms/RuleForm";

export default function RuleCreatePage() {
  return (
    <div className="p-6">
      {/* Header */}
      <div className="mb-6">
        <Link
          to="/rules"
          className="text-sm text-blue-600 hover:text-blue-800 mb-2 inline-block"
        >
          ← Back to Rules
        </Link>
        <h1 className="text-3xl font-bold text-gray-900">Create New Rule</h1>
        <p className="mt-2 text-gray-600">
          Define a new automation rule by connecting a trigger to an action
        </p>
      </div>

      {/* Form */}
      <RuleForm />
    </div>
  );
}
