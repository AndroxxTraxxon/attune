import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useActions } from "@/hooks/useActions";
import { useCreateQueue, useUpdateQueue } from "@/hooks/useQueues";
import {
  WorkQueueBatchMode,
  WorkQueueUpdateStrategy,
  type WorkQueueResponse,
} from "@/api/queues";
import {
  parseJsonObject,
  prettyJson,
} from "./queueUtils";

interface QueueFormProps {
  initialData?: WorkQueueResponse;
  isEditing?: boolean;
}

function getErrorMessage(error: unknown, fallback: string): string {
  const maybeAxios = error as { response?: { data?: { message?: string } } };
  return maybeAxios.response?.data?.message ||
    (error instanceof Error ? error.message : fallback);
}

export default function QueueForm({
  initialData,
  isEditing = false,
}: QueueFormProps) {
  const navigate = useNavigate();
  const createQueue = useCreateQueue();
  const updateQueue = useUpdateQueue();
  const { data: actionsData } = useActions({ page: 1, pageSize: 200 });

  const [ref, setRef] = useState(() => initialData?.ref ?? "");
  const [label, setLabel] = useState(() => initialData?.label ?? "");
  const [description, setDescription] = useState(
    () => initialData?.description ?? "",
  );
  const [dispatchActionRef, setDispatchActionRef] = useState(
    () => initialData?.dispatch_action_ref ?? "",
  );
  const [enabled, setEnabled] = useState(() => initialData?.enabled ?? true);
  const [defaultPriority, setDefaultPriority] = useState(
    () => initialData?.default_priority ?? 0,
  );
  const [allowPendingUpdate, setAllowPendingUpdate] = useState(
    () => initialData?.allow_pending_update ?? false,
  );
  const [updateStrategy, setUpdateStrategy] = useState<WorkQueueUpdateStrategy>(
    () => initialData?.update_strategy ?? WorkQueueUpdateStrategy.REPLACE,
  );
  const [batchMode, setBatchMode] = useState<WorkQueueBatchMode>(
    () => initialData?.batch_mode ?? WorkQueueBatchMode.SINGLE,
  );
  const [config, setConfig] = useState(() => prettyJson(initialData?.config));
  const [errors, setErrors] = useState<Record<string, string>>({});
  const isImmutableStrategy = updateStrategy === WorkQueueUpdateStrategy.IMMUTABLE;
  const effectiveAllowPendingUpdate = isImmutableStrategy ? false : allowPendingUpdate;

  const actions = actionsData?.data ?? [];
  const actionOptions =
    initialData?.dispatch_action_ref &&
    !actions.some((action) => action.ref === initialData.dispatch_action_ref)
      ? [
          {
            id: -1,
            ref: initialData.dispatch_action_ref,
            label: initialData.dispatch_action_ref,
          },
          ...actions,
        ]
      : actions;

  const isSubmitting = createQueue.isPending || updateQueue.isPending;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();

    const nextErrors: Record<string, string> = {};
    if (!ref.trim() && !isEditing) {
      nextErrors.ref = "Queue ref is required";
    }
    if (!label.trim()) {
      nextErrors.label = "Label is required";
    }
    if (!dispatchActionRef.trim()) {
      nextErrors.dispatch_action_ref = "Dispatch action is required";
    }

    let parsedConfig: ReturnType<typeof parseJsonObject> | undefined;
    try {
      parsedConfig = parseJsonObject("Config", config);
    } catch (error) {
      nextErrors.config =
        error instanceof Error ? error.message : "Config must be valid JSON";
    }

    if (Object.keys(nextErrors).length > 0) {
      setErrors(nextErrors);
      return;
    }

    try {
      if (isEditing && initialData) {
        await updateQueue.mutateAsync({
          ref: initialData.ref,
          data: {
            label: label.trim(),
            description: description.trim()
              ? { op: "set", value: description.trim() }
              : { op: "clear" },
            enabled,
            dispatch_action_ref: dispatchActionRef,
            default_priority: defaultPriority,
            allow_pending_update: effectiveAllowPendingUpdate,
            update_strategy: updateStrategy,
            batch_mode: batchMode,
            config: parsedConfig,
          },
        });
        navigate(`/queues/${encodeURIComponent(initialData.ref)}`);
        return;
      }

      const response = await createQueue.mutateAsync({
        ref: ref.trim(),
        label: label.trim(),
        description: description.trim() || null,
        enabled,
        dispatch_action_ref: dispatchActionRef,
        default_priority: defaultPriority,
        allow_pending_update: effectiveAllowPendingUpdate,
        update_strategy: updateStrategy,
        batch_mode: batchMode,
        config: parsedConfig,
      });
      navigate(`/queues/${encodeURIComponent(response.data.ref)}`);
    } catch (error) {
      setErrors({
        submit: getErrorMessage(error, "Failed to save queue"),
      });
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {errors.submit && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 text-sm text-red-700">
          {errors.submit}
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <div className="bg-white rounded-lg shadow p-6 space-y-4">
          <h2 className="text-lg font-semibold text-gray-900">Basics</h2>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Queue Ref
            </label>
            <input
              value={ref}
              onChange={(e) => setRef(e.target.value)}
              disabled={isEditing}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm disabled:bg-gray-100"
              placeholder="ops.manual_review"
            />
            <p className="mt-1 text-xs text-gray-500">
              Use a unique ref in <span className="font-mono">pack.name</span> format.
            </p>
            {errors.ref && <p className="mt-1 text-sm text-red-600">{errors.ref}</p>}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Label
            </label>
            <input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
              placeholder="Manual Review Queue"
            />
            {errors.label && <p className="mt-1 text-sm text-red-600">{errors.label}</p>}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Description
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={4}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
              placeholder="Describe what this queue dispatches and who uses it"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Dispatch Action
            </label>
            <select
              value={dispatchActionRef}
              onChange={(e) => setDispatchActionRef(e.target.value)}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="">Select an action</option>
              {actionOptions.map((action) => (
                <option key={action.ref} value={action.ref}>
                  {action.ref}
                  {action.label && action.label !== action.ref
                    ? ` — ${action.label}`
                    : ""}
                </option>
              ))}
            </select>
            {errors.dispatch_action_ref && (
              <p className="mt-1 text-sm text-red-600">{errors.dispatch_action_ref}</p>
            )}
          </div>

          <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-4">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="mt-1 h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
            />
            <span>
              <span className="block text-sm font-medium text-gray-900">
                Queue enabled
              </span>
              <span className="block text-sm text-gray-500">
                Disabled queues remain visible but should not accept new work through normal flows.
              </span>
            </span>
          </label>
        </div>

        <div className="bg-white rounded-lg shadow p-6 space-y-4">
          <h2 className="text-lg font-semibold text-gray-900">Dispatch behaviour</h2>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Default priority
            </label>
            <input
              type="number"
              value={defaultPriority}
              onChange={(e) => setDefaultPriority(Number(e.target.value))}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            />
          </div>

          <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-4">
            <input
              type="checkbox"
              checked={effectiveAllowPendingUpdate}
              onChange={(e) => setAllowPendingUpdate(e.target.checked)}
              disabled={isImmutableStrategy}
              className="mt-1 h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
            />
            <span>
              <span className="block text-sm font-medium text-gray-900">
                Allow pending item updates
              </span>
              <span className="block text-sm text-gray-500">
                When enabled, enqueue requests can update an existing queued or retry item with the same key.
              </span>
              {isImmutableStrategy && (
                <span className="mt-2 block text-sm text-amber-700">
                  Immutable queues always reject duplicate pending keys, so pending updates are turned off.
                </span>
              )}
            </span>
          </label>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Update strategy
            </label>
            <select
              value={updateStrategy}
              onChange={(e) =>
                setUpdateStrategy(e.target.value as WorkQueueUpdateStrategy)
              }
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            >
              <option value={WorkQueueUpdateStrategy.REPLACE}>Replace existing payload + metadata</option>
              <option value={WorkQueueUpdateStrategy.MERGE_PATCH}>Merge patch existing payload + metadata</option>
              <option value={WorkQueueUpdateStrategy.IMMUTABLE}>Reject duplicate pending item keys</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Batch mode
            </label>
            <select
              value={batchMode}
              onChange={(e) => setBatchMode(e.target.value as WorkQueueBatchMode)}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
            >
              <option value={WorkQueueBatchMode.SINGLE}>Single item dispatch</option>
              <option value={WorkQueueBatchMode.BATCH}>Batch dispatch</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Queue config (JSON)
            </label>
            <textarea
              value={config}
              onChange={(e) => setConfig(e.target.value)}
              rows={12}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm"
            />
            {errors.config && <p className="mt-1 text-sm text-red-600">{errors.config}</p>}
          </div>
        </div>
      </div>

      <div className="flex items-center justify-end gap-3">
        <button
          type="button"
          onClick={() => navigate(isEditing && initialData ? `/queues/${encodeURIComponent(initialData.ref)}` : "/queues")}
          className="px-4 py-2 rounded-lg bg-gray-100 text-gray-700 hover:bg-gray-200 transition-colors"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={isSubmitting}
          className="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isSubmitting ? "Saving..." : isEditing ? "Save Changes" : "Create Queue"}
        </button>
      </div>
    </form>
  );
}
