import { useState, useCallback, useMemo, useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useQueries } from "@tanstack/react-query";
import {
  ArrowLeft,
  Save,
  AlertTriangle,
  FileCode,
  Code,
  LayoutDashboard,
  X,
  Zap,
  Settings2,
} from "lucide-react";
import SearchableSelect from "@/components/common/SearchableSelect";
import yaml from "js-yaml";
import type { WorkflowYamlDefinition } from "@/types/workflow";
import ActionPalette from "@/components/workflows/ActionPalette";
import WorkflowInputsPanel from "@/components/workflows/WorkflowInputsPanel";
import WorkflowCanvas from "@/components/workflows/WorkflowCanvas";
import type { EdgeHoverInfo } from "@/components/workflows/WorkflowEdges";
import TaskInspector from "@/components/workflows/TaskInspector";
import { useActions } from "@/hooks/useActions";
import { ActionsService } from "@/api";
import { usePacks } from "@/hooks/usePacks";
import { useWorkflow } from "@/hooks/useWorkflows";
import {
  useSaveWorkflowFile,
  useUpdateWorkflowFile,
} from "@/hooks/useWorkflows";
import type {
  WorkflowTask,
  WorkflowBuilderState,
  PaletteAction,
  TransitionPreset,
} from "@/types/workflow";
import {
  generateUniqueTaskName,
  generateTaskId,
  builderStateToDefinition,
  definitionToBuilderState,
  validateWorkflow,
  addTransitionTarget,
  removeTaskFromTransitions,
  renameTaskInTransitions,
  findStartingTaskIds,
} from "@/types/workflow";

const INITIAL_STATE: WorkflowBuilderState = {
  name: "",
  label: "",
  description: "",
  version: "1.0.0",
  packRef: "",
  parameters: {},
  output: {},
  vars: {},
  tasks: [],
  tags: [],
  enabled: true,
};

export default function WorkflowBuilderPage() {
  const navigate = useNavigate();
  const { ref: editRef } = useParams<{ ref?: string }>();
  const isEditing = !!editRef;

  // Data fetching
  const { data: actionsData, isLoading: actionsLoading } = useActions({
    pageSize: 200,
  });
  const { data: packsData } = usePacks({ pageSize: 100 });
  const { data: existingWorkflow, isLoading: workflowLoading } = useWorkflow(
    editRef || "",
  );

  // Mutations
  const saveWorkflowFile = useSaveWorkflowFile();
  const updateWorkflowFile = useUpdateWorkflowFile();

  // Builder state
  const [state, setState] = useState<WorkflowBuilderState>(INITIAL_STATE);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  const [showErrors, setShowErrors] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [initialized, setInitialized] = useState(false);
  const [showYamlPreview, setShowYamlPreview] = useState(false);
  const [sidebarTab, setSidebarTab] = useState<"actions" | "inputs">("actions");
  const [highlightedTransition, setHighlightedTransition] = useState<{
    taskId: string;
    transitionIndex: number;
  } | null>(null);

  // Start-node warning toast state
  const [startWarningVisible, setStartWarningVisible] = useState(false);
  const [startWarningDismissed, setStartWarningDismissed] = useState(false);
  const [showSaveConfirm, setShowSaveConfirm] = useState(false);
  const [prevWarningKey, setPrevWarningKey] = useState<string | null>(null);
  const [justInitialized, setJustInitialized] = useState(false);

  const handleEdgeClick = useCallback(
    (info: EdgeHoverInfo | null) => {
      if (info) {
        // Select the source task so TaskInspector opens for it
        setSelectedTaskId(info.taskId);
        setHighlightedTransition(info);
      } else {
        setHighlightedTransition(null);
      }
    },
    [setSelectedTaskId],
  );

  // Initialize state from existing workflow (edit mode)
  if (isEditing && existingWorkflow && !initialized && !workflowLoading) {
    const workflow = existingWorkflow.data;
    if (workflow) {
      // Extract name from ref (e.g., "pack.name" -> "name")
      const refParts = workflow.ref.split(".");
      const name =
        refParts.length > 1 ? refParts.slice(1).join(".") : workflow.ref;

      const builderState = definitionToBuilderState(
        {
          ref: workflow.ref,
          label: workflow.label,
          description: workflow.description || undefined,
          version: workflow.version,
          parameters: workflow.param_schema || undefined,
          output: workflow.out_schema || undefined,
          tasks:
            ((workflow.definition as Record<string, unknown>)
              ?.tasks as WorkflowYamlDefinition["tasks"]) || [],
          tags: workflow.tags,
        },
        workflow.pack_ref,
        name,
      );
      setState(builderState);
      setInitialized(true);
      setJustInitialized(true);
    }
  }

  // Derived data
  const paletteActions: PaletteAction[] = useMemo(() => {
    const actions = (actionsData?.data || []) as Array<{
      id: number;
      ref: string;
      label: string;
      description?: string;
      pack_ref: string;
    }>;
    return actions.map((a) => ({
      id: a.id,
      ref: a.ref,
      label: a.label,
      description: a.description || "",
      pack_ref: a.pack_ref,
    }));
  }, [actionsData]);

  // Fetch full action details for every unique action ref used in the workflow.
  // React Query caches each response, so repeated refs don't cause extra requests.
  const uniqueActionRefs = useMemo(() => {
    const refs = new Set<string>();
    for (const task of state.tasks) {
      if (task.action) refs.add(task.action);
    }
    return [...refs];
  }, [state.tasks]);

  const actionDetailQueries = useQueries({
    queries: uniqueActionRefs.map((ref) => ({
      queryKey: ["actions", ref],
      queryFn: () => ActionsService.getAction({ ref }),
      staleTime: 30_000,
      enabled: !!ref,
    })),
  });

  // Build action schema map from individually-fetched action details
  const actionSchemaMap = useMemo(() => {
    const map = new Map<string, Record<string, unknown> | null>();
    for (let i = 0; i < uniqueActionRefs.length; i++) {
      const query = actionDetailQueries[i];
      if (query.data?.data) {
        map.set(
          uniqueActionRefs[i],
          (query.data.data.param_schema as Record<string, unknown>) || null,
        );
      }
    }
    return map;
  }, [uniqueActionRefs, actionDetailQueries]);

  const packs = useMemo(() => {
    return (packsData?.data || []) as Array<{
      id: number;
      ref: string;
      label: string;
    }>;
  }, [packsData]);

  const selectedTask = useMemo(
    () => state.tasks.find((t) => t.id === selectedTaskId) || null,
    [state.tasks, selectedTaskId],
  );

  const allTaskNames = useMemo(
    () => state.tasks.map((t) => t.name),
    [state.tasks],
  );

  const startingTaskIds = useMemo(
    () => findStartingTaskIds(state.tasks),
    [state.tasks],
  );

  const startNodeWarning = useMemo(() => {
    if (state.tasks.length === 0) return null;
    const count = startingTaskIds.size;
    if (count === 0)
      return {
        level: "error" as const,
        message:
          "No starting tasks found — every task is a target of another transition, so the workflow has no entry point.",
      };
    if (count > 1)
      return {
        level: "warn" as const,
        message: `${count} starting tasks found (${state.tasks
          .filter((t) => startingTaskIds.has(t.id))
          .map((t) => `"${t.name}"`)
          .join(", ")}). Workflows typically have a single entry point.`,
      };
    return null;
  }, [state.tasks, startingTaskIds]);

  // Render-phase state adjustment: detect warning key changes for immediate
  // show/hide without refs or synchronous setState inside effects.
  const warningKey = startNodeWarning
    ? `${startNodeWarning.level}:${startingTaskIds.size}`
    : null;

  if (warningKey !== prevWarningKey) {
    setPrevWarningKey(warningKey);

    if (!warningKey) {
      // Condition resolved → immediately hide and allow future warnings
      if (startWarningVisible) setStartWarningVisible(false);
      if (startWarningDismissed) setStartWarningDismissed(false);
    } else if (justInitialized) {
      // Loaded from persistent storage with a problem → show immediately
      if (!startWarningVisible) setStartWarningVisible(true);
      setJustInitialized(false);
    }
  }

  // Debounce timer: starts a 15-second countdown for non-initial warning
  // appearances. The timer callback is async so it satisfies React's rules.
  useEffect(() => {
    if (!startNodeWarning || startWarningVisible || startWarningDismissed) {
      return;
    }

    const timer = setTimeout(() => {
      setStartWarningVisible(true);
    }, 15_000);

    return () => clearTimeout(timer);
  }, [startNodeWarning, startWarningVisible, startWarningDismissed]);

  // State updaters
  const updateMetadata = useCallback(
    (updates: Partial<WorkflowBuilderState>) => {
      setState((prev) => ({ ...prev, ...updates }));
      setSaveSuccess(false);
      setSaveError(null);
    },
    [],
  );

  const handleAddTaskFromPalette = useCallback(
    (action: PaletteAction) => {
      // Generate a task name from the action ref
      const baseName = action.ref.split(".").pop() || "task";
      const name = generateUniqueTaskName(state.tasks, baseName);

      // Position below existing tasks
      let maxY = 0;
      for (const task of state.tasks) {
        if (task.position.y > maxY) {
          maxY = task.position.y;
        }
      }

      const newTask: WorkflowTask = {
        id: generateTaskId(),
        name,
        action: action.ref,
        input: {},
        position: {
          x: 300,
          y: state.tasks.length === 0 ? 60 : maxY + 160,
        },
      };

      setState((prev) => ({
        ...prev,
        tasks: [...prev.tasks, newTask],
      }));
      setSelectedTaskId(newTask.id);
      setSaveSuccess(false);
    },
    [state.tasks],
  );

  const handleAddTask = useCallback((task: WorkflowTask) => {
    setState((prev) => ({
      ...prev,
      tasks: [...prev.tasks, task],
    }));
    setSaveSuccess(false);
  }, []);

  const handleUpdateTask = useCallback(
    (taskId: string, updates: Partial<WorkflowTask>) => {
      setState((prev) => {
        // Detect a name change so we can propagate it to transitions
        const oldName =
          updates.name !== undefined
            ? prev.tasks.find((t) => t.id === taskId)?.name
            : undefined;
        const newName = updates.name;
        const isRename =
          oldName !== undefined && newName !== undefined && oldName !== newName;

        return {
          ...prev,
          tasks: prev.tasks.map((t) => {
            if (t.id === taskId) {
              // Apply the updates, then also fix self-referencing transitions
              const merged = { ...t, ...updates };
              if (isRename) {
                const updatedNext = renameTaskInTransitions(
                  merged.next,
                  oldName,
                  newName,
                );
                if (updatedNext !== merged.next) {
                  return { ...merged, next: updatedNext };
                }
              }
              return merged;
            }
            // Update transition `do` lists that reference the old name
            if (isRename) {
              const updatedNext = renameTaskInTransitions(
                t.next,
                oldName,
                newName,
              );
              if (updatedNext !== t.next) {
                return { ...t, next: updatedNext };
              }
            }
            return t;
          }),
        };
      });
      setSaveSuccess(false);
    },
    [],
  );

  const handleDeleteTask = useCallback(
    (taskId: string) => {
      const taskToDelete = state.tasks.find((t) => t.id === taskId);
      if (!taskToDelete) return;

      setState((prev) => ({
        ...prev,
        tasks: prev.tasks
          .filter((t) => t.id !== taskId)
          .map((t) => {
            // Clean up any transitions that reference the deleted task
            const cleanedNext = removeTaskFromTransitions(
              t.next,
              taskToDelete.name,
            );
            if (cleanedNext !== t.next) {
              return { ...t, next: cleanedNext };
            }
            return t;
          }),
      }));

      if (selectedTaskId === taskId) {
        setSelectedTaskId(null);
      }
      setSaveSuccess(false);
    },
    [state.tasks, selectedTaskId],
  );

  const handleSetConnection = useCallback(
    (fromTaskId: string, preset: TransitionPreset, toTaskName: string) => {
      setState((prev) => ({
        ...prev,
        tasks: prev.tasks.map((t) => {
          if (t.id !== fromTaskId) return t;
          const next = addTransitionTarget(t, preset, toTaskName);
          return { ...t, next };
        }),
      }));
      setSaveSuccess(false);
    },
    [],
  );

  const doSave = useCallback(async () => {
    // Validate
    const errors = validateWorkflow(state);
    setValidationErrors(errors);

    if (errors.length > 0) {
      setShowErrors(true);
      return;
    }

    const definition = builderStateToDefinition(state, actionSchemaMap);

    try {
      setSaveError(null);

      if (isEditing && editRef) {
        await updateWorkflowFile.mutateAsync({
          workflowRef: editRef,
          data: {
            name: state.name,
            label: state.label,
            description: state.description || undefined,
            version: state.version,
            pack_ref: state.packRef,
            definition,
            param_schema:
              Object.keys(state.parameters).length > 0
                ? state.parameters
                : undefined,
            out_schema:
              Object.keys(state.output).length > 0 ? state.output : undefined,
            tags: state.tags.length > 0 ? state.tags : undefined,
            enabled: state.enabled,
          },
        });
      } else {
        const fileData = {
          name: state.name,
          label: state.label,
          description: state.description || undefined,
          version: state.version,
          pack_ref: state.packRef,
          definition,
          param_schema:
            Object.keys(state.parameters).length > 0
              ? state.parameters
              : undefined,
          out_schema:
            Object.keys(state.output).length > 0 ? state.output : undefined,
          tags: state.tags.length > 0 ? state.tags : undefined,
          enabled: state.enabled,
        };
        try {
          await saveWorkflowFile.mutateAsync(fileData);
        } catch (createErr: unknown) {
          const apiErr = createErr as { status?: number };
          if (apiErr?.status === 409) {
            // Workflow already exists — fall back to update
            const workflowRef = `${state.packRef}.${state.name}`;
            await updateWorkflowFile.mutateAsync({
              workflowRef,
              data: fileData,
            });
          } else {
            throw createErr;
          }
        }
      }

      // After a successful first save, navigate to the edit URL so the
      // page transitions into edit mode (locks ref, uses update on next save).
      if (!isEditing) {
        const newRef = `${state.packRef}.${state.name}`;
        navigate(`/actions/workflows/${newRef}/edit`, { replace: true });
        return;
      }

      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
    } catch (err: unknown) {
      const error = err as { body?: { message?: string }; message?: string };
      const message =
        error?.body?.message || error?.message || "Failed to save workflow";
      setSaveError(message);
    }
  }, [
    state,
    isEditing,
    editRef,
    saveWorkflowFile,
    updateWorkflowFile,
    actionSchemaMap,
    navigate,
  ]);

  const handleSave = useCallback(() => {
    // If there's a start-node problem, show the toast immediately and
    // require confirmation before saving
    if (startNodeWarning) {
      setStartWarningVisible(true);
      setStartWarningDismissed(false);
      setShowSaveConfirm(true);
      return;
    }
    doSave();
  }, [startNodeWarning, doSave]);

  // YAML preview — generate proper YAML from builder state
  const yamlPreview = useMemo(() => {
    if (!showYamlPreview) return "";
    try {
      const definition = builderStateToDefinition(state, actionSchemaMap);
      return yaml.dump(definition, {
        indent: 2,
        lineWidth: 120,
        noRefs: true,
        sortKeys: false,
        quotingType: '"',
        forceQuotes: false,
      });
    } catch {
      return "# Error generating YAML preview";
    }
  }, [state, showYamlPreview, actionSchemaMap]);

  const isSaving = saveWorkflowFile.isPending || updateWorkflowFile.isPending;

  if (isEditing && workflowLoading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600" />
      </div>
    );
  }

  return (
    <div className="h-[calc(100vh-4rem)] flex flex-col overflow-hidden">
      {/* Top toolbar */}
      <div className="flex-shrink-0 bg-white border-b border-gray-200 px-4 py-2.5">
        <div className="flex items-center justify-between">
          {/* Left section: Back + metadata */}
          <div className="flex items-center gap-3 flex-1 min-w-0">
            <button
              onClick={() =>
                navigate(isEditing ? `/actions/${editRef}` : "/actions")
              }
              className="p-1.5 rounded hover:bg-gray-100 text-gray-500 hover:text-gray-700 transition-colors flex-shrink-0"
              title={isEditing ? "Back to Workflow" : "Back to Actions"}
            >
              <ArrowLeft className="w-5 h-5" />
            </button>

            <div className="flex items-center gap-2 flex-1 min-w-0">
              {/* Pack selector */}
              <SearchableSelect
                value={state.packRef}
                onChange={(v) => updateMetadata({ packRef: String(v) })}
                options={packs.map((pack) => ({
                  value: pack.ref,
                  label: pack.ref,
                }))}
                placeholder="Pack..."
                className="max-w-[140px]"
                disabled={isEditing}
              />

              <span className="text-gray-400 text-lg font-light">/</span>

              {/* Workflow name */}
              <input
                type="text"
                value={state.name}
                onChange={(e) =>
                  updateMetadata({
                    name: e.target.value.replace(/[^a-zA-Z0-9_-]/g, "_"),
                  })
                }
                className={`px-2 py-1.5 border border-gray-300 rounded text-sm font-mono w-48 ${isEditing ? "bg-gray-100 cursor-not-allowed text-gray-500" : "focus:ring-2 focus:ring-blue-500 focus:border-blue-500"}`}
                placeholder="workflow_name"
                disabled={isEditing}
              />

              <span className="text-gray-400 text-lg font-light">—</span>

              {/* Label */}
              <input
                type="text"
                value={state.label}
                onChange={(e) => updateMetadata({ label: e.target.value })}
                className="px-2 py-1.5 border border-gray-300 rounded text-sm focus:ring-2 focus:ring-blue-500 focus:border-blue-500 flex-1 min-w-[160px] max-w-[300px]"
                placeholder="Workflow Label"
              />

              {/* Version */}
              <input
                type="text"
                value={state.version}
                onChange={(e) => updateMetadata({ version: e.target.value })}
                className="px-2 py-1.5 border border-gray-300 rounded text-sm font-mono focus:ring-2 focus:ring-blue-500 focus:border-blue-500 w-20"
                placeholder="1.0.0"
              />
            </div>
          </div>

          {/* Right section: Actions */}
          <div className="flex items-center gap-2 flex-shrink-0 ml-4">
            {/* Validation errors badge */}
            {validationErrors.length > 0 && (
              <button
                onClick={() => setShowErrors(!showErrors)}
                className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium text-amber-700 bg-amber-50 border border-amber-200 rounded hover:bg-amber-100 transition-colors"
              >
                <AlertTriangle className="w-3.5 h-3.5" />
                {validationErrors.length} issue
                {validationErrors.length !== 1 ? "s" : ""}
              </button>
            )}

            {/* Raw YAML / Visual mode toggle */}
            <div className="flex items-center bg-gray-100 rounded-lg p-0.5">
              <button
                onClick={() => setShowYamlPreview(false)}
                className={`flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
                  !showYamlPreview
                    ? "bg-white text-gray-900 shadow-sm"
                    : "text-gray-500 hover:text-gray-700"
                }`}
                title="Visual builder"
              >
                <LayoutDashboard className="w-3.5 h-3.5" />
                Visual
              </button>
              <button
                onClick={() => setShowYamlPreview(true)}
                className={`flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
                  showYamlPreview
                    ? "bg-white text-gray-900 shadow-sm"
                    : "text-gray-500 hover:text-gray-700"
                }`}
                title="Raw YAML view"
              >
                <Code className="w-3.5 h-3.5" />
                Raw YAML
              </button>
            </div>

            {/* Save success indicator */}
            {saveSuccess && (
              <span className="text-xs text-green-600 font-medium">
                ✓ Saved
              </span>
            )}

            {/* Save error indicator */}
            {saveError && (
              <span
                className="text-xs text-red-600 font-medium max-w-[200px] truncate"
                title={saveError}
              >
                ✗ {saveError}
              </span>
            )}

            {/* Save button */}
            <button
              onClick={handleSave}
              disabled={isSaving}
              className="flex items-center gap-1.5 px-4 py-1.5 bg-blue-600 text-white text-sm font-medium rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors shadow-sm"
            >
              <Save className="w-4 h-4" />
              {isSaving ? "Saving..." : isEditing ? "Update" : "Save"}
            </button>
          </div>
        </div>

        {/* Description row (collapsible) */}
        <div className="mt-2 flex items-center gap-2">
          <input
            type="text"
            value={state.description}
            onChange={(e) => updateMetadata({ description: e.target.value })}
            className="flex-1 px-2 py-1 border border-gray-200 rounded text-xs text-gray-600 focus:ring-1 focus:ring-blue-500 focus:border-blue-500"
            placeholder="Workflow description (optional)"
          />
          <div className="flex items-center gap-1.5 flex-shrink-0">
            <input
              type="text"
              value={state.tags.join(", ")}
              onChange={(e) =>
                updateMetadata({
                  tags: e.target.value
                    .split(",")
                    .map((t) => t.trim())
                    .filter(Boolean),
                })
              }
              className="px-2 py-1 border border-gray-200 rounded text-xs text-gray-600 focus:ring-1 focus:ring-blue-500 focus:border-blue-500 w-40"
              placeholder="Tags (comma-sep)"
            />
            <label className="flex items-center gap-1 text-xs text-gray-600">
              <input
                type="checkbox"
                checked={state.enabled}
                onChange={(e) => updateMetadata({ enabled: e.target.checked })}
                className="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
              />
              Enabled
            </label>
          </div>
        </div>
      </div>

      {/* Validation errors panel */}
      {showErrors && validationErrors.length > 0 && (
        <div className="flex-shrink-0 bg-amber-50 border-b border-amber-200 px-4 py-2">
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 text-amber-600 mt-0.5 flex-shrink-0" />
            <div className="flex-1">
              <p className="text-xs font-medium text-amber-800 mb-1">
                Please fix the following issues before saving:
              </p>
              <ul className="text-xs text-amber-700 space-y-0.5">
                {validationErrors.map((error, index) => (
                  <li key={index}>• {error}</li>
                ))}
              </ul>
            </div>
            <button
              onClick={() => setShowErrors(false)}
              className="text-amber-400 hover:text-amber-600"
            >
              ×
            </button>
          </div>
        </div>
      )}

      {/* Main content area */}
      <div className="flex-1 flex overflow-hidden">
        {showYamlPreview ? (
          /* Raw YAML mode — full-width YAML view */
          <div className="flex-1 flex flex-col overflow-hidden bg-gray-900">
            <div className="flex items-center gap-2 px-4 py-2 bg-gray-800 border-b border-gray-700 flex-shrink-0">
              <FileCode className="w-4 h-4 text-gray-400" />
              <span className="text-sm font-medium text-gray-300">
                Workflow Definition
              </span>
              <span className="text-[10px] text-gray-500 ml-1">
                (read-only preview of the generated YAML)
              </span>
            </div>
            <pre className="flex-1 overflow-auto p-6 text-sm font-mono text-green-400 whitespace-pre leading-relaxed">
              {yamlPreview}
            </pre>
          </div>
        ) : (
          <>
            {/* Left sidebar: tabbed Actions / Inputs */}
            <div className="w-64 border-r border-gray-200 bg-gray-50 flex flex-col h-full overflow-hidden">
              {/* Tab header */}
              <div className="flex border-b border-gray-200 bg-white flex-shrink-0">
                <button
                  onClick={() => setSidebarTab("actions")}
                  className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors ${
                    sidebarTab === "actions"
                      ? "text-blue-600 border-b-2 border-blue-600 bg-blue-50/50"
                      : "text-gray-500 hover:text-gray-700 hover:bg-gray-50"
                  }`}
                >
                  <Zap className="w-3.5 h-3.5" />
                  Actions
                </button>
                <button
                  onClick={() => setSidebarTab("inputs")}
                  className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors ${
                    sidebarTab === "inputs"
                      ? "text-blue-600 border-b-2 border-blue-600 bg-blue-50/50"
                      : "text-gray-500 hover:text-gray-700 hover:bg-gray-50"
                  }`}
                >
                  <Settings2 className="w-3.5 h-3.5" />
                  Inputs
                  {Object.keys(state.parameters).length > 0 && (
                    <span className="text-[10px] bg-blue-100 text-blue-700 px-1.5 py-0.5 rounded-full">
                      {Object.keys(state.parameters).length}
                    </span>
                  )}
                </button>
              </div>

              {/* Tab content */}
              {sidebarTab === "actions" ? (
                <ActionPalette
                  actions={paletteActions}
                  isLoading={actionsLoading}
                  onAddTask={handleAddTaskFromPalette}
                />
              ) : (
                <WorkflowInputsPanel
                  parameters={state.parameters}
                  output={state.output}
                  onParametersChange={(parameters) =>
                    setState((prev) => ({ ...prev, parameters }))
                  }
                  onOutputChange={(output) =>
                    setState((prev) => ({ ...prev, output }))
                  }
                />
              )}
            </div>

            {/* Center: Canvas */}
            <WorkflowCanvas
              tasks={state.tasks}
              selectedTaskId={selectedTaskId}
              onSelectTask={setSelectedTaskId}
              onUpdateTask={handleUpdateTask}
              onDeleteTask={handleDeleteTask}
              onAddTask={handleAddTask}
              onSetConnection={handleSetConnection}
              onEdgeClick={handleEdgeClick}
            />

            {/* Right: Task Inspector */}
            {selectedTask && (
              <TaskInspector
                task={selectedTask}
                allTaskNames={allTaskNames}
                availableActions={paletteActions}
                onUpdate={handleUpdateTask}
                onClose={() => setSelectedTaskId(null)}
                highlightTransitionIndex={
                  highlightedTransition?.taskId === selectedTask.id
                    ? highlightedTransition.transitionIndex
                    : null
                }
              />
            )}
          </>
        )}
      </div>

      {/* Floating start-node warning toast */}
      {startNodeWarning && startWarningVisible && (
        <div
          className="fixed top-20 left-1/2 -translate-x-1/2 z-50 animate-fade-in"
          style={{
            animation: "fadeInDown 0.25s ease-out both",
          }}
        >
          <div
            className={`flex items-center gap-2.5 px-4 py-2.5 rounded-lg shadow-lg border ${
              startNodeWarning.level === "error"
                ? "bg-red-50 border-red-300 text-red-800"
                : "bg-amber-50 border-amber-300 text-amber-800"
            }`}
            style={{ maxWidth: 520 }}
          >
            <AlertTriangle
              className={`w-4 h-4 flex-shrink-0 ${
                startNodeWarning.level === "error"
                  ? "text-red-500"
                  : "text-amber-500"
              }`}
            />
            <p className="text-xs font-medium flex-1">
              {startNodeWarning.message}
            </p>
            <button
              onClick={() => {
                setStartWarningVisible(false);
                setStartWarningDismissed(true);
              }}
              className={`p-0.5 rounded hover:bg-black/5 flex-shrink-0 ${
                startNodeWarning.level === "error"
                  ? "text-red-400 hover:text-red-600"
                  : "text-amber-400 hover:text-amber-600"
              }`}
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      )}

      {/* Confirmation modal for saving with start-node warnings */}
      {showSaveConfirm && startNodeWarning && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center">
          {/* Backdrop */}
          <div
            className="absolute inset-0 bg-black/40"
            onClick={() => setShowSaveConfirm(false)}
          />
          {/* Modal */}
          <div className="relative bg-white rounded-lg shadow-xl border border-gray-200 p-6 max-w-md w-full mx-4">
            <div className="flex items-start gap-3">
              <div
                className={`p-2 rounded-full flex-shrink-0 ${
                  startNodeWarning.level === "error"
                    ? "bg-red-100"
                    : "bg-amber-100"
                }`}
              >
                <AlertTriangle
                  className={`w-5 h-5 ${
                    startNodeWarning.level === "error"
                      ? "text-red-600"
                      : "text-amber-600"
                  }`}
                />
              </div>
              <div className="flex-1">
                <h3 className="text-sm font-semibold text-gray-900 mb-1">
                  {startNodeWarning.level === "error"
                    ? "No starting tasks"
                    : "Multiple starting tasks"}
                </h3>
                <p className="text-xs text-gray-600 mb-4">
                  {startNodeWarning.message} Are you sure you want to save this
                  workflow?
                </p>
                <div className="flex items-center justify-end gap-2">
                  <button
                    onClick={() => setShowSaveConfirm(false)}
                    className="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-100 rounded hover:bg-gray-200 transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={() => {
                      setShowSaveConfirm(false);
                      doSave();
                    }}
                    className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded hover:bg-blue-700 transition-colors"
                  >
                    Save Anyway
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Inline style for fade-in animation */}
      <style>{`
          @keyframes fadeInDown {
            from {
              opacity: 0;
              transform: translate(-50%, -8px);
            }
            to {
              opacity: 1;
              transform: translate(-50%, 0);
            }
          }
        `}</style>
    </div>
  );
}
