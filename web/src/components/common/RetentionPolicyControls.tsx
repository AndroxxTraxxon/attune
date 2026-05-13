import type { RetentionPolicy } from "@/components/common/retentionPolicy";

const RETENTION_POLICIES: Array<{ value: RetentionPolicy; label: string }> = [
  { value: "versions", label: "Versions" },
  { value: "days", label: "Days" },
  { value: "hours", label: "Hours" },
  { value: "minutes", label: "Minutes" },
];

interface RetentionPolicyControlsProps {
  title: string;
  description?: string;
  policy: RetentionPolicy | null;
  limit: number | null;
  onChange: (value: {
    policy: RetentionPolicy | null;
    limit: number | null;
  }) => void;
  inheritedLabel?: string;
}

export default function RetentionPolicyControls({
  title,
  description,
  policy,
  limit,
  onChange,
  inheritedLabel = "Inherit default",
}: RetentionPolicyControlsProps) {
  const enabled = Boolean(policy || limit);
  const effectivePolicy = policy ?? "versions";
  const effectiveLimit = limit ?? 5;

  return (
    <div className="rounded-lg border border-gray-200 p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-medium text-gray-900">{title}</div>
          {description && (
            <p className="mt-1 text-xs text-gray-500">{description}</p>
          )}
        </div>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => {
              onChange(
                event.target.checked
                  ? { policy: effectivePolicy, limit: effectiveLimit }
                  : { policy: null, limit: null },
              );
            }}
            className="rounded border-gray-300"
          />
          Override
        </label>
      </div>

      {enabled ? (
        <div className="mt-3 grid grid-cols-2 gap-3">
          <label className="text-xs text-gray-600">
            Policy
            <select
              value={effectivePolicy}
              onChange={(event) =>
                onChange({
                  policy: event.target.value as RetentionPolicy,
                  limit: effectiveLimit,
                })
              }
              className="mt-1 block w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm"
            >
              {RETENTION_POLICIES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-gray-600">
            Limit
            <input
              type="number"
              min={1}
              value={effectiveLimit}
              onChange={(event) =>
                onChange({
                  policy: effectivePolicy,
                  limit: Math.max(1, Number(event.target.value || 1)),
                })
              }
              className="mt-1 block w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm"
            />
          </label>
        </div>
      ) : (
        <p className="mt-3 text-xs text-gray-500">{inheritedLabel}</p>
      )}
    </div>
  );
}
