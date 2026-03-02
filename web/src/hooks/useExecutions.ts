import {
  useQuery,
  useMutation,
  useQueryClient,
  keepPreviousData,
} from "@tanstack/react-query";
import { ExecutionsService } from "@/api";
import type { ExecutionStatus } from "@/api";
import { OpenAPI } from "@/api/core/OpenAPI";
import { request as __request } from "@/api/core/request";

interface ExecutionsQueryParams {
  page?: number;
  pageSize?: number;
  status?: ExecutionStatus;
  actionRef?: string;
  packName?: string;
  ruleRef?: string;
  triggerRef?: string;
  executor?: number;
  topLevelOnly?: boolean;
}

export function useExecutions(params?: ExecutionsQueryParams) {
  // Check if any filters are applied
  const hasFilters =
    params?.status ||
    params?.actionRef ||
    params?.packName ||
    params?.ruleRef ||
    params?.triggerRef ||
    params?.executor ||
    params?.topLevelOnly;

  return useQuery({
    queryKey: ["executions", params],
    queryFn: async () => {
      const response = await ExecutionsService.listExecutions({
        page: params?.page,
        perPage: params?.pageSize,
        status: params?.status,
        actionRef: params?.actionRef,
        packName: params?.packName,
        ruleRef: params?.ruleRef,
        triggerRef: params?.triggerRef,
        executor: params?.executor,
        topLevelOnly: params?.topLevelOnly,
      });
      return response;
    },
    // Use shorter staleTime when filters are active to ensure fresh results
    // Use longer staleTime for unfiltered list since SSE handles real-time updates
    staleTime: hasFilters ? 5000 : 30000,
    // Refetch in background when filters change to get latest data
    refetchOnMount: hasFilters ? "always" : true,
    // Keep previous results visible while new data loads (prevents flash of empty state)
    placeholderData: keepPreviousData,
  });
}

export function useExecution(id: number) {
  return useQuery({
    queryKey: ["executions", id],
    queryFn: async () => {
      const response = await ExecutionsService.getExecution({ id });
      return response;
    },
    enabled: !!id,
    staleTime: 30000, // 30 seconds - SSE handles real-time updates
  });
}

/**
 * Fetch child executions (workflow tasks) for a given parent execution ID.
 *
 * Enabled only when `parentId` is provided. Polls every 5 seconds while any
 * child execution is still in a running/pending state so the UI stays current.
 */
/**
 * Request a manual execution of an action (or workflow).
 *
 * Calls POST /api/v1/executions/execute and returns the created execution,
 * including its `id` which callers can use to navigate to the detail page.
 */
export function useRequestExecution() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      actionRef,
      parameters,
    }: {
      actionRef: string;
      parameters?: Record<string, unknown>;
    }) => {
      const response = await __request(OpenAPI, {
        method: "POST",
        url: "/api/v1/executions/execute",
        body: {
          action_ref: actionRef,
          parameters: parameters ?? null,
        },
        mediaType: "application/json",
      });
      return response as {
        data: { id: number; status: string; action_ref: string };
      };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["executions"] });
    },
  });
}

export function useChildExecutions(parentId: number | undefined) {
  return useQuery({
    queryKey: ["executions", { parent: parentId }],
    queryFn: async () => {
      const response = await ExecutionsService.listExecutions({
        parent: parentId,
        perPage: 100,
      });
      return response;
    },
    enabled: !!parentId,
    staleTime: 5000,
    // Re-fetch periodically so in-progress tasks update
    refetchInterval: (query) => {
      const data = query.state.data;
      if (!data) return false;
      const hasActive = data.data.some(
        (e) =>
          e.status === "requested" ||
          e.status === "scheduling" ||
          e.status === "scheduled" ||
          e.status === "running",
      );
      return hasActive ? 5000 : false;
    },
  });
}
