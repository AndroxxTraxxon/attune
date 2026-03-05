import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { SensorsService } from "@/api";
import type { CreateSensorRequest, UpdateSensorRequest } from "@/api";

interface SensorsQueryParams {
  page?: number;
  pageSize?: number;
  packRef?: string;
  enabled?: boolean;
}

// Fetch all sensors with pagination
export function useSensors(params?: SensorsQueryParams) {
  return useQuery({
    queryKey: ["sensors", params],
    queryFn: async () => {
      if (params?.packRef) {
        return await SensorsService.listSensorsByPack({
          packRef: params.packRef,
          page: params?.page || 1,
          pageSize: params?.pageSize || 50,
        });
      }
      return await SensorsService.listSensors({
        page: params?.page || 1,
        pageSize: params?.pageSize || 50,
      });
    },
    staleTime: 30000, // 30 seconds
  });
}

// Fetch enabled sensors only
export function useEnabledSensors(
  params?: Omit<SensorsQueryParams, "enabled">,
) {
  return useSensors({ ...params, enabled: true });
}

// Fetch single sensor by reference
export function useSensor(ref: string) {
  return useQuery({
    queryKey: ["sensors", ref],
    queryFn: async () => {
      return await SensorsService.getSensor({ ref });
    },
    enabled: !!ref,
    staleTime: 30000,
  });
}

// Fetch sensors by pack
export function usePackSensors(packRef: string) {
  return useQuery({
    queryKey: ["packs", packRef, "sensors"],
    queryFn: async () => {
      return await SensorsService.listSensorsByPack({
        packRef,
        page: 1,
        pageSize: 100,
      });
    },
    enabled: !!packRef,
    staleTime: 30000,
  });
}

// Create a new sensor
export function useCreateSensor() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateSensorRequest) => {
      return await SensorsService.createSensor({ requestBody: data });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sensors"] });
    },
  });
}

// Update existing sensor
export function useUpdateSensor() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      ref,
      data,
    }: {
      ref: string;
      data: UpdateSensorRequest;
    }) => {
      return await SensorsService.updateSensor({ ref, requestBody: data });
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["sensors"] });
      queryClient.invalidateQueries({ queryKey: ["sensors", variables.ref] });
    },
  });
}

// Delete sensor
export function useDeleteSensor() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (ref: string) => {
      await SensorsService.deleteSensor({ ref });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sensors"] });
    },
  });
}
