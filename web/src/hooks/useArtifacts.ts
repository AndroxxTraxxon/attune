import { useQuery } from "@tanstack/react-query";
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

export type OwnerType = "system" | "pack" | "action" | "sensor" | "rule";

export type RetentionPolicyType = "versions" | "days" | "hours" | "minutes";

export interface ArtifactSummary {
  id: number;
  ref: string;
  type: ArtifactType;
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
  content_type: string | null;
  size_bytes: number | null;
  created_by: string | null;
  created: string;
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
    staleTime: isRunning ? 3000 : 10000,
    refetchInterval: isRunning ? 3000 : 10000,
  });
}

/**
 * Fetch a single artifact by ID (includes data field for progress artifacts).
 */
export function useArtifact(id: number | undefined) {
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
    staleTime: 3000,
    refetchInterval: 3000,
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
