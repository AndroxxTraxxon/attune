import { useState } from "react";
import { useParams, Link } from "react-router-dom";
import {
  Shield,
  ArrowLeft,
  Trash2,
  Plus,
  Tag,
  Snowflake,
  Sun,
  ShieldCheck,
  User,
  FileJson,
  Search,
  KeyRound,
  Copy,
} from "lucide-react";
import {
  useIdentity,
  usePermissionSets,
  useCreateIdentityRoleAssignment,
  useDeleteIdentityRoleAssignment,
  useCreatePermissionAssignment,
  useDeletePermissionAssignment,
  useFreezeIdentity,
  useUnfreezeIdentity,
  useIntegrationTokens,
  useCreateIntegrationToken,
  useRevokeIntegrationToken,
  useDeleteIntegrationToken,
  type IntegrationToken,
} from "@/hooks/usePermissions";

interface RoleAssignment {
  id: number;
  identity_id: number;
  role: string;
  source: string;
  managed: boolean;
  created: string;
  updated: string;
}

interface DirectPermission {
  id: number;
  identity_id: number;
  permission_set_id: number;
  permission_set_ref: string;
  created: string;
}

interface IdentityDetail {
  id: number;
  login: string;
  display_name: string | null;
  frozen: boolean;
  attributes: Record<string, unknown>;
  roles: RoleAssignment[];
  direct_permissions: DirectPermission[];
}

export default function IdentityDetailPage() {
  const { id: idParam } = useParams<{ id: string }>();
  const id = Number(idParam) || 0;

  const { data: rawData, isLoading, error } = useIdentity(id);
  const { data: permissionSets } = usePermissionSets();

  const createRoleMutation = useCreateIdentityRoleAssignment();
  const deleteRoleMutation = useDeleteIdentityRoleAssignment();
  const createPermMutation = useCreatePermissionAssignment();
  const deletePermMutation = useDeletePermissionAssignment();
  const freezeMutation = useFreezeIdentity();
  const unfreezeMutation = useUnfreezeIdentity();
  const { data: integrationTokens = [] } = useIntegrationTokens(id);
  const createTokenMutation = useCreateIntegrationToken();
  const revokeTokenMutation = useRevokeIntegrationToken();
  const deleteTokenMutation = useDeleteIntegrationToken();

  const [showAddRole, setShowAddRole] = useState(false);
  const [newRole, setNewRole] = useState("");
  const [showAssignPerm, setShowAssignPerm] = useState(false);
  const [selectedPermSetRef, setSelectedPermSetRef] = useState("");
  const [permSetSearch, setPermSetSearch] = useState("");
  const [showCreateToken, setShowCreateToken] = useState(false);
  const [tokenLabel, setTokenLabel] = useState("");
  const [tokenDescription, setTokenDescription] = useState("");
  const [tokenExpiresAt, setTokenExpiresAt] = useState("");
  const [newTokenSecret, setNewTokenSecret] = useState<string | null>(null);

  const identity = (rawData as unknown as { data: IdentityDetail } | undefined)?.data;

  const handleAddRole = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newRole.trim()) return;
    try {
      await createRoleMutation.mutateAsync({ identityId: id, role: newRole.trim() });
      setNewRole("");
      setShowAddRole(false);
    } catch (err) {
      console.error("Failed to add role:", err);
    }
  };

  const handleDeleteRole = async (assignmentId: number, role: string) => {
    if (window.confirm("Remove role \"" + role + "\" from this identity?")) {
      try {
        await deleteRoleMutation.mutateAsync(assignmentId);
      } catch (err) {
        console.error("Failed to delete role assignment:", err);
      }
    }
  };

  const handleAssignPermission = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedPermSetRef) return;
    try {
      await createPermMutation.mutateAsync({ identity_id: id, permission_set_ref: selectedPermSetRef });
      setSelectedPermSetRef("");
      setPermSetSearch("");
      setShowAssignPerm(false);
    } catch (err) {
      console.error("Failed to assign permission set:", err);
    }
  };

  const handleDeletePermission = async (assignmentId: number, ref: string) => {
    if (window.confirm("Remove permission set \"" + ref + "\" from this identity?")) {
      try {
        await deletePermMutation.mutateAsync(assignmentId);
      } catch (err) {
        console.error("Failed to remove permission assignment:", err);
      }
    }
  };

  const handleToggleFreeze = async () => {
    if (!identity) return;
    const action = identity.frozen ? "unfreeze" : "freeze";
    if (!window.confirm("Are you sure you want to " + action + " identity \"" + identity.login + "\"?")) return;
    try {
      if (identity.frozen) {
        await unfreezeMutation.mutateAsync(id);
      } else {
        await freezeMutation.mutateAsync(id);
      }
    } catch (err) {
      console.error("Failed to " + action + " identity:", err);
    }
  };

  const handleCreateIntegrationToken = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!tokenLabel.trim()) return;
    const response = await createTokenMutation.mutateAsync({
      identityId: id,
      data: {
        label: tokenLabel.trim(),
        description: tokenDescription.trim() || null,
        expires_at: tokenExpiresAt ? new Date(tokenExpiresAt).toISOString() : null,
      },
    });
    setNewTokenSecret(response.token);
    setTokenLabel("");
    setTokenDescription("");
    setTokenExpiresAt("");
    setShowCreateToken(false);
  };

  const handleRevokeIntegrationToken = async (token: IntegrationToken) => {
    if (!window.confirm(`Revoke integration token "${token.label}"?`)) return;
    await revokeTokenMutation.mutateAsync({
      identityId: id,
      tokenId: token.id,
      reason: "Revoked from web UI",
    });
  };

  const handleDeleteIntegrationToken = async (token: IntegrationToken) => {
    if (!window.confirm(`Delete integration token metadata for "${token.label}"?`)) return;
    await deleteTokenMutation.mutateAsync({ identityId: id, tokenId: token.id });
  };

  const copyNewToken = async () => {
    if (!newTokenSecret) return;
    await navigator.clipboard.writeText(newTokenSecret);
  };

  const formatDate = (dateString: string) => new Date(dateString).toLocaleString();

  const assignedPermSetRefs = new Set(identity?.direct_permissions?.map((p) => p.permission_set_ref) ?? []);
  const availablePermSets = (permissionSets ?? []).filter((ps) => !assignedPermSetRefs.has(ps.ref));
  const filteredAvailablePermSets = permSetSearch.trim()
    ? availablePermSets.filter((ps) =>
        ps.ref.toLowerCase().includes(permSetSearch.toLowerCase()) ||
        (ps.label ?? "").toLowerCase().includes(permSetSearch.toLowerCase())
      )
    : availablePermSets;

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <p className="mt-3 text-sm text-gray-500">Loading identity...</p>
          </div>
        </div>
      </div>
    );
  }

  if (error || !identity) {
    return (
      <div className="p-6">
        <Link to="/access-control" className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800 mb-6">
          <ArrowLeft className="w-4 h-4" /> Back to Access Control
        </Link>
        <div className="bg-white rounded-lg shadow p-12 text-center">
          <Shield className="mx-auto h-12 w-12 text-gray-400" />
          <p className="mt-4 text-red-600">Failed to load identity</p>
          <p className="text-sm text-gray-500 mt-1">{error instanceof Error ? error.message : "Identity not found"}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-5xl">
      <Link to="/access-control" className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800 mb-6">
        <ArrowLeft className="w-4 h-4" /> Back to Access Control
      </Link>

      {/* Header */}
      <div className="bg-white rounded-lg shadow p-6 mb-6">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-4">
            <div className="bg-blue-100 p-3 rounded-full">
              <User className="w-6 h-6 text-blue-600" />
            </div>
            <div>
              <div className="flex items-center gap-3">
                <h1 className="text-2xl font-bold text-gray-900">{identity.login}</h1>
                {identity.frozen && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-semibold bg-blue-100 text-blue-800">
                    <Snowflake className="w-3 h-3" /> Frozen
                  </span>
                )}
              </div>
              {identity.display_name && <p className="text-gray-600 mt-1">{identity.display_name}</p>}
              <p className="text-sm text-gray-400 mt-1">ID: {identity.id}</p>
            </div>
          </div>
          <button onClick={handleToggleFreeze} disabled={freezeMutation.isPending || unfreezeMutation.isPending}
            className={"inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 " + (identity.frozen ? "bg-green-50 text-green-700 hover:bg-green-100 border border-green-200" : "bg-blue-50 text-blue-700 hover:bg-blue-100 border border-blue-200")}>
            {identity.frozen ? (<><Sun className="w-4 h-4" /> Unfreeze</>) : (<><Snowflake className="w-4 h-4" /> Freeze</>)}
          </button>
        </div>
      </div>

      {/* Roles Section */}
      <div className="bg-white rounded-lg shadow p-6 mb-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Tag className="w-5 h-5 text-violet-500" />
            <h2 className="text-lg font-semibold text-gray-900">Role Assignments</h2>
            <span className="text-sm text-gray-500">({identity.roles?.length || 0})</span>
          </div>
          <button onClick={() => setShowAddRole(!showAddRole)} className="inline-flex items-center gap-1 px-3 py-1.5 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700 transition-colors">
            <Plus className="w-4 h-4" /> Add Role
          </button>
        </div>

        {showAddRole && (
          <form onSubmit={handleAddRole} className="flex items-center gap-3 mb-4 p-3 bg-gray-50 rounded-lg">
            <input type="text" value={newRole} onChange={(e) => setNewRole(e.target.value)} placeholder="Role name (e.g. admin, operator, viewer)" className="flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-violet-500 text-sm" autoFocus />
            <button type="submit" disabled={!newRole.trim() || createRoleMutation.isPending} className="px-4 py-2 bg-violet-600 text-white rounded-lg hover:bg-violet-700 disabled:opacity-50 text-sm transition-colors">
              {createRoleMutation.isPending ? "Adding..." : "Add"}
            </button>
            <button type="button" onClick={() => { setShowAddRole(false); setNewRole(""); }} className="px-4 py-2 text-gray-600 hover:text-gray-900 text-sm">Cancel</button>
          </form>
        )}

        {identity.roles && identity.roles.length > 0 ? (
          <div className="divide-y divide-gray-100">
            {identity.roles.map((ra) => (
              <div key={ra.id} className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3">
                  <span className="inline-flex items-center px-2.5 py-1 rounded-full text-sm font-medium bg-purple-100 text-purple-800">{ra.role}</span>
                  <span className="text-xs text-gray-500">Source: {ra.source}</span>
                  {ra.managed && (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-800">Managed</span>
                  )}
                  <span className="text-xs text-gray-400">{formatDate(ra.created)}</span>
                </div>
                {!ra.managed && (
                  <button onClick={() => handleDeleteRole(ra.id, ra.role)} className="text-red-400 hover:text-red-600 p-1" title="Remove role">
                    <Trash2 className="w-4 h-4" />
                  </button>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div className="text-center py-6">
            <Tag className="mx-auto h-8 w-8 text-gray-300" />
            <p className="mt-2 text-sm text-gray-500">No roles assigned</p>
          </div>
        )}
      </div>

      {/* Direct Permission Sets Section */}
      <div className="bg-white rounded-lg shadow p-6 mb-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-5 h-5 text-indigo-500" />
            <h2 className="text-lg font-semibold text-gray-900">Direct Permission Sets</h2>
            <span className="text-sm text-gray-500">({identity.direct_permissions?.length || 0})</span>
          </div>
          <button onClick={() => setShowAssignPerm(!showAssignPerm)} className="inline-flex items-center gap-1 px-3 py-1.5 text-sm bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors">
            <Plus className="w-4 h-4" /> Assign Permission Set
          </button>
        </div>

        {showAssignPerm && (
          <form onSubmit={handleAssignPermission} className="mb-4 p-3 bg-gray-50 rounded-lg space-y-2">
            {selectedPermSetRef ? (
              <div className="flex items-center gap-2">
                <div className="flex items-center gap-2 px-3 py-1.5 bg-indigo-100 text-indigo-800 rounded-md text-sm font-mono flex-1">
                  <Shield className="w-3.5 h-3.5 flex-shrink-0" />
                  <span className="truncate">{selectedPermSetRef}</span>
                  <button type="button" onClick={() => { setSelectedPermSetRef(""); setPermSetSearch(""); }} className="ml-auto text-indigo-500 hover:text-indigo-700">
                    &#x2715;
                  </button>
                </div>
                <button type="submit" disabled={createPermMutation.isPending} className="px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 disabled:opacity-50 text-sm transition-colors whitespace-nowrap">
                  {createPermMutation.isPending ? "Assigning..." : "Assign"}
                </button>
                <button type="button" onClick={() => { setShowAssignPerm(false); setSelectedPermSetRef(""); setPermSetSearch(""); }} className="px-3 py-2 text-gray-600 hover:text-gray-900 text-sm">Cancel</button>
              </div>
            ) : (
              <div className="relative">
                <div className="flex items-center gap-2">
                  <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" />
                    <input
                      type="text"
                      value={permSetSearch}
                      onChange={(e) => setPermSetSearch(e.target.value)}
                      placeholder="Search permission sets by ref or label..."
                      className="w-full pl-9 pr-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500 text-sm"
                      autoFocus
                    />
                  </div>
                  <button type="button" onClick={() => { setShowAssignPerm(false); setPermSetSearch(""); }} className="px-3 py-2 text-gray-600 hover:text-gray-900 text-sm">Cancel</button>
                </div>
                {permSetSearch.trim() && (
                  <div className="mt-1 bg-white border border-gray-200 rounded-lg shadow-lg max-h-52 overflow-y-auto">
                    {filteredAvailablePermSets.length === 0 ? (
                      <div className="px-4 py-3 text-sm text-gray-500">No matching permission sets</div>
                    ) : (
                      filteredAvailablePermSets.map((ps) => (
                        <button
                          key={ps.ref}
                          type="button"
                          onClick={() => { setSelectedPermSetRef(ps.ref); setPermSetSearch(""); }}
                          className="w-full flex items-start gap-3 px-4 py-2.5 text-left hover:bg-indigo-50 transition-colors"
                        >
                          <Shield className="w-4 h-4 text-indigo-400 flex-shrink-0 mt-0.5" />
                          <div className="min-w-0">
                            <div className="text-sm font-mono font-medium text-gray-900 truncate">{ps.ref}</div>
                            {ps.label && <div className="text-xs text-gray-500 truncate">{ps.label}</div>}
                          </div>
                        </button>
                      ))
                    )}
                  </div>
                )}
              </div>
            )}
          </form>
        )}

        {identity.direct_permissions && identity.direct_permissions.length > 0 ? (
          <div className="divide-y divide-gray-100">
            {identity.direct_permissions.map((dp) => (
              <div key={dp.id} className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3">
                  <Shield className="w-4 h-4 text-indigo-400" />
                  <span className="text-sm font-mono font-medium text-gray-900">{dp.permission_set_ref}</span>
                  <span className="text-xs text-gray-400">Assigned {formatDate(dp.created)}</span>
                </div>
                <button onClick={() => handleDeletePermission(dp.id, dp.permission_set_ref)} className="text-red-400 hover:text-red-600 p-1" title="Remove permission set">
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-center py-6">
            <ShieldCheck className="mx-auto h-8 w-8 text-gray-300" />
            <p className="mt-2 text-sm text-gray-500">No direct permission sets assigned</p>
            <p className="text-xs text-gray-400 mt-1">Permission sets can also be inherited through roles</p>
          </div>
        )}
      </div>

      {/* Integration Tokens Section */}
      <div className="bg-white rounded-lg shadow p-6 mb-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <KeyRound className="w-5 h-5 text-amber-500" />
            <h2 className="text-lg font-semibold text-gray-900">Integration Tokens</h2>
            <span className="text-sm text-gray-500">({integrationTokens.length})</span>
          </div>
          <button onClick={() => setShowCreateToken(!showCreateToken)} className="inline-flex items-center gap-1 px-3 py-1.5 text-sm bg-amber-600 text-white rounded-lg hover:bg-amber-700 transition-colors">
            <Plus className="w-4 h-4" /> Create Token
          </button>
        </div>

        {newTokenSecret && (
          <div className="mb-4 rounded-lg border border-amber-200 bg-amber-50 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-amber-900">Copy this token now. It will not be shown again.</p>
                <code className="mt-2 block break-all rounded bg-white px-3 py-2 text-xs text-amber-900 border border-amber-200">{newTokenSecret}</code>
              </div>
              <button type="button" onClick={copyNewToken} className="inline-flex items-center gap-1 rounded-md border border-amber-300 px-2 py-1 text-xs font-medium text-amber-800 hover:bg-amber-100">
                <Copy className="w-3 h-3" /> Copy
              </button>
            </div>
            <button type="button" onClick={() => setNewTokenSecret(null)} className="mt-3 text-xs text-amber-800 hover:text-amber-950">Dismiss</button>
          </div>
        )}

        {showCreateToken && (
          <form onSubmit={handleCreateIntegrationToken} className="mb-4 p-3 bg-gray-50 rounded-lg space-y-3">
            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500">Label</label>
              <input value={tokenLabel} onChange={(e) => setTokenLabel(e.target.value)} required maxLength={255} placeholder="CI deploy bot" className="mt-1 w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-amber-500" />
            </div>
            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500">Description</label>
              <textarea value={tokenDescription} onChange={(e) => setTokenDescription(e.target.value)} maxLength={2000} rows={2} placeholder="What this integration token is used for" className="mt-1 w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-amber-500" />
            </div>
            <div>
              <label className="block text-xs font-medium uppercase tracking-wide text-gray-500">Expires At</label>
              <input type="datetime-local" value={tokenExpiresAt} onChange={(e) => setTokenExpiresAt(e.target.value)} className="mt-1 w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-amber-500" />
            </div>
            <div className="flex justify-end gap-2">
              <button type="button" onClick={() => setShowCreateToken(false)} className="px-3 py-2 text-sm text-gray-600 hover:text-gray-900">Cancel</button>
              <button type="submit" disabled={!tokenLabel.trim() || createTokenMutation.isPending} className="px-4 py-2 text-sm text-white bg-amber-600 rounded-lg hover:bg-amber-700 disabled:opacity-50">
                {createTokenMutation.isPending ? "Creating..." : "Create"}
              </button>
            </div>
          </form>
        )}

        {integrationTokens.length > 0 ? (
          <div className="divide-y divide-gray-100">
            {integrationTokens.map((token) => (
              <div key={token.id} className="flex items-center justify-between py-3 gap-4">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-gray-900 truncate">{token.label}</span>
                    <span className={"inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium " + (token.active ? "bg-green-100 text-green-800" : "bg-gray-100 text-gray-600")}>
                      {token.active ? "Active" : "Inactive"}
                    </span>
                  </div>
                  <div className="mt-1 text-xs text-gray-500">
                    <span className="font-mono">{token.token_prefix}...{token.token_suffix}</span>
                    <span className="mx-2">·</span>
                    <span>Created {formatDate(token.created)}</span>
                    <span className="mx-2">·</span>
                    <span>Expires {token.expires_at ? formatDate(token.expires_at) : "never"}</span>
                    <span className="mx-2">·</span>
                    <span>Last used {token.last_used_at ? formatDate(token.last_used_at) : "never"}</span>
                  </div>
                  {token.description && <p className="mt-1 text-xs text-gray-500 truncate">{token.description}</p>}
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  {token.active && (
                    <button onClick={() => handleRevokeIntegrationToken(token)} disabled={revokeTokenMutation.isPending} className="px-3 py-1.5 text-xs font-medium text-amber-700 bg-amber-50 border border-amber-200 rounded-md hover:bg-amber-100 disabled:opacity-50">
                      Revoke
                    </button>
                  )}
                  <button onClick={() => handleDeleteIntegrationToken(token)} disabled={deleteTokenMutation.isPending} className="text-red-400 hover:text-red-600 p-1 disabled:opacity-50" title="Delete token metadata">
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-center py-6">
            <KeyRound className="mx-auto h-8 w-8 text-gray-300" />
            <p className="mt-2 text-sm text-gray-500">No integration tokens</p>
            <p className="text-xs text-gray-400 mt-1">Create a token to enable passwordless API login for an integration.</p>
          </div>
        )}
      </div>

      {/* Attributes Section */}
      <div className="bg-white rounded-lg shadow p-6">
        <div className="flex items-center gap-2 mb-4">
          <FileJson className="w-5 h-5 text-gray-500" />
          <h2 className="text-lg font-semibold text-gray-900">Attributes</h2>
        </div>
        <pre className="bg-gray-50 border border-gray-200 rounded-lg p-4 text-sm text-gray-800 overflow-x-auto max-h-64 overflow-y-auto font-mono leading-relaxed">
          {JSON.stringify(identity.attributes, null, 2)}
        </pre>
      </div>
    </div>
  );
}
