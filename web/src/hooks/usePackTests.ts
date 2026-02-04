import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

// Temporary types until API client is regenerated
interface PackTestResult {
  pack_ref: string;
  pack_version: string;
  execution_time: string;
  status: string;
  total_tests: number;
  passed: number;
  failed: number;
  skipped: number;
  pass_rate: number;
  duration_ms: number;
  test_suites: any[];
}

interface PackTestExecution {
  id: number;
  pack_id: number;
  pack_version: string;
  execution_time: string;
  trigger_reason: string;
  total_tests: number;
  passed: number;
  failed: number;
  skipped: number;
  pass_rate: number;
  duration_ms: number;
  result: PackTestResult;
  created: string;
}

interface PackTestHistoryResponse {
  data: {
    items: PackTestExecution[];
    meta: {
      page: number;
      page_size: number;
      total_items: number;
      total_pages: number;
    };
  };
}

interface PackTestLatestResponse {
  data: PackTestExecution | null;
}

// Fetch test history for a pack
export function usePackTestHistory(
  packRef: string,
  params?: { page?: number; pageSize?: number },
) {
  return useQuery({
    queryKey: ["pack-tests", packRef, params],
    queryFn: async (): Promise<PackTestHistoryResponse> => {
      const queryParams = new URLSearchParams();
      if (params?.page) queryParams.append("page", params.page.toString());
      if (params?.pageSize)
        queryParams.append("page_size", params.pageSize.toString());

      const token = localStorage.getItem("access_token");
      const response = await fetch(
        `http://localhost:8080/api/v1/packs/${packRef}/tests?${queryParams}`,
        {
          headers: {
            Authorization: `Bearer ${token}`,
          },
        },
      );

      if (!response.ok) {
        throw new Error(`Failed to fetch test history: ${response.statusText}`);
      }

      return response.json();
    },
    enabled: !!packRef,
    staleTime: 30000, // 30 seconds
  });
}

// Fetch latest test result for a pack
export function usePackLatestTest(packRef: string) {
  return useQuery({
    queryKey: ["pack-tests", packRef, "latest"],
    queryFn: async (): Promise<PackTestLatestResponse> => {
      const token = localStorage.getItem("access_token");
      const response = await fetch(
        `http://localhost:8080/api/v1/packs/${packRef}/tests/latest`,
        {
          headers: {
            Authorization: `Bearer ${token}`,
          },
        },
      );

      if (!response.ok) {
        if (response.status === 404) {
          return { data: null };
        }
        throw new Error(`Failed to fetch latest test: ${response.statusText}`);
      }

      return response.json();
    },
    enabled: !!packRef,
    staleTime: 30000,
  });
}

// Execute pack tests
export function useExecutePackTests() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (packRef: string): Promise<{ data: PackTestResult }> => {
      const token = localStorage.getItem("access_token");
      const response = await fetch(
        `http://localhost:8080/api/v1/packs/${packRef}/test`,
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${token}`,
            "Content-Type": "application/json",
          },
        },
      );

      if (!response.ok) {
        const error = await response.json().catch(() => ({}));
        throw new Error(
          error.error || `Failed to execute tests: ${response.statusText}`,
        );
      }

      return response.json();
    },
    onSuccess: (_, packRef) => {
      // Invalidate test history and latest test queries
      queryClient.invalidateQueries({ queryKey: ["pack-tests", packRef] });
    },
  });
}

// Register pack with test execution
export function useRegisterPack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      path,
      force = false,
      skipTests = false,
    }: {
      path: string;
      force?: boolean;
      skipTests?: boolean;
    }): Promise<{
      data: {
        pack: any;
        test_result: PackTestResult | null;
        tests_skipped: boolean;
      };
    }> => {
      const token = localStorage.getItem("access_token");
      const response = await fetch(
        "http://localhost:8080/api/v1/packs/register",
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${token}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            path,
            force,
            skip_tests: skipTests,
          }),
        },
      );

      if (!response.ok) {
        const error = await response.json().catch(() => ({}));
        throw new Error(
          error.error || `Failed to register pack: ${response.statusText}`,
        );
      }

      return response.json();
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

// Install pack from remote source
export function useInstallPack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      source,
      refSpec,
      force = false,
      skipTests = false,
      skipDeps = false,
    }: {
      source: string;
      refSpec?: string;
      force?: boolean;
      skipTests?: boolean;
      skipDeps?: boolean;
    }): Promise<{
      data: {
        pack: any;
        test_result: PackTestResult | null;
        tests_skipped: boolean;
      };
    }> => {
      const token = localStorage.getItem("access_token");
      const response = await fetch(
        "http://localhost:8080/api/v1/packs/install",
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${token}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            source,
            ref_spec: refSpec,
            force,
            skip_tests: skipTests,
            skip_deps: skipDeps,
          }),
        },
      );

      if (!response.ok) {
        const error = await response.json().catch(() => ({}));
        throw new Error(
          error.error || `Failed to install pack: ${response.statusText}`,
        );
      }

      return response.json();
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
