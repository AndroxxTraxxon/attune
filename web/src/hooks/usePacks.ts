import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { PacksService } from "@/api";
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
