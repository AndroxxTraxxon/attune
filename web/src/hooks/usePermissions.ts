import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { PermissionsService } from "@/api";
import type {
  CreateIdentityRequest,
  UpdateIdentityRequest,
  CreatePermissionAssignmentRequest,
} from "@/api";

// Fetch all identities with pagination
export function useIdentities(params?: { page?: number; pageSize?: number }) {
  return useQuery({
    queryKey: ["identities", params],
    queryFn: async () => {
      return await PermissionsService.listIdentities({
        page: params?.page || 1,
        pageSize: params?.pageSize || 50,
      });
    },
    staleTime: 30000,
  });
}

// Fetch single identity by ID
export function useIdentity(id: number) {
  return useQuery({
    queryKey: ["identities", id],
    queryFn: async () => {
      return await PermissionsService.getIdentity({ id });
    },
    enabled: id > 0,
    staleTime: 30000,
  });
}

// Create a new identity
export function useCreateIdentity() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateIdentityRequest) => {
      return await PermissionsService.createIdentity({ requestBody: data });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

// Update an existing identity
export function useUpdateIdentity() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      id,
      data,
    }: {
      id: number;
      data: UpdateIdentityRequest;
    }) => {
      return await PermissionsService.updateIdentity({ id, requestBody: data });
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
      queryClient.invalidateQueries({
        queryKey: ["identities", variables.id],
      });
    },
  });
}

// Delete an identity
export function useDeleteIdentity() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.deleteIdentity({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

// Fetch permission sets
export function usePermissionSets(packRef?: string | null) {
  return useQuery({
    queryKey: ["permission-sets", packRef],
    queryFn: async () => {
      return await PermissionsService.listPermissionSets({ packRef });
    },
    staleTime: 30000,
  });
}

// Fetch permission assignments for an identity
export function useIdentityPermissions(id: number) {
  return useQuery({
    queryKey: ["identity-permissions", id],
    queryFn: async () => {
      return await PermissionsService.listIdentityPermissions({ id });
    },
    enabled: id > 0,
    staleTime: 30000,
  });
}

// Create a permission assignment
export function useCreatePermissionAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreatePermissionAssignmentRequest) => {
      return await PermissionsService.createPermissionAssignment({
        requestBody: data,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
      queryClient.invalidateQueries({ queryKey: ["identity-permissions"] });
      queryClient.invalidateQueries({ queryKey: ["permission-sets"] });
    },
  });
}

// Delete a permission assignment
export function useDeletePermissionAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.deletePermissionAssignment({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
      queryClient.invalidateQueries({ queryKey: ["identity-permissions"] });
      queryClient.invalidateQueries({ queryKey: ["permission-sets"] });
    },
  });
}

// Create a role assignment for an identity
export function useCreateIdentityRoleAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      identityId,
      role,
    }: {
      identityId: number;
      role: string;
    }) => {
      return await PermissionsService.createIdentityRoleAssignment({
        id: identityId,
        requestBody: { role },
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

// Delete a role assignment from an identity
export function useDeleteIdentityRoleAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.deleteIdentityRoleAssignment({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

// Create a role assignment for a permission set
export function useCreatePermissionSetRoleAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      permissionSetId,
      role,
    }: {
      permissionSetId: number;
      role: string;
    }) => {
      return await PermissionsService.createPermissionSetRoleAssignment({
        id: permissionSetId,
        requestBody: { role },
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["permission-sets"] });
    },
  });
}

// Delete a role assignment from a permission set
export function useDeletePermissionSetRoleAssignment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.deletePermissionSetRoleAssignment({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["permission-sets"] });
    },
  });
}

// Freeze an identity
export function useFreezeIdentity() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.freezeIdentity({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}

// Unfreeze an identity
export function useUnfreezeIdentity() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      return await PermissionsService.unfreezeIdentity({ id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identities"] });
    },
  });
}
