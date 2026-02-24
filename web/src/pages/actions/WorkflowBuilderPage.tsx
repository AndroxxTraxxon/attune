import { useState, useCallback, useMemo, useRef } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Save,
  AlertTriangle,
  FileCode,
  Code,
  LayoutDashboard,
} from "lucide-react";
import yaml from "js-yaml";
import type { WorkflowYamlDefinition } from "@/types/workflow";
import ActionPalette from "@/components/workflows/ActionPalette";
import WorkflowCanvas from "@/components/workflows/WorkflowCanvas";
import type { EdgeHoverInfo } from "@/components/workflows/WorkflowEdges";
import TaskInspector from "@/components/workflows/TaskInspector";
import { useActions } from "@/hooks/useActions";
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
  const [highlightedTransition, setHighlightedTransition] = useState<{
    taskId: string;
    transitionIndex: number;
  } | null>(null);
  const highlightTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );

  const handleEdgeHover = useCallback(
    (info: EdgeHoverInfo | null) => {
      // Clear any pending auto-clear timeout
      if (highlightTimeoutRef.current) {
        clearTimeout(highlightTimeoutRef.current);
        highlightTimeoutRef.current = null;
      }

      if (info) {
        // Select the source task so TaskInspector opens for it
        setSelectedTaskId(info.taskId);
        setHighlightedTransition(info);

        // Auto-clear highlight after 2 seconds so the flash animation plays once
        highlightTimeoutRef.current = setTimeout(() => {
          setHighlightedTransition(null);
          highlightTimeoutRef.current = null;
        }, 2000);
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
      param_schema?: Record<string, unknown> | null;
      out_schema?: Record<string, unknown> | null;
    }>;
    return actions.map((a) => ({
      id: a.id,
      ref: a.ref,
      label: a.label,
      description: a.description || "",
      pack_ref: a.pack_ref,
      param_schema: a.param_schema || null,
      out_schema: a.out_schema || null,
    }));
  }, [actionsData]);

  // Build action schema map for stripping defaults during serialization
  const actionSchemaMap = useMemo(() => {
    const map = new Map<string, Record<string, unknown> | null>();
    for (const action of paletteActions) {
      map.set(action.ref, action.param_schema);
    }
    return map;
  }, [paletteActions]);

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

      // Pre-populate input from action's param_schema
      const input: Record<string, unknown> = {};
      if (action.param_schema && typeof action.param_schema === "object") {
        for (const [key, param] of Object.entries(action.param_schema)) {
          const meta = param as { default?: unknown };
          input[key] = meta?.default !== undefined ? meta.default : "";
        }
      }

      const newTask: WorkflowTask = {
        id: generateTaskId(),
        name,
        action: action.ref,
        input,
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
      setState((prev) => ({
        ...prev,
        tasks: prev.tasks.map((t) =>
          t.id === taskId ? { ...t, ...updates } : t,
        ),
      }));
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

  const handleSave = useCallback(async () => {
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
        await saveWorkflowFile.mutateAsync({
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
        });
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
  ]);

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
              onClick={() => navigate("/actions")}
              className="p-1.5 rounded hover:bg-gray-100 text-gray-500 hover:text-gray-700 transition-colors flex-shrink-0"
              title="Back to Actions"
            >
              <ArrowLeft className="w-5 h-5" />
            </button>

            <div className="flex items-center gap-2 flex-1 min-w-0">
              {/* Pack selector */}
              <select
                value={state.packRef}
                onChange={(e) => updateMetadata({ packRef: e.target.value })}
                className="px-2 py-1.5 border border-gray-300 rounded text-sm focus:ring-2 focus:ring-blue-500 focus:border-blue-500 max-w-[140px]"
              >
                <option value="">Pack...</option>
                {packs.map((pack) => (
                  <option key={pack.id} value={pack.ref}>
                    {pack.ref}
                  </option>
                ))}
              </select>

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
                className="px-2 py-1.5 border border-gray-300 rounded text-sm font-mono focus:ring-2 focus:ring-blue-500 focus:border-blue-500 w-48"
                placeholder="workflow_name"
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
            {/* Left: Action Palette */}
            <ActionPalette
              actions={paletteActions}
              isLoading={actionsLoading}
              onAddTask={handleAddTaskFromPalette}
            />

            {/* Center: Canvas */}
            <WorkflowCanvas
              tasks={state.tasks}
              selectedTaskId={selectedTaskId}
              availableActions={paletteActions}
              onSelectTask={setSelectedTaskId}
              onUpdateTask={handleUpdateTask}
              onDeleteTask={handleDeleteTask}
              onAddTask={handleAddTask}
              onSetConnection={handleSetConnection}
              onEdgeHover={handleEdgeHover}
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
    </div>
  );
}
