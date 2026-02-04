import { useState, useEffect } from "react";

/**
 * Standard JSON Schema format for parameters
 * Follows https://json-schema.org/draft/2020-12/schema
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

/**
 * Props for ParamSchemaForm component
 */
interface ParamSchemaFormProps {
  schema: ParamSchema;
  values: Record<string, any>;
  onChange: (values: Record<string, any>) => void;
  errors?: Record<string, string>;
  disabled?: boolean;
  className?: string;
}

/**
 * Dynamic form component that renders inputs based on a parameter schema.
 * Supports standard JSON Schema format with properties and required array.
 * Supports string, number, integer, boolean, array, object, and enum types.
 */
export default function ParamSchemaForm({
  schema,
  values,
  onChange,
  errors = {},
  disabled = false,
  className = "",
}: ParamSchemaFormProps) {
  const [localErrors, setLocalErrors] = useState<Record<string, string>>({});

  // Merge external and local errors
  const allErrors = { ...localErrors, ...errors };

  const properties = schema.properties || {};
  const requiredFields = schema.required || [];

  // Initialize values with defaults from schema
  useEffect(() => {
    const initialValues = Object.entries(properties).reduce(
      (acc, [key, param]) => {
        if (values[key] === undefined && param?.default !== undefined) {
          acc[key] = param.default;
        }
        return acc;
      },
      { ...values } as Record<string, any>,
    );

    // Only update if there are new defaults
    if (JSON.stringify(initialValues) !== JSON.stringify(values)) {
      onChange(initialValues);
    }
  }, [schema]); // Only run when schema changes

  /**
   * Handle input change for a specific field
   */
  const handleInputChange = (key: string, value: any) => {
    const newValues = { ...values, [key]: value };
    onChange(newValues);

    // Clear error for this field
    if (allErrors[key]) {
      setLocalErrors((prev) => {
        const updated = { ...prev };
        delete updated[key];
        return updated;
      });
    }
  };

  /**
   * Check if a field is required
   */
  const isRequired = (key: string): boolean => {
    return requiredFields.includes(key);
  };

  /**
   * Render input field based on parameter type
   */
  const renderInput = (key: string, param: any) => {
    const type = param?.type || "string";
    const value = values[key] ?? param?.default ?? "";
    const isDisabled = disabled;

    switch (type) {
      case "boolean":
        return (
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={!!value}
              onChange={(e) => handleInputChange(key, e.target.checked)}
              disabled={isDisabled}
              className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
            />
            <span className="text-sm text-gray-700">
              {param?.description || "Enable"}
            </span>
          </label>
        );

      case "number":
      case "integer":
        return (
          <input
            type="number"
            value={value}
            onChange={(e) =>
              handleInputChange(
                key,
                type === "integer"
                  ? parseInt(e.target.value) || 0
                  : parseFloat(e.target.value) || 0,
              )
            }
            disabled={isDisabled}
            className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-gray-100 disabled:cursor-not-allowed"
            placeholder={param?.description}
            step={type === "integer" ? "1" : "any"}
            min={param?.minimum}
            max={param?.maximum}
          />
        );

      case "array":
        return (
          <textarea
            value={
              Array.isArray(value) ? JSON.stringify(value, null, 2) : value
            }
            onChange={(e) => {
              try {
                const parsed = JSON.parse(e.target.value);
                handleInputChange(key, parsed);
              } catch {
                // Allow intermediate invalid JSON while typing
                handleInputChange(key, e.target.value);
              }
            }}
            onBlur={() => {
              // Validate on blur
              try {
                if (typeof value === "string") {
                  const parsed = JSON.parse(value);
                  handleInputChange(key, parsed);
                }
              } catch {
                // Invalid JSON - will be caught by validation
              }
            }}
            disabled={isDisabled}
            rows={4}
            className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm disabled:bg-gray-100 disabled:cursor-not-allowed"
            placeholder='["item1", "item2"]'
          />
        );

      case "object":
        return (
          <textarea
            value={
              typeof value === "object" && value !== null
                ? JSON.stringify(value, null, 2)
                : value
            }
            onChange={(e) => {
              try {
                const parsed = JSON.parse(e.target.value);
                handleInputChange(key, parsed);
              } catch {
                // Allow intermediate invalid JSON while typing
                handleInputChange(key, e.target.value);
              }
            }}
            onBlur={() => {
              // Validate on blur
              try {
                if (typeof value === "string") {
                  const parsed = JSON.parse(value);
                  handleInputChange(key, parsed);
                }
              } catch {
                // Invalid JSON - will be caught by validation
              }
            }}
            disabled={isDisabled}
            rows={4}
            className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm disabled:bg-gray-100 disabled:cursor-not-allowed"
            placeholder='{"key": "value"}'
          />
        );

      default:
        // String type - check for enum
        if (param?.enum && param.enum.length > 0) {
          return (
            <select
              value={value}
              onChange={(e) => handleInputChange(key, e.target.value)}
              disabled={isDisabled}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-gray-100 disabled:cursor-not-allowed"
            >
              <option value="">Select...</option>
              {param.enum.map((option: any) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          );
        }

        // Default to text input
        return (
          <input
            type={param?.secret ? "password" : "text"}
            value={value}
            onChange={(e) => handleInputChange(key, e.target.value)}
            disabled={isDisabled}
            className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-gray-100 disabled:cursor-not-allowed"
            placeholder={param?.description}
            minLength={param?.minLength}
            maxLength={param?.maxLength}
          />
        );
    }
  };

  const paramEntries = Object.entries(properties);

  if (paramEntries.length === 0) {
    return (
      <div className="p-4 bg-gray-50 rounded-lg text-center text-sm text-gray-600">
        No parameters required
      </div>
    );
  }

  return (
    <div className={`space-y-4 ${className}`}>
      {paramEntries.map(([key, param]) => (
        <div key={key}>
          <label className="block mb-2">
            <div className="flex items-center gap-2 mb-1">
              <span className="font-mono font-semibold text-sm">{key}</span>
              {isRequired(key) && (
                <span className="text-xs px-2 py-0.5 bg-red-100 text-red-700 rounded">
                  Required
                </span>
              )}
              <span className="text-xs px-2 py-0.5 bg-gray-100 text-gray-700 rounded">
                {param?.type || "string"}
              </span>
              {param?.secret && (
                <span className="text-xs px-2 py-0.5 bg-yellow-100 text-yellow-700 rounded">
                  Secret
                </span>
              )}
            </div>
            {param?.description && param?.type !== "boolean" && (
              <p className="text-xs text-gray-600 mb-2">{param.description}</p>
            )}
          </label>
          {renderInput(key, param)}
          {allErrors[key] && (
            <p className="text-xs text-red-600 mt-1">{allErrors[key]}</p>
          )}
          {param?.default !== undefined &&
            !values[key] &&
            values[key] !== param.default && (
              <p className="text-xs text-gray-500 mt-1">
                Default: {JSON.stringify(param.default)}
              </p>
            )}
        </div>
      ))}
    </div>
  );
}

/**
 * Utility function to validate parameter values against a schema
 * Supports standard JSON Schema format
 */
export function validateParamSchema(
  schema: ParamSchema,
  values: Record<string, any>,
): Record<string, string> {
  const errors: Record<string, string> = {};
  const properties = schema.properties || {};
  const requiredFields = schema.required || [];

  // Check required fields
  requiredFields.forEach((key) => {
    const value = values[key];
    if (value === undefined || value === null || value === "") {
      errors[key] = "This field is required";
    }
  });

  // Type-specific validation
  Object.entries(properties).forEach(([key, param]) => {
    const value = values[key];

    // Skip if no value and not required
    if (
      (value === undefined || value === null || value === "") &&
      !requiredFields.includes(key)
    ) {
      return;
    }

    const type = param?.type || "string";

    switch (type) {
      case "number":
      case "integer":
        if (typeof value !== "number" && isNaN(Number(value))) {
          errors[key] = `Must be a valid ${type}`;
        } else {
          const numValue = typeof value === "number" ? value : Number(value);
          if (param?.minimum !== undefined && numValue < param.minimum) {
            errors[key] = `Must be at least ${param.minimum}`;
          }
          if (param?.maximum !== undefined && numValue > param.maximum) {
            errors[key] = `Must be at most ${param.maximum}`;
          }
        }
        break;

      case "array":
        if (!Array.isArray(value)) {
          try {
            JSON.parse(value);
          } catch {
            errors[key] = "Must be a valid array (JSON format)";
          }
        }
        break;

      case "object":
        if (typeof value !== "object" || Array.isArray(value)) {
          try {
            const parsed = JSON.parse(value);
            if (typeof parsed !== "object" || Array.isArray(parsed)) {
              errors[key] = "Must be a valid object (JSON format)";
            }
          } catch {
            errors[key] = "Must be a valid object (JSON format)";
          }
        }
        break;

      case "string":
        if (typeof value === "string") {
          if (
            param?.minLength !== undefined &&
            value.length < param.minLength
          ) {
            errors[key] = `Must be at least ${param.minLength} characters`;
          }
          if (
            param?.maxLength !== undefined &&
            value.length > param.maxLength
          ) {
            errors[key] = `Must be at most ${param.maxLength} characters`;
          }
        }
        break;
    }

    // Enum validation
    if (param?.enum && param.enum.length > 0) {
      if (!param.enum.includes(value)) {
        errors[key] = `Must be one of: ${param.enum.join(", ")}`;
      }
    }
  });

  return errors;
}
