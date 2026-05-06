import { useQuery } from "@tanstack/react-query";

import { WorkersService } from "@/api/workers";

export function useWorkers(params?: {
  page?: number;
  pageSize?: number;
  enabled?: boolean;
}) {
  return useQuery({
    queryKey: ["workers", params],
    queryFn: async () =>
      WorkersService.listWorkers({
        page: params?.page || 1,
        pageSize: params?.pageSize || 100,
      }),
    enabled: params?.enabled ?? true,
    staleTime: 30000,
  });
}
