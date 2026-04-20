import { useState } from "react";
import { X } from "lucide-react";
import {
  type WorkQueueItemResponse,
} from "@/api/queues";
import {
  useEnqueueQueueItem,
  useUpdateQueueItem,
} from "@/hooks/useQueues";
import {
  parseJsonObject,
  parseJsonValue,
  prettyJson,
} from "./queueUtils";

interface QueueItemModalProps {
  queueRef: string;
  item?: WorkQueueItemResponse | null;
  onClose: () => void;
}

function getErrorMessage(error: unknown, fallback: string): string {
  const maybeAxios = error as { response?: { data?: { message?: string } } };
  return maybeAxios.response?.data?.message ||
    (error instanceof Error ? error.message : fallback);
}

export default function QueueItemModal({
  queueRef,
  item,
  onClose,
}: QueueItemModalProps) {
  const enqueueItem = useEnqueueQueueItem();
  const updateItem = useUpdateQueueItem();
  const isEditing = !!item;

  const [itemKey, setItemKey] = useState(item?.item_key ?? "");
  const [priority, setPriority] = useState(() => item?.priority ?? 0);
  const [enqueueSource, setEnqueueSource] = useState(
    () => item?.enqueue_source ?? "api",
  );
  const [payload, setPayload] = useState(() => prettyJson(item?.payload, {}));
  const [metadata, setMetadata] = useState(() => prettyJson(item?.metadata, {}));
  const [error, setError] = useState<string | null>(null);

  const isSubmitting = enqueueItem.isPending || updateItem.isPending;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);

    let parsedPayload;
    let parsedMetadata;
    try {
      parsedPayload = parseJsonValue("Payload", payload);
      parsedMetadata = parseJsonObject("Metadata", metadata);
    } catch (parseError) {
      setError(
        parseError instanceof Error
          ? parseError.message
          : "Invalid queue item JSON",
      );
      return;
    }

    try {
      if (item) {
        await updateItem.mutateAsync({
          ref: queueRef,
          itemId: item.id,
          data: {
            item_key: itemKey.trim()
              ? { op: "set", value: itemKey.trim() }
              : { op: "clear" },
            priority,
            payload: parsedPayload,
            metadata: parsedMetadata,
          },
        });
      } else {
        await enqueueItem.mutateAsync({
          ref: queueRef,
          data: {
            item_key: itemKey.trim() || null,
            priority,
            payload: parsedPayload,
            metadata: parsedMetadata,
            enqueue_source: enqueueSource.trim() || "api",
          },
        });
      }
      onClose();
    } catch (submitError) {
      setError(
        getErrorMessage(
          submitError,
          isEditing ? "Failed to update queue item" : "Failed to enqueue item",
        ),
      );
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-3xl max-h-[90vh] overflow-y-auto rounded-lg bg-white shadow-xl">
        <div className="flex items-center justify-between border-b border-gray-200 p-6">
          <div>
            <h2 className="text-2xl font-bold text-gray-900">
              {isEditing ? `Edit Queue Item #${item?.id}` : "Add Queue Item"}
            </h2>
            <p className="mt-1 text-sm text-gray-500 font-mono">{queueRef}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600"
          >
            <X className="h-6 w-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4 p-6">
          {error && (
            <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
              {error}
            </div>
          )}

          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Item key
              </label>
              <input
                value={itemKey}
                onChange={(e) => setItemKey(e.target.value)}
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
                placeholder="order-123"
              />
              <p className="mt-1 text-xs text-gray-500">
                Optional deduplication key for pending updates.
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Priority
              </label>
              <input
                type="number"
                value={priority}
                onChange={(e) => setPriority(Number(e.target.value))}
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
              />
            </div>
          </div>

          {!isEditing && (
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Enqueue source
              </label>
              <input
                value={enqueueSource}
                onChange={(e) => setEnqueueSource(e.target.value)}
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm"
                placeholder="api"
              />
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Payload (JSON)
            </label>
            <textarea
              value={payload}
              onChange={(e) => setPayload(e.target.value)}
              rows={10}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Metadata (JSON object)
            </label>
            <textarea
              value={metadata}
              onChange={(e) => setMetadata(e.target.value)}
              rows={6}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm"
            />
          </div>

          <div className="flex items-center justify-end gap-3 border-t border-gray-200 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg bg-gray-100 px-4 py-2 text-gray-700 hover:bg-gray-200 transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="rounded-lg bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isSubmitting
                ? "Saving..."
                : isEditing
                  ? "Save Item"
                  : "Enqueue Item"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
