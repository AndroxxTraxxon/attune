import { QueryClient } from "@tanstack/react-query";

interface HttpError extends Error {
  response?: {
    status: number;
  };
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error: HttpError) => {
        // Don't retry on 401 (handled by interceptor) or 403 (permission denied)
        if (
          error?.response?.status === 401 ||
          error?.response?.status === 403
        ) {
          return false;
        }
        // Retry once for other errors
        return failureCount < 1;
      },
      refetchOnWindowFocus: false,
      staleTime: 30000, // 30 seconds
      gcTime: 5 * 60 * 1000, // 5 minutes (formerly cacheTime)
    },
    mutations: {
      retry: 0,
    },
  },
});
