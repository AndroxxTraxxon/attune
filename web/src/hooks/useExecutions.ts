import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { ExecutionsService } from "@/api";
import type { ExecutionStatus } from "@/api";

interface ExecutionsQueryParams {
  page?: number;
  pageSize?: number;
  status?: ExecutionStatus;
  actionRef?: string;
  packName?: string;
  ruleRef?: string;
  triggerRef?: string;
  executor?: number;
}

export function useExecutions(params?: ExecutionsQueryParams) {
  // Check if any filters are applied
  const hasFilters =
    params?.status ||
    params?.actionRef ||
    params?.packName ||
    params?.ruleRef ||
    params?.triggerRef ||
    params?.executor;

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
