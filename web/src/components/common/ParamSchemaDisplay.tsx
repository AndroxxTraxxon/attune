/**
 * ParamSchemaDisplay - Read-only display component for parameters
 * Shows parameter values in a human-friendly format based on their schema
 * Supports standard JSON Schema format (https://json-schema.org/draft/2020-12/schema)
 */

/**
 * Standard JSON Schema format for parameters
 */
export interface ParamSchema {
  type?: "object";
  properties?: {
    [key: string]: {
      type?: "string" | "number" | "integer" | "boolean" | "array" | "object";
      description?: string;
      default?: any;
      enum?: string[];
      minimum?: number;
      maximum?: number;
      minLength?: number;
      maxLength?: number;
      secret?: boolean;
    };
  };
  required?: string[];
}

interface ParamSchemaDisplayProps {
  schema: ParamSchema;
  values: Record<string, any>;
  className?: string;
  emptyMessage?: string;
}

/**
 * Read-only component that displays parameter values based on a schema
 */
export default function ParamSchemaDisplay({
  schema,
  values,
  className = "",
  emptyMessage = "No parameters configured",
}: ParamSchemaDisplayProps) {
  const properties = schema.properties || {};
  const requiredFields = schema.required || [];
  const paramEntries = Object.entries(properties);

  // Filter to only show parameters that have values
  const populatedParams = paramEntries.filter(([key]) => {
    const value = values[key];
    return value !== undefined && value !== null && value !== "";
  });

  if (populatedParams.length === 0) {
    return (
      <div className="p-4 bg-gray-50 rounded-lg text-center text-sm text-gray-600">
        {emptyMessage}
      </div>
    );
  }

  /**
   * Check if a field is required
   */
  const isRequired = (key: string): boolean => {
    return requiredFields.includes(key);
  };

  /**
   * Format value for display based on its type
   * Returns both the formatted value and whether it should be displayed inline
   */
  const formatValue = (
    value: any,
    type?: string,
  ): { element: React.JSX.Element; isInline: boolean } => {
    if (value === undefined || value === null) {
      return {
        element: (
          <span className="bg-gray-100 text-gray-500 italic px-2 py-1 rounded">
            Not set
          </span>
        ),
        isInline: true,
      };
    }

    switch (type) {
      case "boolean":
        return {
          element: (
            <span
              className={`inline-flex items-center px-2.5 py-1 rounded text-sm font-medium ${
                value
                  ? "bg-green-50 text-green-700 border border-green-200"
                  : "bg-gray-100 text-gray-700 border border-gray-300"
              }`}
            >
              {value ? "✓ Enabled" : "✗ Disabled"}
            </span>
          ),
          isInline: true,
        };

      case "array":
        if (Array.isArray(value)) {
          if (value.length === 0) {
            return {
              element: (
                <span className="bg-gray-100 text-gray-500 italic px-2 py-1 rounded">
                  Empty array
                </span>
              ),
              isInline: true,
            };
          }
          return {
            element: (
              <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 space-y-1">
                {value.map((item, idx) => (
                  <div
                    key={idx}
                    className="flex items-center gap-2 text-sm text-gray-800"
                  >
                    <span className="text-blue-400">•</span>
                    <span className="font-mono">{JSON.stringify(item)}</span>
                  </div>
                ))}
              </div>
            ),
            isInline: false,
          };
        }
        // Fallback for non-array values
        return {
          element: (
            <pre className="bg-blue-50 border border-blue-200 px-3 py-2 rounded text-xs font-mono overflow-x-auto">
              {JSON.stringify(value, null, 2)}
            </pre>
          ),
          isInline: false,
        };

      case "object":
        if (typeof value === "object" && !Array.isArray(value)) {
          const entries = Object.entries(value);
          if (entries.length === 0) {
            return {
              element: (
                <span className="bg-gray-100 text-gray-500 italic px-2 py-1 rounded">
                  Empty object
                </span>
              ),
              isInline: true,
            };
          }
          return {
            element: (
              <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 space-y-2">
                {entries.map(([k, v]) => (
                  <div key={k} className="flex gap-2">
                    <span className="font-mono text-xs text-amber-900 font-semibold min-w-[100px]">
                      {k}:
                    </span>
                    <span className="font-mono text-xs text-gray-900">
                      {JSON.stringify(v)}
                    </span>
                  </div>
                ))}
              </div>
            ),
            isInline: false,
          };
        }
        // Fallback for non-object values
        return {
          element: (
            <pre className="bg-amber-50 border border-amber-200 px-3 py-2 rounded text-xs font-mono overflow-x-auto">
              {JSON.stringify(value, null, 2)}
            </pre>
          ),
          isInline: false,
        };

      case "number":
      case "integer":
        return {
          element: (
            <span className="bg-indigo-50 text-indigo-900 font-mono text-sm font-medium px-2 py-1 rounded border border-indigo-200">
              {value}
            </span>
          ),
          isInline: true,
        };

      default:
        // String or unknown type
        if (typeof value === "string") {
          // Check if it looks like a template/expression
          if (value.includes("{{") || value.includes("${")) {
            return {
              element: (
                <code className="bg-purple-50 border border-purple-200 text-purple-800 px-2 py-1 rounded text-sm font-mono">
                  {value}
                </code>
              ),
              isInline: true,
            };
          }
          // Regular string - check length for inline vs block
          const isShort = value.length < 60;
          return {
            element: (
              <span className="bg-slate-50 border border-slate-200 text-gray-900 px-2 py-1 rounded text-sm">
                {value}
              </span>
            ),
            isInline: isShort,
          };
        }
        // Fallback for complex types
        return {
          element: (
            <pre className="bg-gray-50 border border-gray-200 px-3 py-2 rounded text-xs font-mono overflow-x-auto">
              {JSON.stringify(value, null, 2)}
            </pre>
          ),
          isInline: false,
        };
    }
  };

  return (
    <div
      className={`bg-white border border-gray-300 rounded-lg p-4 shadow-sm ${className}`}
    >
      <div className="space-y-4">
        {populatedParams.map(([key, param]) => {
          const value = values[key];
          const type = param?.type || "string";
          const { element: valueElement, isInline } = formatValue(value, type);

          return (
            <div
              key={key}
              className="border-b border-gray-200 pb-4 last:border-0 last:pb-0"
            >
              {isInline ? (
                // Inline layout for small values
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="font-mono font-semibold text-sm text-gray-900">
                        {key}
                      </span>
                      <span className="text-xs px-2 py-0.5 bg-blue-50 text-blue-700 rounded font-medium">
                        {type}
                      </span>
                      {isRequired(key) && (
                        <span className="text-xs px-2 py-0.5 bg-red-50 text-red-700 rounded font-medium">
                          Required
                        </span>
                      )}
                      {param?.secret && (
                        <span className="text-xs px-2 py-0.5 bg-yellow-50 text-yellow-700 rounded font-medium">
                          Secret
                        </span>
                      )}
                    </div>
                    {param?.description && (
                      <p className="text-xs text-gray-600">
                        {param.description}
                      </p>
                    )}
                  </div>
                  <div className="flex items-start">{valueElement}</div>
                </div>
              ) : (
                // Block layout for large values
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <span className="font-mono font-semibold text-sm text-gray-900">
                      {key}
                    </span>
                    <span className="text-xs px-2 py-0.5 bg-blue-50 text-blue-700 rounded font-medium">
                      {type}
                    </span>
                    {isRequired(key) && (
                      <span className="text-xs px-2 py-0.5 bg-red-50 text-red-700 rounded font-medium">
                        Required
                      </span>
                    )}
                    {param?.secret && (
                      <span className="text-xs px-2 py-0.5 bg-yellow-50 text-yellow-700 rounded font-medium">
                        Secret
                      </span>
                    )}
                  </div>
                  {param?.description && (
                    <p className="text-xs text-gray-600 mb-2">
                      {param.description}
                    </p>
                  )}
                  <div>{valueElement}</div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/**
 * Compact variant for smaller displays - just shows key-value pairs
 */
export function ParamSchemaDisplayCompact({
  schema,
  values,
  className = "",
}: ParamSchemaDisplayProps) {
  const properties = schema.properties || {};
  const paramEntries = Object.entries(properties);
  const populatedParams = paramEntries.filter(([key]) => {
    const value = values[key];
    return value !== undefined && value !== null && value !== "";
  });

  if (populatedParams.length === 0) {
    return (
      <div className="text-sm text-gray-500 italic">No parameters set</div>
    );
  }

  return (
    <dl className={`grid grid-cols-1 gap-2 ${className}`}>
      {populatedParams.map(([key, param]) => {
        const value = values[key];
        const type = param?.type || "string";

        let displayValue: string;
        if (type === "boolean") {
          displayValue = value ? "Yes" : "No";
        } else if (type === "array" || type === "object") {
          displayValue = JSON.stringify(value);
        } else if (param?.secret && value) {
          displayValue = "••••••••";
        } else {
          displayValue = String(value);
        }

        return (
          <div key={key} className="flex gap-2">
            <dt className="font-mono text-xs text-gray-600 font-semibold min-w-[120px]">
              {key}:
            </dt>
            <dd className="font-mono text-xs text-gray-900 break-all">
              {displayValue}
            </dd>
          </div>
        );
      })}
    </dl>
  );
}
