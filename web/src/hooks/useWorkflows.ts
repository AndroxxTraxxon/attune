import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { WorkflowsService } from "@/api";
import type { CreateWorkflowRequest, UpdateWorkflowRequest } from "@/api";
import type { SaveWorkflowFileRequest } from "@/types/workflow";
import { OpenAPI } from "@/api/core/OpenAPI";
import { request as __request } from "@/api/core/request";

interface WorkflowsQueryParams {
  page?: number;
  pageSize?: number;
  packRef?: string;
  tags?: string;
  search?: string;
}

// Fetch all workflows with pagination and filtering
export function useWorkflows(params?: WorkflowsQueryParams) {
  return useQuery({
    queryKey: ["workflows", params],
    queryFn: async () => {
      const response = await WorkflowsService.listWorkflows({
        page: params?.page || 1,
        pageSize: params?.pageSize || 50,
        tags: params?.tags,
        search: params?.search,
        packRef: params?.packRef,
      });
      return response;
    },
    staleTime: 30000,
  });
}

// Fetch single workflow by ref
export function useWorkflow(ref: string) {
  return useQuery({
    queryKey: ["workflows", ref],
    queryFn: async () => {
      const response = await WorkflowsService.getWorkflow({ ref });
      return response;
    },
    enabled: !!ref,
    staleTime: 30000,
  });
}

// Create a new workflow
export function useCreateWorkflow() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateWorkflowRequest) => {
      const response = await WorkflowsService.createWorkflow({
        requestBody: data,
      });
      return response;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
    },
  });
}

// Update existing workflow
export function useUpdateWorkflow() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      ref,
      data,
    }: {
      ref: string;
      data: UpdateWorkflowRequest;
    }) => {
      const response = await WorkflowsService.updateWorkflow({
        ref,
        requestBody: data,
      });
      return response;
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
      queryClient.invalidateQueries({
        queryKey: ["workflows", variables.ref],
      });
    },
  });
}

// Delete workflow
export function useDeleteWorkflow() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (ref: string) => {
      await WorkflowsService.deleteWorkflow({ ref });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
    },
  });
}

// Save workflow file to disk and sync to DB
// This calls a custom endpoint not in the generated client
export function useSaveWorkflowFile() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: SaveWorkflowFileRequest) => {
      const response = await __request(OpenAPI, {
        method: "POST",
        url: "/api/v1/packs/{pack_ref}/workflow-files",
        path: {
          pack_ref: data.pack_ref,
        },
        body: data,
        mediaType: "application/json",
      });
      return response;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
      queryClient.invalidateQueries({ queryKey: ["actions"] });
    },
  });
}

// Update an existing workflow file on disk and sync to DB
export function useUpdateWorkflowFile() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      workflowRef,
      data,
    }: {
      workflowRef: string;
      data: SaveWorkflowFileRequest;
    }) => {
      const response = await __request(OpenAPI, {
        method: "PUT",
        url: "/api/v1/workflows/{ref}/file",
        path: {
          ref: workflowRef,
        },
        body: data,
        mediaType: "application/json",
      });
      return response;
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
      queryClient.invalidateQueries({
        queryKey: ["workflows", variables.workflowRef],
      });
      queryClient.invalidateQueries({ queryKey: ["actions"] });
    },
  });
}
