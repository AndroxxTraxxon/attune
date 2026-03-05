import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { OpenAPI } from "@/api";
import type { ActionResponse } from "@/api";
import { Play, X } from "lucide-react";
import ParamSchemaForm, {
  validateParamSchema,
  extractProperties,
  type ParamSchema,
} from "@/components/common/ParamSchemaForm";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JsonValue = any;

interface ExecuteActionModalProps {
  action: ActionResponse;
  onClose: () => void;
  initialParameters?: Record<string, JsonValue>;
}

/**
 * Shared modal for executing an action with a dynamic parameter form.
 *
 * Used from:
 * - ActionDetail page (Execute button)
 * - ExecutionDetailPage (Re-Run button, with initialParameters pre-filled from previous execution config)
 */
export default function ExecuteActionModal({
  action,
  onClose,
  initialParameters,
}: ExecuteActionModalProps) {
  const queryClient = useQueryClient();

  const paramSchema: ParamSchema = (action.param_schema as ParamSchema) || {};
  const paramProperties = extractProperties(paramSchema);

  // If initialParameters are provided, use them (stripping out any keys not in the schema)
  const buildInitialValues = (): Record<string, JsonValue> => {
    if (!initialParameters) return {};
    const values: Record<string, JsonValue> = {};
    // Include all initial parameters - even those not in the schema
    // so users can see exactly what was run before
    for (const [key, value] of Object.entries(initialParameters)) {
      if (value !== undefined && value !== null) {
        values[key] = value;
      }
    }
    // Also fill in defaults for any schema properties not covered
    for (const [key, param] of Object.entries(paramProperties)) {
      if (values[key] === undefined && param?.default !== undefined) {
        values[key] = param.default;
      }
    }
    return values;
  };

  const [parameters, setParameters] =
    useState<Record<string, JsonValue>>(buildInitialValues);
  const [paramErrors, setParamErrors] = useState<Record<string, string>>({});
  const [envVars, setEnvVars] = useState<Array<{ key: string; value: string }>>(
    [{ key: "", value: "" }],
  );

  const executeAction = useMutation({
    mutationFn: async (params: {
      parameters: Record<string, JsonValue>;
      envVars: Array<{ key: string; value: string }>;
    }) => {
      const token =
        typeof OpenAPI.TOKEN === "function"
          ? await OpenAPI.TOKEN({} as Parameters<typeof OpenAPI.TOKEN>[0])
          : OpenAPI.TOKEN;

      const response = await fetch(
        `${OpenAPI.BASE}/api/v1/executions/execute`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${token}`,
          },
          body: JSON.stringify({
            action_ref: action.ref,
            parameters: params.parameters,
            env_vars: params.envVars
              .filter((ev) => ev.key.trim() !== "")
              .reduce(
                (acc, ev) => {
                  acc[ev.key] = ev.value;
                  return acc;
                },
                {} as Record<string, string>,
              ),
          }),
        },
      );

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || "Failed to execute action");
      }

      return response.json();
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["executions"] });
      onClose();
      if (data?.data?.id) {
        window.location.href = `/executions/${data.data.id}`;
      }
    },
  });

  const validateForm = (): boolean => {
    const errors = validateParamSchema(paramSchema, parameters);
    setParamErrors(errors);
    return Object.keys(errors).length === 0;
  };

  const handleExecute = async () => {
    if (!validateForm()) {
      return;
    }

    try {
      await executeAction.mutateAsync({ parameters, envVars });
    } catch (err) {
      console.error("Failed to execute action:", err);
    }
  };

  const addEnvVar = () => {
    setEnvVars([...envVars, { key: "", value: "" }]);
  };

  const removeEnvVar = (index: number) => {
    if (envVars.length > 1) {
      setEnvVars(envVars.filter((_, i) => i !== index));
    }
  };

  const updateEnvVar = (
    index: number,
    field: "key" | "value",
    value: string,
  ) => {
    const updated = [...envVars];
    updated[index][field] = value;
    setEnvVars(updated);
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-lg p-6 max-w-2xl w-full max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-xl font-bold">
            {initialParameters ? "Re-Run Action" : "Execute Action"}
          </h3>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600"
          >
            <X className="h-6 w-6" />
          </button>
        </div>

        <div className="mb-4">
          <p className="text-sm text-gray-600">
            Action:{" "}
            <span className="font-mono text-gray-900">{action.ref}</span>
          </p>
          {action.description && (
            <p className="text-sm text-gray-600 mt-1">{action.description}</p>
          )}
          {initialParameters && (
            <p className="text-xs text-blue-600 mt-2 bg-blue-50 px-3 py-1.5 rounded">
              Parameters pre-filled from previous execution. Modify as needed
              before re-running.
            </p>
          )}
        </div>

        {executeAction.error && (
          <div className="mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm">
            {(executeAction.error as Error).message}
          </div>
        )}

        <div className="mb-6">
          <h4 className="text-sm font-semibold text-gray-700 mb-2">
            Parameters
          </h4>
          <ParamSchemaForm
            schema={paramSchema}
            values={parameters}
            onChange={setParameters}
            errors={paramErrors}
          />
        </div>

        <div className="mb-6">
          <h4 className="text-sm font-semibold text-gray-700 mb-2">
            Environment Variables
          </h4>
          <p className="text-xs text-gray-500 mb-3">
            Optional environment variables for this execution (e.g., DEBUG,
            LOG_LEVEL)
          </p>
          <div className="space-y-2">
            {envVars.map((envVar, index) => (
              <div key={index} className="flex gap-2 items-start">
                <input
                  type="text"
                  placeholder="Key"
                  value={envVar.key}
                  onChange={(e) => updateEnvVar(index, "key", e.target.value)}
                  className="flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <input
                  type="text"
                  placeholder="Value"
                  value={envVar.value}
                  onChange={(e) => updateEnvVar(index, "value", e.target.value)}
                  className="flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <button
                  type="button"
                  onClick={() => removeEnvVar(index)}
                  disabled={envVars.length === 1}
                  className="px-3 py-2 text-red-600 hover:text-red-700 disabled:text-gray-300 disabled:cursor-not-allowed"
                  title="Remove"
                >
                  <X className="h-5 w-5" />
                </button>
              </div>
            ))}
          </div>
          <button
            type="button"
            onClick={addEnvVar}
            className="mt-2 text-sm text-blue-600 hover:text-blue-700"
          >
            + Add Environment Variable
          </button>
        </div>

        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            disabled={executeAction.isPending}
            className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleExecute}
            disabled={executeAction.isPending}
            className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 flex items-center gap-2"
          >
            {executeAction.isPending ? (
              <>
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white" />
                Executing...
              </>
            ) : (
              <>
                <Play className="h-4 w-4" />
                {initialParameters ? "Re-Run" : "Execute"}
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
