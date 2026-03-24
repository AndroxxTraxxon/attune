import { useState } from "react";
import { useParams, Link } from "react-router-dom";
import {
  ArrowLeft,
  BarChart3,
  Globe,
  History,
  Key,
  MessageSquare,
  Package,
  Plus,
  Shield,
  Tag,
  Trash2,
  Users,
} from "lucide-react";
import {
  usePermissionSets,
  useCreatePermissionSetRoleAssignment,
  useDeletePermissionSetRoleAssignment,
} from "@/hooks/usePermissions";
import { navIcons } from "@/components/layout/navIcons";

// ── Domain interfaces ──────────────────────────────────────────────────────────

interface PermissionSetRoleAssignment {
  id: number;
  permission_set_id: number;
  permission_set_ref: string | null;
  role: string;
  created: string;
}

interface PermissionSetWithRoles {
  id: number;
  ref: string;
  pack_ref?: string | null;
  label?: string | null;
  description?: string | null;
  grants: unknown;
  roles?: PermissionSetRoleAssignment[];
}

// ── Grants model ───────────────────────────────────────────────────────────────

interface GrantConstraints {
  pack_refs?: string[];
  owner?: string; // "self" | "any" | "none"
  owner_types?: string[];
  owner_refs?: string[];
  visibility?: string[];
  execution_scope?: string; // "self" | "descendants" | "any"
  refs?: string[];
  ids?: number[];
  encrypted?: boolean;
  attributes?: Record<string, unknown>;
}

interface ParsedGrant {
  resource: string;
  actions: string[];
  constraints?: GrantConstraints;
}

function parseGrants(raw: unknown): ParsedGrant[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter(
    (g): g is ParsedGrant =>
      typeof g === "object" &&
      g !== null &&
      typeof (g as ParsedGrant).resource === "string" &&
      Array.isArray((g as ParsedGrant).actions),
  );
}

// ── Display metadata ───────────────────────────────────────────────────────────

type ResourceMeta = {
  icon: React.ComponentType<{ className?: string }>;
  color: string;
  label: string;
};

const RESOURCE_META: Record<string, ResourceMeta> = {
  packs: { icon: navIcons.packs, color: "text-green-600", label: "Packs" },
  actions: {
    icon: navIcons.actions,
    color: "text-yellow-500",
    label: "Actions",
  },
  rules: { icon: navIcons.rules, color: "text-blue-600", label: "Rules" },
  triggers: {
    icon: navIcons.triggers,
    color: "text-orange-500",
    label: "Triggers",
  },
  executions: {
    icon: navIcons.executions,
    color: "text-purple-600",
    label: "Executions",
  },
  events: { icon: navIcons.events, color: "text-cyan-600", label: "Events" },
  enforcements: {
    icon: navIcons.enforcements,
    color: "text-red-500",
    label: "Enforcements",
  },
  inquiries: {
    icon: MessageSquare,
    color: "text-teal-600",
    label: "Inquiries",
  },
  keys: { icon: navIcons.keys, color: "text-amber-600", label: "Keys" },
  artifacts: {
    icon: navIcons.artifacts,
    color: "text-indigo-500",
    label: "Artifacts",
  },
  webhooks: { icon: Globe, color: "text-sky-600", label: "Webhooks" },
  analytics: { icon: BarChart3, color: "text-rose-500", label: "Analytics" },
  history: { icon: History, color: "text-gray-500", label: "History" },
  identities: { icon: Users, color: "text-blue-700", label: "Identities" },
  permissions: {
    icon: navIcons.accessControl,
    color: "text-indigo-600",
    label: "Permissions",
  },
  runtimes: {
    icon: navIcons.runtimes,
    color: "text-blue-600",
    label: "Runtimes",
  },
  sensors: {
    icon: navIcons.sensors,
    color: "text-purple-600",
    label: "Sensors",
  },
};

const ACTION_STYLE: Record<string, string> = {
  read: "bg-slate-100 text-slate-700",
  create: "bg-emerald-100 text-emerald-800",
  update: "bg-amber-100 text-amber-800",
  delete: "bg-red-100 text-red-800",
  execute: "bg-violet-100 text-violet-800",
  cancel: "bg-orange-100 text-orange-800",
  respond: "bg-cyan-100 text-cyan-800",
  manage: "bg-indigo-100 text-indigo-800",
};

// ── Constraint chips ───────────────────────────────────────────────────────────

function ConstraintChips({ c }: { c: GrantConstraints }) {
  const chips: React.ReactNode[] = [];

  if (c.pack_refs?.length) {
    chips.push(
      <span
        key="pack_refs"
        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-green-50 text-green-700 border border-green-200"
      >
        <Package className="w-3 h-3 shrink-0" />
        {c.pack_refs.join(", ")}
      </span>,
    );
  }

  if (c.owner) {
    const labels: Record<string, string> = {
      self: "Own resources",
      any: "Any owner",
      none: "No owner",
    };
    chips.push(
      <span
        key="owner"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-blue-50 text-blue-700 border border-blue-200"
      >
        Owner: {labels[c.owner] ?? c.owner}
      </span>,
    );
  }

  if (c.owner_types?.length) {
    chips.push(
      <span
        key="owner_types"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-slate-100 text-slate-600 border border-slate-200"
      >
        Type: {c.owner_types.join(", ")}
      </span>,
    );
  }

  if (c.owner_refs?.length) {
    chips.push(
      <span
        key="owner_refs"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-slate-100 text-slate-600 border border-slate-200 font-mono"
      >
        Owner: {c.owner_refs.join(", ")}
      </span>,
    );
  }

  if (c.visibility?.length) {
    chips.push(
      <span
        key="visibility"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-sky-50 text-sky-700 border border-sky-200"
      >
        Visibility: {c.visibility.join(", ")}
      </span>,
    );
  }

  if (c.execution_scope) {
    const labels: Record<string, string> = {
      self: "Own executions",
      descendants: "Own + children",
      any: "All executions",
    };
    chips.push(
      <span
        key="execution_scope"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-purple-50 text-purple-700 border border-purple-200"
      >
        Scope: {labels[c.execution_scope] ?? c.execution_scope}
      </span>,
    );
  }

  if (c.refs?.length) {
    chips.push(
      <span
        key="refs"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-slate-100 text-slate-600 border border-slate-200 font-mono"
      >
        {c.refs.join(", ")}
      </span>,
    );
  }

  if (c.ids?.length) {
    chips.push(
      <span
        key="ids"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-slate-100 text-slate-600 border border-slate-200"
      >
        IDs: {c.ids.join(", ")}
      </span>,
    );
  }

  if (c.encrypted !== undefined) {
    chips.push(
      <span
        key="encrypted"
        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-amber-50 text-amber-700 border border-amber-200"
      >
        <Key className="w-3 h-3 shrink-0" />
        {c.encrypted ? "Encrypted only" : "Unencrypted only"}
      </span>,
    );
  }

  if (c.attributes && Object.keys(c.attributes).length > 0) {
    const text = Object.entries(c.attributes)
      .map(([k, v]) => `${k} = ${JSON.stringify(v)}`)
      .join(", ");
    chips.push(
      <span
        key="attributes"
        className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-rose-50 text-rose-700 border border-rose-200 font-mono"
      >
        {text}
      </span>,
    );
  }

  if (chips.length === 0) {
    return <span className="text-xs text-gray-300">—</span>;
  }

  return <div className="flex flex-col gap-1">{chips}</div>;
}

// ── Grants table ───────────────────────────────────────────────────────────────

function GrantsView({ grants }: { grants: ParsedGrant[] }) {
  if (grants.length === 0) {
    return (
      <div className="p-8 text-center">
        <Shield className="mx-auto h-8 w-8 text-gray-300" />
        <p className="mt-2 text-sm text-gray-500">No grants defined</p>
      </div>
    );
  }

  const hasConstraints = grants.some(
    (g) => g.constraints && Object.keys(g.constraints).length > 0,
  );

  return (
    <div className="overflow-y-auto max-h-[28rem]">
      <table className="min-w-full">
        <thead className="bg-gray-50 border-b border-gray-200 sticky top-0 z-10">
          <tr>
            <th className="px-4 py-2.5 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-36">
              Resource
            </th>
            <th className="px-4 py-2.5 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Permissions
            </th>
            {hasConstraints && (
              <th className="px-4 py-2.5 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                Conditions
              </th>
            )}
          </tr>
        </thead>
        <tbody className="divide-y divide-gray-100">
          {grants.map((grant, i) => {
            const meta = RESOURCE_META[grant.resource];
            const Icon = meta?.icon ?? Shield;
            const iconColor = meta?.color ?? "text-gray-400";
            const label =
              meta?.label ??
              grant.resource.charAt(0).toUpperCase() + grant.resource.slice(1);

            return (
              <tr key={i} className="hover:bg-gray-50">
                <td className="px-4 py-2.5 whitespace-nowrap">
                  <div className="flex items-center gap-1.5">
                    <Icon className={`w-3.5 h-3.5 shrink-0 ${iconColor}`} />
                    <span className="text-sm font-medium text-gray-800">
                      {label}
                    </span>
                  </div>
                </td>

                <td className="px-4 py-2.5">
                  <div className="flex flex-wrap gap-1">
                    {grant.actions.map((action) => (
                      <span
                        key={action}
                        className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${
                          ACTION_STYLE[action] ?? "bg-gray-100 text-gray-700"
                        }`}
                      >
                        {action}
                      </span>
                    ))}
                  </div>
                </td>

                {hasConstraints && (
                  <td className="px-4 py-2.5">
                    {grant.constraints &&
                    Object.keys(grant.constraints).length > 0 ? (
                      <ConstraintChips c={grant.constraints} />
                    ) : (
                      <span className="text-xs text-gray-300">—</span>
                    )}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// ── Page ───────────────────────────────────────────────────────────────────────

export default function PermissionSetDetailPage() {
  const { ref } = useParams<{ ref: string }>();

  const { data: permissionSetsRaw, isLoading, error } = usePermissionSets();
  const createRoleAssignment = useCreatePermissionSetRoleAssignment();
  const deleteRoleAssignment = useDeletePermissionSetRoleAssignment();

  const [newRole, setNewRole] = useState("");
  const [showAddRole, setShowAddRole] = useState(false);

  const permissionSets = permissionSetsRaw as
    | PermissionSetWithRoles[]
    | undefined;
  const permissionSet = permissionSets?.find((ps) => ps.ref === ref);

  const handleAddRole = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newRole.trim()) return;
    try {
      await createRoleAssignment.mutateAsync({
        permissionSetId: permissionSet!.id,
        role: newRole.trim(),
      });
      setNewRole("");
      setShowAddRole(false);
    } catch (err) {
      console.error("Failed to add role:", err);
    }
  };

  const handleDeleteRole = async (assignmentId: number, roleName: string) => {
    if (window.confirm(`Remove role "${roleName}" from this permission set?`)) {
      try {
        await deleteRoleAssignment.mutateAsync(assignmentId);
      } catch (err) {
        console.error("Failed to delete role assignment:", err);
      }
    }
  };

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <p className="mt-3 text-sm text-gray-500">
              Loading permission set…
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (error || !permissionSet) {
    return (
      <div className="p-6">
        <Link
          to="/access-control?tab=permission-sets"
          className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800 mb-6"
        >
          <ArrowLeft className="w-4 h-4" />
          Back to Access Control
        </Link>
        <div className="bg-white rounded-lg shadow p-12 text-center">
          <Shield className="mx-auto h-12 w-12 text-gray-400" />
          <p className="mt-4 text-red-600">
            {error
              ? "Failed to load permission set"
              : "Permission set not found"}
          </p>
        </div>
      </div>
    );
  }

  const roles = permissionSet.roles || [];
  const parsedGrants = parseGrants(permissionSet.grants);

  return (
    <div className="p-6">
      {/* Back link */}
      <Link
        to="/access-control?tab=permission-sets"
        className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800 mb-6"
      >
        <ArrowLeft className="w-4 h-4" />
        Back to Access Control
      </Link>

      {/* Header */}
      <div className="bg-white rounded-lg shadow p-6 mb-6">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-4">
            <div className="flex-shrink-0 w-12 h-12 bg-indigo-100 rounded-lg flex items-center justify-center">
              <Shield className="w-6 h-6 text-indigo-600" />
            </div>
            <div>
              <h1 className="text-2xl font-bold text-gray-900 font-mono">
                {permissionSet.ref}
              </h1>
              {permissionSet.label && (
                <p className="text-lg text-gray-700 mt-0.5">
                  {permissionSet.label}
                </p>
              )}
              {permissionSet.description && (
                <p className="text-sm text-gray-500 mt-1">
                  {permissionSet.description}
                </p>
              )}
            </div>
          </div>
          {permissionSet.pack_ref && (
            <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-sm font-medium bg-green-100 text-green-800">
              <Package className="w-3.5 h-3.5" />
              {permissionSet.pack_ref}
            </span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Roles Section */}
        <div className="bg-white rounded-lg shadow">
          <div className="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Tag className="w-5 h-5 text-gray-500" />
              <h2 className="text-lg font-semibold text-gray-900">
                Role Assignments
              </h2>
              <span className="text-sm text-gray-500">({roles.length})</span>
            </div>
            <button
              onClick={() => setShowAddRole(!showAddRole)}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-blue-600 hover:text-blue-800 hover:bg-blue-50 rounded-md transition-colors"
            >
              <Plus className="w-4 h-4" />
              Add Role
            </button>
          </div>

          {showAddRole && (
            <form
              onSubmit={handleAddRole}
              className="px-6 py-3 bg-blue-50 border-b border-blue-100 flex items-center gap-3"
            >
              <input
                type="text"
                value={newRole}
                onChange={(e) => setNewRole(e.target.value)}
                placeholder="Enter role name…"
                className="flex-1 px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                autoFocus
              />
              <button
                type="submit"
                disabled={!newRole.trim() || createRoleAssignment.isPending}
                className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {createRoleAssignment.isPending ? "Adding…" : "Add"}
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowAddRole(false);
                  setNewRole("");
                }}
                className="px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900"
              >
                Cancel
              </button>
            </form>
          )}

          {createRoleAssignment.isError && (
            <div className="px-6 py-2 bg-red-50 border-b border-red-100">
              <p className="text-sm text-red-600">
                Failed to add role.{" "}
                {createRoleAssignment.error instanceof Error
                  ? createRoleAssignment.error.message
                  : "Please try again."}
              </p>
            </div>
          )}

          <div className="divide-y divide-gray-100">
            {roles.length === 0 ? (
              <div className="px-6 py-8 text-center">
                <Tag className="mx-auto h-8 w-8 text-gray-300" />
                <p className="mt-2 text-sm text-gray-500">
                  No roles assigned to this permission set
                </p>
                <p className="text-xs text-gray-400 mt-1">
                  Identities with a matching role will inherit these grants
                </p>
              </div>
            ) : (
              roles.map((assignment) => (
                <div
                  key={assignment.id}
                  className="px-6 py-3 flex items-center justify-between hover:bg-gray-50"
                >
                  <div className="flex items-center gap-3">
                    <span className="inline-flex items-center px-2.5 py-1 rounded-full text-sm font-medium bg-purple-100 text-purple-800">
                      {assignment.role}
                    </span>
                    <span className="text-xs text-gray-400">
                      Added {new Date(assignment.created).toLocaleDateString()}
                    </span>
                  </div>
                  <button
                    onClick={() =>
                      handleDeleteRole(assignment.id, assignment.role)
                    }
                    className="text-red-400 hover:text-red-600 p-1 rounded transition-colors"
                    title="Remove role assignment"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Grants Section */}
        <div className="bg-white rounded-lg shadow">
          <div className="px-6 py-4 border-b border-gray-200 flex items-center gap-2">
            <Shield className="w-5 h-5 text-gray-500" />
            <h2 className="text-lg font-semibold text-gray-900">Grants</h2>
            <span className="text-sm text-gray-500">
              ({parsedGrants.length})
            </span>
          </div>
          <GrantsView grants={parsedGrants} />
        </div>
      </div>
    </div>
  );
}
