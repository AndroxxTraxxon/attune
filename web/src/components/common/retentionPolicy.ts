export type RetentionPolicy = "versions" | "days" | "hours" | "minutes";

export function formatRetention(
  policy?: RetentionPolicy | null,
  limit?: number | null,
  fallback = "System default",
) {
  if (!policy || !limit) return fallback;
  return `${limit} ${policy}`;
}
