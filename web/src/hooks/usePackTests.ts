import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { PacksService, ApiError } from "@/api";

// Fetch test history for a pack
export function usePackTestHistory(
  packRef: string,
  params?: { page?: number; pageSize?: number },
) {
  return useQuery({
    queryKey: ["pack-tests", packRef, params],
    queryFn: async () => {
      return PacksService.getPackTestHistory({
        ref: packRef,
        page: params?.page,
        pageSize: params?.pageSize,
      });
    },
    enabled: !!packRef,
    staleTime: 30000, // 30 seconds
  });
}

// Fetch latest test result for a pack
export function usePackLatestTest(packRef: string) {
  return useQuery({
    queryKey: ["pack-tests", packRef, "latest"],
    queryFn: async () => {
      try {
        return await PacksService.getPackLatestTest({ ref: packRef });
      } catch (error) {
        if (error instanceof ApiError && error.status === 404) {
          return { data: null };
        }
        throw error;
      }
    },
    enabled: !!packRef,
    staleTime: 30000,
  });
}

// Execute pack tests
export function useExecutePackTests() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (packRef: string) => {
      return PacksService.testPack({ ref: packRef });
    },
    onSuccess: (_, packRef) => {
      // Invalidate test history and latest test queries
      queryClient.invalidateQueries({ queryKey: ["pack-tests", packRef] });
    },
  });
}

// Install pack from remote source
export function useInstallPack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      source,
      refSpec,
      skipTests = false,
      skipDeps = false,
    }: {
      source: string;
      refSpec?: string;
      skipTests?: boolean;
      skipDeps?: boolean;
    }) => {
      return PacksService.installPack({
        requestBody: {
          source,
          ref_spec: refSpec,
          skip_tests: skipTests,
          skip_deps: skipDeps,
        },
      });
    },
    onSuccess: (data) => {
      // Invalidate packs list and test queries
      queryClient.invalidateQueries({ queryKey: ["packs"] });
      if (data.data.pack.ref) {
        queryClient.invalidateQueries({
          queryKey: ["pack-tests", data.data.pack.ref],
        });
      }
    },
  });
}
