import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { PacksService } from "@/api";
import { request as __request } from "@/api/core/request";
import { OpenAPI } from "@/api/core/OpenAPI";
import type { CreatePackRequest, UpdatePackRequest } from "@/api";

interface PacksQueryParams {
  page?: number;
  pageSize?: number;
}

// Fetch all packs with pagination
export function usePacks(params?: PacksQueryParams) {
  return useQuery({
    queryKey: ["packs", params],
    queryFn: async () => {
      const response = await PacksService.listPacks({
        page: params?.page || 1,
        pageSize: params?.pageSize || 50,
      });
      return response;
    },
    staleTime: 30000, // 30 seconds
  });
}

// Fetch single pack by ref
export function usePack(ref: string) {
  return useQuery({
    queryKey: ["packs", ref],
    queryFn: async () => {
      const response = await PacksService.getPack({ ref });
      return response;
    },
    enabled: !!ref,
    staleTime: 30000,
  });
}

// Create a new pack
export function useCreatePack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreatePackRequest) => {
      const response = await PacksService.createPack({ requestBody: data });
      return response;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["packs"] });
    },
  });
}

// Update existing pack
export function useUpdatePack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      ref,
      data,
    }: {
      ref: string;
      data: UpdatePackRequest;
    }) => {
      const response = await PacksService.updatePack({
        ref,
        requestBody: data,
      });
      return response;
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["packs"] });
      queryClient.invalidateQueries({ queryKey: ["packs", variables.ref] });
    },
  });
}

// Delete pack
export function useDeletePack() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (ref: string) => {
      await PacksService.deletePack({ ref });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["packs"] });
    },
  });
}

export function usePackIndices() {
  return useQuery({
    queryKey: ["pack-indices"],
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    queryFn: (): Promise<any> =>
      __request(OpenAPI, {
        method: "GET",
        url: "/api/v1/pack-indices",
      }),
    staleTime: 30000,
  });
}

export function useCreatePackIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: {
      name?: string;
      url: string;
      position?: number;
      enabled: boolean;
      headers: Record<string, string>;
    }) =>
      __request(OpenAPI, {
        method: "POST",
        url: "/api/v1/pack-indices",
        body: data,
        mediaType: "application/json",
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["pack-indices"] });
      queryClient.invalidateQueries({ queryKey: ["indexed-packs"] });
    },
  });
}

export function useUpdatePackIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      data,
    }: {
      id: number;
      data: {
        name?: string | null;
        url?: string;
        position?: number;
        enabled?: boolean;
      };
    }) =>
      __request(OpenAPI, {
        method: "PUT",
        url: "/api/v1/pack-indices/{id}",
        path: { id },
        body: data,
        mediaType: "application/json",
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["pack-indices"] });
      queryClient.invalidateQueries({ queryKey: ["indexed-packs"] });
    },
  });
}

export function useDeletePackIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      __request(OpenAPI, {
        method: "DELETE",
        url: "/api/v1/pack-indices/{id}",
        path: { id },
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["pack-indices"] });
      queryClient.invalidateQueries({ queryKey: ["indexed-packs"] });
    },
  });
}

export function useIndexedPacks(query?: string) {
  return useQuery({
    queryKey: ["indexed-packs", query],
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    queryFn: (): Promise<any> =>
      __request(OpenAPI, {
        method: "GET",
        url: "/api/v1/pack-indices/browse",
        query: { q: query || undefined },
      }),
    staleTime: 30000,
  });
}
