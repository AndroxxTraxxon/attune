import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { usePacks } from "@/hooks/usePacks";
import { useTriggers, useTrigger } from "@/hooks/useTriggers";
import { useActions, useAction } from "@/hooks/useActions";
import { useCreateRule, useUpdateRule } from "@/hooks/useRules";
import ParamSchemaForm, {
  validateParamSchema,
  type ParamSchema,
} from "@/components/common/ParamSchemaForm";
import type { RuleResponse } from "@/types/api";
import { labelToRef, extractLocalRef, combineRefs } from "@/lib/format-utils";

interface RuleFormProps {
  rule?: RuleResponse;
  onSuccess?: () => void;
  onCancel?: () => void;
}

export default function RuleForm({ rule, onSuccess, onCancel }: RuleFormProps) {
  const navigate = useNavigate();
  const isEditing = !!rule;

  // Form state
  const [packId, setPackId] = useState<number>(rule?.pack || 0);
  const [localRef, setLocalRef] = useState(
    rule?.ref ? extractLocalRef(rule.ref) : "",
  );
  const [label, setLabel] = useState(rule?.label || "");
  const [description, setDescription] = useState(rule?.description || "");
  const [triggerId, setTriggerId] = useState<number>(rule?.trigger || 0);
  const [actionId, setActionId] = useState<number>(rule?.action || 0);
  const [conditions, setConditions] = useState(
    rule?.conditions ? JSON.stringify(rule.conditions, null, 2) : "",
  );
  const [triggerParameters, setTriggerParameters] = useState<
    Record<string, any>
  >(rule?.trigger_params || {});
  const [actionParameters, setActionParameters] = useState<Record<string, any>>(
    rule?.action_params || {},
  );
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [triggerParamErrors, setTriggerParamErrors] = useState<
    Record<string, string>
  >({});
  const [actionParamErrors, setActionParamErrors] = useState<
    Record<string, string>
  >({});

  // Data fetching
  const { data: packsData } = usePacks({ pageSize: 1000 });
  const packs = packsData?.data || [];

  const selectedPack = packs.find((p) => p.id === packId);

  // Fetch ALL triggers and actions from all packs, not just the selected pack
  // This allows rules in ad-hoc packs to reference triggers/actions from other packs
  const { data: triggersData } = useTriggers({ pageSize: 1000 });
  const { data: actionsData } = useActions({ pageSize: 1000 });

  const triggers = triggersData?.data || [];
  const actions = actionsData?.data || [];

  // Get selected trigger and action refs for detail fetching
  const selectedTriggerSummary = triggers.find((t) => t.id === triggerId);
  const selectedActionSummary = actions.find((a: any) => a.id === actionId);

  // Fetch full trigger details (including param_schema) when a trigger is selected
  const { data: triggerDetailsData } = useTrigger(
    selectedTriggerSummary?.ref || "",
  );
  const selectedTrigger = triggerDetailsData?.data;

  // Fetch full action details (including param_schema) when an action is selected
  const { data: actionDetailsData } = useAction(
    selectedActionSummary?.ref || "",
  );
  const selectedAction = actionDetailsData?.data;

  // Extract param schemas from full details
  const triggerParamSchema: ParamSchema =
    ((selectedTrigger as any)?.param_schema as ParamSchema) || {};
  const actionParamSchema: ParamSchema =
    ((selectedAction as any)?.param_schema as ParamSchema) || {};

  // Mutations
  const createRule = useCreateRule();
  const updateRule = useUpdateRule();

  // Reset triggers, actions, and parameters when pack changes
  useEffect(() => {
    if (!isEditing) {
      setTriggerId(0);
      setActionId(0);
      setTriggerParameters({});
      setActionParameters({});
    }
  }, [packId, isEditing]);

  // Reset trigger parameters when trigger changes
  useEffect(() => {
    if (!isEditing) {
      setTriggerParameters({});
    }
  }, [triggerId, isEditing]);

  // Reset action parameters when action changes
  useEffect(() => {
    if (!isEditing) {
      setActionParameters({});
    }
  }, [actionId, isEditing]);

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!localRef.trim()) {
      newErrors.ref = "Reference is required";
    }

    if (!label.trim()) {
      newErrors.label = "Label is required";
    }

    if (!description.trim()) {
      newErrors.description = "Description is required";
    }

    if (!packId) {
      newErrors.pack = "Pack is required";
    }

    if (!triggerId) {
      newErrors.trigger = "Trigger is required";
    }

    if (!actionId) {
      newErrors.action = "Action is required";
    }

    // Validate conditions JSON if provided
    if (conditions.trim()) {
      try {
        JSON.parse(conditions);
      } catch (e) {
        newErrors.conditions = "Invalid JSON format";
      }
    }

    // Validate trigger parameters (allow templates in rule context)
    const triggerErrors = validateParamSchema(
      triggerParamSchema,
      triggerParameters,
      true,
    );
    setTriggerParamErrors(triggerErrors);

    // Validate action parameters (allow templates in rule context)
    const actionErrors = validateParamSchema(
      actionParamSchema,
      actionParameters,
      true,
    );
    setActionParamErrors(actionErrors);

    setErrors(newErrors);

    return (
      Object.keys(newErrors).length === 0 &&
      Object.keys(triggerErrors).length === 0 &&
      Object.keys(actionErrors).length === 0
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) {
      return;
    }

    // Get the selected pack, trigger, and action
    const selectedPackData = packs.find((p) => p.id === packId);

    // Combine pack ref and local ref to create full ref
    const fullRef = combineRefs(selectedPackData?.ref || "", localRef.trim());

    const formData: any = {
      pack_ref: selectedPackData?.ref || "",
      ref: fullRef,
      label: label.trim(),
      description: description.trim(),
      trigger_ref: selectedTrigger?.ref || "",
      action_ref: selectedAction?.ref || "",
      enabled,
    };

    // Only add optional fields if they have values
    if (conditions.trim()) {
      formData.conditions = JSON.parse(conditions);
    }

    // Add trigger parameters if any
    if (Object.keys(triggerParameters).length > 0) {
      formData.trigger_params = triggerParameters;
    }

    // Add action parameters if any
    if (Object.keys(actionParameters).length > 0) {
      formData.action_params = actionParameters;
    }

    try {
      if (isEditing && rule) {
        await updateRule.mutateAsync({ ref: rule.ref, data: formData });
      } else {
        const newRuleResponse = await createRule.mutateAsync(formData);
        if (!onSuccess) {
          navigate(`/rules/${newRuleResponse.data.ref}`);
        }
      }

      if (onSuccess) {
        onSuccess();
      }
    } catch (err) {
      console.error("Failed to save rule:", err);
      setErrors({
        submit: err instanceof Error ? err.message : "Failed to save rule",
      });
    }
  };

  const handleCancel = () => {
    if (onCancel) {
      onCancel();
    } else {
      navigate("/rules");
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {errors.submit && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4">
          <p className="text-sm text-red-600">{errors.submit}</p>
        </div>
      )}

      {/* Basic Information */}
      <div className="bg-white rounded-lg shadow p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">
          Basic Information
        </h3>

        {/* Pack Selection */}
        <div>
          <label
            htmlFor="pack"
            className="block text-sm font-medium text-gray-700 mb-1"
          >
            Pack <span className="text-red-500">*</span>
          </label>
          <select
            id="pack"
            value={packId}
            onChange={(e) => setPackId(Number(e.target.value))}
            disabled={isEditing}
            className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${
              errors.pack ? "border-red-500" : "border-gray-300"
            } ${isEditing ? "bg-gray-100 cursor-not-allowed" : ""}`}
          >
            <option value={0}>Select a pack...</option>
            {packs.map((pack: any) => (
              <option key={pack.id} value={pack.id}>
                {pack.label} ({pack.version})
              </option>
            ))}
          </select>
          {errors.pack && (
            <p className="mt-1 text-sm text-red-600">{errors.pack}</p>
          )}
        </div>

        {/* Label - MOVED FIRST */}
        <div>
          <label
            htmlFor="label"
            className="block text-sm font-medium text-gray-700 mb-1"
          >
            Label <span className="text-red-500">*</span>
          </label>
          <input
            type="text"
            id="label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            onBlur={() => {
              // Auto-populate localRef from label if localRef is empty and not editing
              if (!isEditing && !localRef.trim() && label.trim()) {
                setLocalRef(labelToRef(label));
              }
            }}
            placeholder="e.g., Notify on Error"
            className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${
              errors.label ? "border-red-500" : "border-gray-300"
            }`}
          />
          {errors.label && (
            <p className="mt-1 text-sm text-red-600">{errors.label}</p>
          )}
          <p className="mt-1 text-xs text-gray-500">
            Human-readable name for display
          </p>
        </div>

        {/* Reference - MOVED AFTER LABEL with Pack Prefix */}
        <div>
          <label
            htmlFor="ref"
            className="block text-sm font-medium text-gray-700 mb-1"
          >
            Reference <span className="text-red-500">*</span>
          </label>
          <div className="input-with-prefix">
            <span className={`prefix ${errors.ref ? "error" : ""}`}>
              {selectedPack?.ref || "pack"}.
            </span>
            <input
              type="text"
              id="ref"
              value={localRef}
              onChange={(e) => setLocalRef(e.target.value)}
              placeholder="e.g., notify_on_error"
              disabled={isEditing}
              className={errors.ref ? "error" : ""}
            />
          </div>
          {errors.ref && (
            <p className="mt-1 text-sm text-red-600">{errors.ref}</p>
          )}
          <p className="mt-1 text-xs text-gray-500">
            Local identifier within the pack. Auto-populated from label.
          </p>
        </div>

        {/* Description */}
        <div>
          <label
            htmlFor="description"
            className="block text-sm font-medium text-gray-700 mb-1"
          >
            Description <span className="text-red-500">*</span>
          </label>
          <textarea
            id="description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Describe what this rule does..."
            rows={3}
            className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${
              errors.description ? "border-red-500" : "border-gray-300"
            }`}
          />
          {errors.description && (
            <p className="mt-1 text-sm text-red-600">{errors.description}</p>
          )}
        </div>

        {/* Enabled Toggle */}
        <div className="flex items-center">
          <input
            type="checkbox"
            id="enabled"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
            className="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
          />
          <label htmlFor="enabled" className="ml-2 text-sm text-gray-700">
            Enable rule immediately
          </label>
        </div>
      </div>

      {/* Trigger Configuration */}
      <div className="bg-white rounded-lg shadow p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">
          Trigger Configuration
        </h3>

        {!packId ? (
          <p className="text-sm text-gray-500">
            Select a pack first to choose a trigger
          </p>
        ) : !triggers || triggers.length === 0 ? (
          <p className="text-sm text-gray-500">
            No triggers available in the system
          </p>
        ) : (
          <>
            {/* Trigger Selection */}
            <div>
              <label
                htmlFor="trigger"
                className="block text-sm font-medium text-gray-700 mb-1"
              >
                Trigger <span className="text-red-500">*</span>
              </label>
              <select
                id="trigger"
                value={triggerId}
                onChange={(e) => setTriggerId(Number(e.target.value))}
                disabled={isEditing}
                className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${
                  errors.trigger ? "border-red-500" : "border-gray-300"
                } ${isEditing ? "bg-gray-100 cursor-not-allowed" : ""}`}
              >
                <option value={0}>Select a trigger...</option>
                {triggers.map((trigger: any) => (
                  <option key={trigger.id} value={trigger.id}>
                    {trigger.ref} - {trigger.label}
                  </option>
                ))}
              </select>
              {errors.trigger && (
                <p className="mt-1 text-sm text-red-600">{errors.trigger}</p>
              )}
            </div>

            {/* Trigger Parameters - Dynamic Form */}
            {selectedTrigger && (
              <div>
                <h4 className="text-sm font-medium text-gray-700 mb-3">
                  Trigger Parameters
                </h4>
                <ParamSchemaForm
                  schema={triggerParamSchema}
                  values={triggerParameters}
                  onChange={setTriggerParameters}
                  errors={triggerParamErrors}
                  allowTemplates
                />
              </div>
            )}

            {/* Conditions (JSON) */}
            <div>
              <label
                htmlFor="conditions"
                className="block text-sm font-medium text-gray-700 mb-1"
              >
                Match Conditions (JSON)
              </label>
              <textarea
                id="conditions"
                value={conditions}
                onChange={(e) => setConditions(e.target.value)}
                placeholder={`{\n  "and": [\n    {"var": "payload.severity", ">=": 3},\n    {"var": "payload.status", "==": "error"}\n  ]\n}`}
                rows={8}
                className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-sm ${
                  errors.conditions ? "border-red-500" : "border-gray-300"
                }`}
              />
              {errors.conditions && (
                <p className="mt-1 text-sm text-red-600">{errors.conditions}</p>
              )}
              <p className="mt-1 text-xs text-gray-500">
                Optional. Leave empty to match all events from this trigger.
              </p>
            </div>
          </>
        )}
      </div>

      {/* Action Configuration */}
      <div className="bg-white rounded-lg shadow p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">
          Action Configuration
        </h3>

        {!packId ? (
          <p className="text-sm text-gray-500">
            Select a pack first to choose an action
          </p>
        ) : !actions || actions.length === 0 ? (
          <p className="text-sm text-gray-500">
            No actions available in the system
          </p>
        ) : (
          <>
            {/* Action Selection */}
            <div>
              <label
                htmlFor="action"
                className="block text-sm font-medium text-gray-700 mb-1"
              >
                Action <span className="text-red-500">*</span>
              </label>
              <select
                id="action"
                value={actionId}
                onChange={(e) => setActionId(Number(e.target.value))}
                disabled={isEditing}
                className={`w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${
                  errors.action ? "border-red-500" : "border-gray-300"
                } ${isEditing ? "bg-gray-100 cursor-not-allowed" : ""}`}
              >
                <option value={0}>Select an action...</option>
                {actions.map((action: any) => (
                  <option key={action.id} value={action.id}>
                    {action.ref} - {action.label}
                  </option>
                ))}
              </select>
              {errors.action && (
                <p className="mt-1 text-sm text-red-600">{errors.action}</p>
              )}
            </div>

            {/* Action Parameters - Dynamic Form */}
            {selectedAction && (
              <div>
                <h4 className="text-sm font-medium text-gray-700 mb-3">
                  Action Parameters
                </h4>
                <ParamSchemaForm
                  schema={actionParamSchema}
                  values={actionParameters}
                  onChange={setActionParameters}
                  errors={actionParamErrors}
                  allowTemplates
                />
              </div>
            )}
          </>
        )}
      </div>

      {/* Form Actions */}
      <div className="flex justify-end gap-3">
        <button
          type="button"
          onClick={handleCancel}
          className="px-4 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-50 transition-colors"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={createRule.isPending || updateRule.isPending}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {createRule.isPending || updateRule.isPending
            ? "Saving..."
            : isEditing
              ? "Update Rule"
              : "Create Rule"}
        </button>
      </div>
    </form>
  );
}
