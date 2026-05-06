import { extractProperties } from "@/components/common/ParamSchemaForm";

export type JsonObject = Record<string, unknown>;

export function isJsonObject(value: unknown): value is JsonObject {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

export function sortedSchemaEntries(schema: unknown) {
  return Object.entries(
    extractProperties(isJsonObject(schema) ? schema : null),
  ).sort(([aKey, a], [bKey, b]) => {
    const aPos = a.position ?? Number.MAX_SAFE_INTEGER;
    const bPos = b.position ?? Number.MAX_SAFE_INTEGER;
    return aPos === bPos ? aKey.localeCompare(bKey) : aPos - bPos;
  });
}

export function hasSchemaFields(schema: unknown): boolean {
  return sortedSchemaEntries(schema).length > 0;
}
