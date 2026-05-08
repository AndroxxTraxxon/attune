import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { OpenAPI } from "@/api/core/OpenAPI";
import { request as __request } from "@/api/core/request";

// Artifact types matching the backend ArtifactType enum
export type ArtifactType =
  | "file_binary"
  | "file_datatable"
  | "file_image"
  | "file_text"
  | "other"
  | "progress"
  | "url";

export type ArtifactVisibility = "public" | "private";

export type OwnerType = "system" | "pack" | "action" | "sensor" | "rule";

export type RetentionPolicyType = "versions" | "days" | "hours" | "minutes";

export interface ArtifactSummary {
  id: number;
  ref: string;
  type: ArtifactType;
  visibility: ArtifactVisibility;
  name: string | null;
  content_type: string | null;
  size_bytes: number | null;
  execution: number | null;
  scope: OwnerType;
  owner: string;
  created: string;
  updated: string;
}

export interface ArtifactResponse {
  id: number;
  ref: string;
  scope: OwnerType;
  owner: string;
  type: ArtifactType;
  visibility: ArtifactVisibility;
  retention_policy: RetentionPolicyType;
  retention_limit: number;
  name: string | null;
  description: string | null;
  content_type: string | null;
  size_bytes: number | null;
  execution: number | null;
  data?: unknown;
  created: string;
  updated: string;
}

export interface ArtifactVersionSummary {
  id: number;
  version: number;
  execution: number | null;
  content_type: string | null;
  size_bytes: number | null;
  created_by: string | null;
  created: string;
}

// ============================================================================
// Search / List params
// ============================================================================

export interface ArtifactsListParams {
  page?: number;
  perPage?: number;
  scope?: OwnerType;
  owner?: string;
  type?: ArtifactType;
  visibility?: ArtifactVisibility;
  execution?: number;
  name?: string;
}

// ============================================================================
// Paginated list response shape
// ============================================================================

export interface PaginatedArtifacts {
  items: ArtifactSummary[];
  pagination: {
    page: number;
    page_size: number;
    total_items: number;
    total_pages: number;
  };
}

// ============================================================================
// Hooks
// ============================================================================

/**
 * Fetch a paginated, filterable list of all artifacts.
 *
 * Uses GET /api/v1/artifacts with query params for server-side filtering.
 */
export function useArtifactsList(params: ArtifactsListParams = {}) {
  return useQuery({
    queryKey: ["artifacts", "list", params],
    queryFn: async () => {
      const query: Record<string, string> = {};
      if (params.page) query.page = String(params.page);
      if (params.perPage) query.per_page = String(params.perPage);
      if (params.scope) query.scope = params.scope;
      if (params.owner) query.owner = params.owner;
      if (params.type) query.type = params.type;
      if (params.visibility) query.visibility = params.visibility;
      if (params.execution) query.execution = String(params.execution);
      if (params.name) query.name = params.name;

      const response = await __request<PaginatedArtifacts>(OpenAPI, {
        method: "GET",
        url: "/api/v1/artifacts",
        query,
      });
      return response;
    },
    staleTime: 10000,
    placeholderData: keepPreviousData,
  });
}

/**
 * Fetch all artifacts for a given execution ID.
 *
 * Uses the GET /api/v1/executions/{execution_id}/artifacts endpoint.
 */
export function useExecutionArtifacts(
  executionId: number | undefined,
  isRunning = false,
) {
  return useQuery({
    queryKey: ["artifacts", "execution", executionId],
    queryFn: async () => {
      const response = await __request<{ data: ArtifactSummary[] }>(OpenAPI, {
        method: "GET",
        url: "/api/v1/executions/{execution_id}/artifacts",
        path: {
          execution_id: executionId!,
        },
      });
      return response;
    },
    enabled: !!executionId,
    staleTime: isRunning ? 3000 : 30000,
    refetchInterval: isRunning ? 3000 : false,
  });
}

/**
 * Fetch a single artifact by ID (includes data field for progress artifacts).
 *
 * @param isRunning - When true, polls every 3s for live updates. When false,
 *   uses a longer stale time and disables automatic polling.
 */
export function useArtifact(id: number | undefined, isRunning = false) {
  return useQuery({
    queryKey: ["artifacts", id],
    queryFn: async () => {
      const response = await __request<{ data: ArtifactResponse }>(OpenAPI, {
        method: "GET",
        url: "/api/v1/artifacts/{id}",
        path: {
          id: id!,
        },
      });
      return response;
    },
    enabled: !!id,
    staleTime: isRunning ? 3000 : 30000,
    refetchInterval: isRunning ? 3000 : false,
  });
}

/**
 * Fetch versions for a given artifact ID.
 */
export function useArtifactVersions(artifactId: number | undefined) {
  return useQuery({
    queryKey: ["artifacts", artifactId, "versions"],
    queryFn: async () => {
      const response = await __request<{ data: ArtifactVersionSummary[] }>(
        OpenAPI,
        {
          method: "GET",
          url: "/api/v1/artifacts/{id}/versions",
          path: {
            id: artifactId!,
          },
        },
      );
      return response;
    },
    enabled: !!artifactId,
    staleTime: 10000,
  });
}
