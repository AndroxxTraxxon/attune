function escapeRegExp(value: string): string {
  return value.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
}

export function matchesRefFilter(
  actualRef: string | null | undefined,
  filterRef: string | undefined,
): boolean {
  if (!filterRef) return true;
  if (!actualRef) return false;

  if (filterRef.includes("*")) {
    const pattern = filterRef.split("*").map(escapeRegExp).join(".*");
    return new RegExp(`^${pattern}$`).test(actualRef);
  }

  return actualRef === filterRef;
}

export function packPrefix(ref: string | null | undefined): string | undefined {
  if (!ref) return undefined;
  const [pack] = ref.split(".");
  return pack || undefined;
}
