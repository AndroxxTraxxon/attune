import {
  MUTABLE_PENDING_STATUSES,
  WorkQueueBatchMode,
  WorkQueueItemStatus,
  WorkQueueUpdateStrategy,
  type JsonValue,
} from "@/api/queues";

export function prettyJson(value: unknown, fallback: unknown = {}): string {
  return JSON.stringify(value ?? fallback, null, 2);
}

export function parseJsonObject(label: string, raw: string): Record<string, JsonValue> {
  if (!raw.trim()) {
    throw new Error(`${label} is required`);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error(`${label} must be valid JSON`);
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${label} must be a JSON object`);
  }

  return parsed as Record<string, JsonValue>;
}

export function parseJsonValue(label: string, raw: string): JsonValue {
  if (!raw.trim()) {
    throw new Error(`${label} is required`);
  }

  try {
    return JSON.parse(raw) as JsonValue;
  } catch {
    throw new Error(`${label} must be valid JSON`);
  }
}

export function formatDateTime(value?: string | null): string {
  if (!value) {
    return "—";
  }
  return new Date(value).toLocaleString();
}

export function formatJsonPreview(value: JsonValue, maxLength = 80): string {
  const text = JSON.stringify(value);
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength - 1)}…`;
}

export function isMutablePendingStatus(status: WorkQueueItemStatus): boolean {
  return MUTABLE_PENDING_STATUSES.some((candidate) => candidate === status);
}

export function getQueueSourceBadge(isAdhoc: boolean) {
  if (isAdhoc) {
    return {
      label: "API-managed",
      classes: "bg-blue-100 text-blue-800",
      description: "Editable in the UI and managed by the API.",
    };
  }

  return {
    label: "Pack-managed",
    classes: "bg-purple-100 text-purple-800",
    description: "Read-only in the UI. Update the pack queue definition files instead.",
  };
}

export function getStatusBadge(status: WorkQueueItemStatus) {
  const map: Record<WorkQueueItemStatus, { label: string; classes: string }> = {
    [WorkQueueItemStatus.QUEUED]: {
      label: "Queued",
      classes: "bg-blue-100 text-blue-800",
    },
    [WorkQueueItemStatus.LEASED]: {
      label: "Leased",
      classes: "bg-amber-100 text-amber-800",
    },
    [WorkQueueItemStatus.RETRY]: {
      label: "Retry",
      classes: "bg-orange-100 text-orange-800",
    },
    [WorkQueueItemStatus.COMPLETED]: {
      label: "Completed",
      classes: "bg-green-100 text-green-800",
    },
    [WorkQueueItemStatus.FAILED]: {
      label: "Failed",
      classes: "bg-red-100 text-red-800",
    },
    [WorkQueueItemStatus.SKIPPED]: {
      label: "Skipped",
      classes: "bg-gray-100 text-gray-800",
    },
    [WorkQueueItemStatus.CANCELLED]: {
      label: "Cancelled",
      classes: "bg-slate-100 text-slate-800",
    },
  };

  return map[status];
}

export function getUpdateStrategyLabel(strategy: WorkQueueUpdateStrategy): string {
  switch (strategy) {
    case WorkQueueUpdateStrategy.IMMUTABLE:
      return "Immutable";
    case WorkQueueUpdateStrategy.MERGE_PATCH:
      return "Merge Patch";
    case WorkQueueUpdateStrategy.REPLACE:
    default:
      return "Replace";
  }
}

export function getBatchModeLabel(mode: WorkQueueBatchMode): string {
  return mode === WorkQueueBatchMode.BATCH ? "Batch" : "Single";
}
