/**
 * Workflow Builder Types
 *
 * These types represent the client-side workflow builder state
 * and map to the backend workflow YAML format.
 *
 * Uses the Orquesta-style task transition model where each task has a `next`
 * list of transitions. Each transition specifies:
 *   - `when` — a condition expression (e.g., "{{ succeeded() }}", "{{ failed() }}")
 *   - `publish` — variables to publish into the workflow context
 *   - `do` — next tasks to invoke when the condition is met
 */

/** Position of a node on the canvas */
export interface NodePosition {
  x: number;
  y: number;
}

/**
 * A single task transition evaluated after task completion.
 *
 * Transitions are evaluated in order. When `when` is not defined,
 * the transition is unconditional (fires on any completion).
 */
/** Line style for transition edges */
export type LineStyle = "solid" | "dashed" | "dotted" | "dash-dot";

export interface TaskTransition {
  /** Condition expression (e.g., "{{ succeeded() }}", "{{ failed() }}") */
  when?: string;
  /** Variables to publish into the workflow context on this transition */
  publish?: PublishDirective[];
  /** Next tasks to invoke when transition criteria is met */
  do?: string[];
  /** Custom display label for the transition (overrides auto-derived label) */
  label?: string;
  /** Custom color for the transition edge (CSS color string, e.g., "#ff6600") */
  color?: string;
  /** Custom line style for the transition edge (overrides type-based default) */
  line_style?: LineStyle;
  /** Intermediate waypoints per target task (keyed by target task name) for edge routing */
  edge_waypoints?: Record<string, NodePosition[]>;
  /** Label position per target task as t-parameter (0–1) along the edge path */
  label_positions?: Record<string, number>;
}

/** A task node in the workflow builder */
export interface WorkflowTask {
  /** Unique ID for the builder (not persisted) */
  id: string;
  /** Task name (used in YAML) */
  name: string;
  /** Action reference (e.g., "core.echo") */
  action: string;
  /** Input parameters (template strings or values) */
  input: Record<string, unknown>;
  /** Task transitions — evaluated in order after task completes */
  next?: TaskTransition[];
  /** Delay in seconds before executing this task */
  delay?: number;
  /** Retry configuration */
  retry?: RetryConfig;
  /** Timeout in seconds */
  timeout?: number;
  /** With-items iteration expression */
  with_items?: string;
  /** Batch size for with-items */
  batch_size?: number;
  /** Concurrency limit for with-items */
  concurrency?: number;
  /** Join barrier count */
  join?: number;
  /** Visual position on canvas */
  position: NodePosition;
}

/** Retry configuration */
export interface RetryConfig {
  /** Number of retry attempts */
  count: number;
  /** Initial delay in seconds */
  delay: number;
  /** Backoff strategy */
  backoff?: "constant" | "linear" | "exponential";
  /** Maximum delay in seconds */
  max_delay?: number;
  /** Only retry on specific error conditions */
  on_error?: string;
}

/** Variable publishing directive */
export type PublishDirective = Record<string, string>;

/**
 * Transition handle presets for the visual builder.
 *
 * These map to common `when` expressions and provide a quick way
 * to create transitions without typing expressions manually.
 */
export type TransitionPreset = "succeeded" | "failed" | "always";

/** The `when` expression for each preset (undefined = unconditional) */
export const PRESET_WHEN: Record<TransitionPreset, string | undefined> = {
  succeeded: "{{ succeeded() }}",
  failed: "{{ failed() }}",
  always: undefined,
};

/** Human-readable labels for presets */
export const PRESET_LABELS: Record<TransitionPreset, string> = {
  succeeded: "On Success",
  failed: "On Failure",
  always: "Always",
};

/** Default edge colors for each preset */
export const PRESET_COLORS: Record<TransitionPreset, string> = {
  succeeded: "#22c55e", // green-500
  failed: "#ef4444", // red-500
  always: "#6b7280", // gray-500
};

export const PRESET_STYLES: Record<TransitionPreset, LineStyle> = {
  succeeded: "solid",
  failed: "dashed",
  always: "solid",
};

/**
 * Classify a `when` expression into an edge visual type.
 * Used for edge coloring and labeling.
 */
export type EdgeType = "success" | "failure" | "complete" | "custom";

/** Default colors for each EdgeType (mirrors PRESET_COLORS but keyed by EdgeType). */
export const EDGE_TYPE_COLORS: Record<EdgeType, string> = {
  success: "#22c55e", // green-500
  failure: "#ef4444", // red-500
  complete: "#6b7280", // gray-500 (unconditional / always)
  custom: "#8b5cf6", // violet-500
};

export function classifyTransitionWhen(when?: string): EdgeType {
  if (!when) return "complete"; // unconditional
  const lower = when.toLowerCase().replace(/\s+/g, "");
  if (lower.includes("succeeded()")) return "success";
  if (lower.includes("failed()")) return "failure";
  return "custom";
}

/** Human-readable short label for a `when` expression */
export function transitionLabel(when?: string, customLabel?: string): string {
  if (customLabel) return customLabel;
  if (!when) return "always";
  const lower = when.toLowerCase().replace(/\s+/g, "");
  if (lower.includes("succeeded()")) return "succeeded";
  if (lower.includes("failed()")) return "failed";
  // Truncate custom expressions for display
  if (when.length > 30) return when.slice(0, 27) + "...";
  return when;
}

/** An edge/connection between two tasks */
export interface WorkflowEdge {
  /** Source task ID */
  from: string;
  /** Target task ID */
  to: string;
  /** Target task name (stable key for waypoints) */
  toName: string;
  /** Visual type of transition (derived from `when`) */
  type: EdgeType;
  /** Label to display on the edge */
  label?: string;
  /** Index of the transition in the source task's `next` array */
  transitionIndex: number;
  /** Custom color override for the edge (CSS color string) */
  color?: string;
  /** Custom line style override for the edge */
  lineStyle?: LineStyle;
  /** Intermediate waypoints for this specific edge */
  waypoints?: NodePosition[];
  /** Label position as t-parameter (0–1) along the edge path; default 0.5 */
  labelPosition?: number;
}

/**
 * Cancellation policy for a workflow.
 *
 * Controls what happens to running tasks when a workflow is cancelled:
 * - `allow_finish` (default): Running tasks complete naturally; only
 *   pending/requested tasks are cancelled and no new tasks are dispatched.
 * - `cancel_running`: All running and pending tasks are forcefully cancelled.
 *   Running processes receive SIGINT → SIGTERM → SIGKILL via the worker.
 */
export type CancellationPolicy = "allow_finish" | "cancel_running";

/** Human-readable labels for each cancellation policy */
export const CANCELLATION_POLICY_LABELS: Record<CancellationPolicy, string> = {
  allow_finish: "Allow running tasks to finish",
  cancel_running: "Cancel running tasks",
};

/** Complete workflow builder state */
export interface WorkflowBuilderState {
  /** Workflow name (used to derive ref and filename) */
  name: string;
  /** Human-readable label */
  label: string;
  /** Description */
  description: string;
  /** Semantic version */
  version: string;
  /** Pack reference this workflow belongs to */
  packRef: string;
  /** Input parameter schema (flat format) */
  parameters: Record<string, ParamDefinition>;
  /** Output schema (flat format) */
  output: Record<string, ParamDefinition>;
  /** Workflow-scoped variables */
  vars: Record<string, unknown>;
  /** Task nodes */
  tasks: WorkflowTask[];
  /** Tags */
  tags: string[];
  /** Cancellation policy (default: allow_finish) */
  cancellationPolicy: CancellationPolicy;
}

/** Parameter definition in flat schema format */
export interface ParamDefinition {
  type: string;
  description?: string;
  required?: boolean;
  secret?: boolean;
  default?: unknown;
  enum?: string[];
  [key: string]: unknown;
}

/** Workflow definition as stored in the YAML file / API */
/**
 * Full workflow definition — used for DB storage and the save API payload.
 * Contains both action-level metadata AND the execution graph.
 */
export interface WorkflowYamlDefinition {
  ref: string;
  label: string;
  description?: string;
  version: string;
  parameters?: Record<string, unknown>;
  output?: Record<string, unknown>;
  vars?: Record<string, unknown>;
  tasks: WorkflowYamlTask[];
  output_map?: Record<string, string>;
  tags?: string[];
  cancellation_policy?: CancellationPolicy;
}

/**
 * Graph-only workflow definition — written to the `.workflow.yaml` file on disk.
 *
 * Action-linked workflow files contain only the execution graph. The companion
 * action YAML (`actions/{name}.yaml`) is authoritative for `ref`, `label`,
 * `description`, `parameters`, `output`, and `tags`.
 */
export interface WorkflowGraphDefinition {
  version: string;
  vars?: Record<string, unknown>;
  tasks: WorkflowYamlTask[];
  output_map?: Record<string, string>;
  cancellation_policy?: CancellationPolicy;
}

/**
 * Action YAML definition — written to the companion `actions/{name}.yaml` file.
 *
 * Controls the action's identity and exposed interface. References the workflow
 * file via `workflow_file`.
 */
export interface ActionYamlDefinition {
  ref: string;
  label: string;
  description?: string;
  workflow_file: string;
  parameters?: Record<string, unknown>;
  output?: Record<string, unknown>;
  tags?: string[];
}

/** Chart-only metadata for a transition edge (not consumed by the backend) */
export interface TransitionChartMeta {
  /** Custom display label for the transition */
  label?: string;
  /** Custom color for the transition edge (CSS color string) */
  color?: string;
  /** Custom line style for the transition edge */
  line_style?: LineStyle;
  /** Intermediate waypoints per target task (keyed by target task name) */
  edge_waypoints?: Record<string, NodePosition[]>;
  /** Label position per target task as t-parameter (0–1) along the edge path */
  label_positions?: Record<string, number>;
}

/** Transition as represented in YAML format */
export interface WorkflowYamlTransition {
  when?: string;
  publish?: PublishDirective[];
  do?: string[];
  /** Visual metadata (label, color, line style, waypoints) — ignored by backend */
  __chart_meta__?: TransitionChartMeta;
}

/** Chart-only metadata for a task node (not consumed by the backend) */
export interface TaskChartMeta {
  /** Visual position on the canvas */
  position?: NodePosition;
}

/** Task as represented in YAML format */
export interface WorkflowYamlTask {
  name: string;
  action?: string;
  input?: Record<string, unknown>;
  delay?: number;
  with_items?: string;
  batch_size?: number;
  concurrency?: number;
  retry?: RetryConfig;
  timeout?: number;
  next?: WorkflowYamlTransition[];
  join?: number;
  /** Visual metadata (position) — ignored by backend */
  __chart_meta__?: TaskChartMeta;
}

/** Request to save a workflow file to disk and sync to DB */
export interface SaveWorkflowFileRequest {
  /** Workflow name (becomes filename: {name}.workflow.yaml) */
  name: string;
  /** Human-readable label */
  label: string;
  /** Description */
  description?: string;
  /** Semantic version */
  version: string;
  /** Pack reference */
  pack_ref: string;
  /** The full workflow definition as JSON */
  definition: WorkflowYamlDefinition;
  /** Parameter schema (flat format) */
  param_schema?: Record<string, unknown>;
  /** Output schema (flat format) */
  out_schema?: Record<string, unknown>;
  /** Tags */
  tags?: string[];
}

/** An action summary used in the action palette */
export interface PaletteAction {
  id: number;
  ref: string;
  label: string;
  description: string;
  pack_ref: string;
}

// ---------------------------------------------------------------------------
// Conversion functions
// ---------------------------------------------------------------------------

/**
 * Check if two values are deeply equal for the purpose of default comparison.
 * Handles primitives, arrays, and plain objects.
 */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a !== "object") return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  const aObj = a as Record<string, unknown>;
  const bObj = b as Record<string, unknown>;
  const aKeys = Object.keys(aObj);
  const bKeys = Object.keys(bObj);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => deepEqual(aObj[key], bObj[key]));
}

/**
 * Strip input values that match their schema defaults.
 * Returns a new object containing only user-modified values.
 */
export function stripDefaultInputs(
  input: Record<string, unknown>,
  paramSchema: Record<string, unknown> | null | undefined,
): Record<string, unknown> {
  if (!paramSchema || typeof paramSchema !== "object") return input;
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(input)) {
    const schemaDef = paramSchema[key] as
      | { default?: unknown }
      | null
      | undefined;
    if (
      schemaDef &&
      schemaDef.default !== undefined &&
      deepEqual(value, schemaDef.default)
    ) {
      continue; // skip — matches default
    }
    // Also skip empty strings when there's no default (user never filled it in)
    if (value === "" && (!schemaDef || schemaDef.default === undefined)) {
      continue;
    }
    result[key] = value;
  }
  return result;
}

/**
 * Convert builder state to YAML definition for saving.
 *
 * When `actionSchemas` is provided (a map of action ref → param_schema),
 * input values that match their schema defaults are omitted from the output
 * so only user-modified parameters appear in the generated YAML.
 */
export function builderStateToDefinition(
  state: WorkflowBuilderState,
  actionSchemas?: Map<string, Record<string, unknown> | null>,
): WorkflowYamlDefinition {
  const graph = builderStateToGraph(state, actionSchemas);
  const definition: WorkflowYamlDefinition = {
    ref: `${state.packRef}.${state.name}`,
    label: state.label,
    version: state.version,
    tasks: graph.tasks,
  };

  if (state.description) {
    definition.description = state.description;
  }

  if (Object.keys(state.parameters).length > 0) {
    definition.parameters = state.parameters;
  }

  if (Object.keys(state.output).length > 0) {
    definition.output = state.output;
  }

  if (graph.vars && Object.keys(graph.vars).length > 0) {
    definition.vars = graph.vars;
  }

  if (graph.output_map) {
    definition.output_map = graph.output_map;
  }

  if (state.tags.length > 0) {
    definition.tags = state.tags;
  }

  if (state.cancellationPolicy !== "allow_finish") {
    definition.cancellation_policy = state.cancellationPolicy;
  }

  return definition;
}

/**
 * Extract the graph-only workflow definition from builder state.
 *
 * This produces the content that should be written to the `.workflow.yaml`
 * file on disk — no `ref`, `label`, `description`, `parameters`, `output`,
 * or `tags`. Those belong in the companion action YAML.
 */
export function builderStateToGraph(
  state: WorkflowBuilderState,
  actionSchemas?: Map<string, Record<string, unknown> | null>,
): WorkflowGraphDefinition {
  const tasks: WorkflowYamlTask[] = state.tasks.map((task) => {
    const yamlTask: WorkflowYamlTask = {
      name: task.name,
    };

    if (task.action) {
      yamlTask.action = task.action;
    }

    // Filter input: strip values that match schema defaults
    const schema = actionSchemas?.get(task.action);
    const effectiveInput = schema
      ? stripDefaultInputs(task.input, schema)
      : task.input;
    if (Object.keys(effectiveInput).length > 0) {
      yamlTask.input = effectiveInput;
    }

    if (task.delay) yamlTask.delay = task.delay;
    if (task.with_items) yamlTask.with_items = task.with_items;
    if (task.batch_size) yamlTask.batch_size = task.batch_size;
    if (task.concurrency) yamlTask.concurrency = task.concurrency;
    if (task.retry) yamlTask.retry = task.retry;
    if (task.timeout) yamlTask.timeout = task.timeout;
    if (task.join) yamlTask.join = task.join;

    // Persist canvas position in __chart_meta__ so layout is restored on reload
    yamlTask.__chart_meta__ = {
      position: { x: task.position.x, y: task.position.y },
    };

    // Serialize transitions as `next` array
    if (task.next && task.next.length > 0) {
      yamlTask.next = task.next.map((t) => {
        const yt: WorkflowYamlTransition = {};
        if (t.when) yt.when = t.when;
        if (t.publish && t.publish.length > 0) yt.publish = t.publish;
        if (t.do && t.do.length > 0) yt.do = t.do;
        // Store label/color/line_style/waypoints in __chart_meta__
        const hasChartMeta =
          t.label ||
          t.color ||
          t.line_style ||
          t.edge_waypoints ||
          t.label_positions;
        if (hasChartMeta) {
          yt.__chart_meta__ = {};
          if (t.label) yt.__chart_meta__.label = t.label;
          if (t.color) yt.__chart_meta__.color = t.color;
          if (t.line_style) yt.__chart_meta__.line_style = t.line_style;
          if (t.edge_waypoints && Object.keys(t.edge_waypoints).length > 0) {
            yt.__chart_meta__.edge_waypoints = t.edge_waypoints;
          }
          if (t.label_positions && Object.keys(t.label_positions).length > 0) {
            yt.__chart_meta__.label_positions = t.label_positions;
          }
        }
        return yt;
      });
    }

    return yamlTask;
  });

  const graph: WorkflowGraphDefinition = {
    version: state.version,
    tasks,
  };

  if (Object.keys(state.vars).length > 0) {
    graph.vars = state.vars;
  }

  if (state.cancellationPolicy !== "allow_finish") {
    graph.cancellation_policy = state.cancellationPolicy;
  }

  return graph;
}

/**
 * Extract the action YAML definition from builder state.
 *
 * This produces the content for the companion `actions/{name}.yaml` file
 * that owns action-level metadata and references the workflow file.
 */
export function builderStateToActionYaml(
  state: WorkflowBuilderState,
): ActionYamlDefinition {
  const action: ActionYamlDefinition = {
    ref: `${state.packRef}.${state.name}`,
    label: state.label,
    workflow_file: `workflows/${state.name}.workflow.yaml`,
  };

  if (state.description) {
    action.description = state.description;
  }

  if (Object.keys(state.parameters).length > 0) {
    action.parameters = state.parameters;
  }

  if (Object.keys(state.output).length > 0) {
    action.output = state.output;
  }

  if (state.tags.length > 0) {
    action.tags = state.tags;
  }

  return action;
}

// ---------------------------------------------------------------------------
// Legacy format conversion helpers
// ---------------------------------------------------------------------------

/** Legacy task fields that may appear in older workflow definitions */
interface LegacyYamlTask extends WorkflowYamlTask {
  on_success?: string;
  on_failure?: string;
  on_complete?: string;
  on_timeout?: string;
  decision?: { when?: string; next: string; default?: boolean }[];
  publish?: PublishDirective[];
}

/**
 * Convert legacy on_success/on_failure/etc fields to `next` transitions.
 * This allows the builder to load workflows saved in the old format.
 */
function legacyTransitionsToNext(task: LegacyYamlTask): TaskTransition[] {
  const transitions: TaskTransition[] = [];

  if (task.on_success) {
    transitions.push({
      when: "{{ succeeded() }}",
      do: [task.on_success],
    });
  }

  if (task.on_failure) {
    transitions.push({
      when: "{{ failed() }}",
      do: [task.on_failure],
    });
  }

  if (task.on_complete) {
    // on_complete = unconditional (fires regardless of success/failure)
    transitions.push({
      do: [task.on_complete],
    });
  }

  if (task.on_timeout) {
    transitions.push({
      when: "{{ timed_out() }}",
      do: [task.on_timeout],
    });
  }

  // Convert legacy decision branches
  if (task.decision) {
    for (const branch of task.decision) {
      transitions.push({
        when: branch.when || undefined,
        do: [branch.next],
      });
    }
  }

  // If legacy task had publish but no transitions, create a publish-only transition
  if (task.publish && task.publish.length > 0 && transitions.length === 0) {
    transitions.push({
      when: "{{ succeeded() }}",
      publish: task.publish,
    });
  } else if (
    task.publish &&
    task.publish.length > 0 &&
    transitions.length > 0
  ) {
    // Attach publish to the first succeeded transition, or the first transition
    const succeededIdx = transitions.findIndex(
      (t) => t.when && t.when.toLowerCase().includes("succeeded()"),
    );
    const idx = succeededIdx >= 0 ? succeededIdx : 0;
    transitions[idx].publish = task.publish;
  }

  return transitions;
}

/**
 * Convert a YAML definition back to builder state (for editing existing workflows).
 * Supports both new `next` format and legacy `on_success`/`on_failure` format.
 */
export function definitionToBuilderState(
  definition: WorkflowYamlDefinition,
  packRef: string,
  name: string,
): WorkflowBuilderState {
  const tasks: WorkflowTask[] = (definition.tasks || []).map(
    (rawTask, index) => {
      const task = rawTask as LegacyYamlTask;

      // Determine transitions: prefer `next` if present, otherwise convert legacy fields
      let next: TaskTransition[] | undefined;
      if (task.next && task.next.length > 0) {
        next = task.next.map((t) => ({
          when: t.when,
          publish: t.publish,
          do: t.do,
          label: t.__chart_meta__?.label,
          color: t.__chart_meta__?.color,
          line_style: t.__chart_meta__?.line_style,
          edge_waypoints: t.__chart_meta__?.edge_waypoints,
          label_positions: t.__chart_meta__?.label_positions,
        }));
      } else {
        const converted = legacyTransitionsToNext(task);
        next = converted.length > 0 ? converted : undefined;
      }

      return {
        id: `task-${index}-${Date.now()}`,
        name: task.name,
        action: task.action || "",
        input: task.input || {},
        next,
        delay: task.delay,
        retry: task.retry,
        timeout: task.timeout,
        with_items: task.with_items,
        batch_size: task.batch_size,
        concurrency: task.concurrency,
        join: task.join,
        position: task.__chart_meta__?.position ?? {
          x: 300,
          y: 80 + index * 160,
        },
      };
    },
  );

  return {
    name,
    label: definition.label,
    description: definition.description || "",
    version: definition.version,
    packRef,
    parameters: (definition.parameters || {}) as Record<
      string,
      ParamDefinition
    >,
    output: (definition.output || {}) as Record<string, ParamDefinition>,
    vars: definition.vars || {},
    tasks,
    tags: definition.tags || [],
    cancellationPolicy: definition.cancellation_policy || "allow_finish",
  };
}

// ---------------------------------------------------------------------------
// Edge derivation
// ---------------------------------------------------------------------------

/**
 * Derive visual edges from task transitions.
 *
 * Each entry in a task's `next` array can target multiple tasks via `do`.
 * Each target produces a separate edge with the same visual type/label.
 */
export function deriveEdges(tasks: WorkflowTask[]): WorkflowEdge[] {
  const edges: WorkflowEdge[] = [];
  const taskNameToId = new Map<string, string>();

  for (const task of tasks) {
    taskNameToId.set(task.name, task.id);
  }

  for (const task of tasks) {
    if (!task.next) continue;

    for (let ti = 0; ti < task.next.length; ti++) {
      const transition = task.next[ti];
      const edgeType = classifyTransitionWhen(transition.when);
      const label = transitionLabel(transition.when, transition.label);

      if (transition.do) {
        for (const targetName of transition.do) {
          const targetId = taskNameToId.get(targetName);
          if (targetId) {
            edges.push({
              from: task.id,
              to: targetId,
              toName: targetName,
              type: edgeType,
              label,
              transitionIndex: ti,
              color: transition.color,
              lineStyle: transition.line_style,
              waypoints: transition.edge_waypoints?.[targetName],
              labelPosition: transition.label_positions?.[targetName],
            });
          }
        }
      }
    }
  }

  return edges;
}

// ---------------------------------------------------------------------------
// Task transition helpers
// ---------------------------------------------------------------------------

/**
 * Find or create a transition in a task's `next` array that matches a preset.
 *
 * If a transition with a matching `when` expression already exists, returns
 * its index. Otherwise, appends a new transition and returns the new index.
 */
export function findOrCreateTransition(
  task: WorkflowTask,
  preset: TransitionPreset,
): { next: TaskTransition[]; index: number } {
  const whenExpr = PRESET_WHEN[preset];
  const next = [...(task.next || [])];

  // Look for an existing transition with the same `when`
  const existingIndex = next.findIndex((t) => {
    if (whenExpr === undefined) return t.when === undefined;
    return (
      t.when?.toLowerCase().replace(/\s+/g, "") ===
      whenExpr.toLowerCase().replace(/\s+/g, "")
    );
  });

  if (existingIndex >= 0) {
    return { next, index: existingIndex };
  }

  // Create new transition with default label, color, and line style for the preset
  const newTransition: TaskTransition = {
    label: PRESET_LABELS[preset],
    color: PRESET_COLORS[preset],
    line_style: PRESET_STYLES[preset],
  };
  if (whenExpr) newTransition.when = whenExpr;
  next.push(newTransition);
  return { next, index: next.length - 1 };
}

/**
 * Add a target task to a transition's `do` list.
 * If the target is already in the list, this is a no-op.
 * Returns the updated `next` array.
 */
export function addTransitionTarget(
  task: WorkflowTask,
  preset: TransitionPreset,
  targetTaskName: string,
): TaskTransition[] {
  const { next, index } = findOrCreateTransition(task, preset);
  const transition = { ...next[index] };
  const doList = [...(transition.do || [])];

  if (!doList.includes(targetTaskName)) {
    doList.push(targetTaskName);
  }

  transition.do = doList;
  next[index] = transition;
  return next;
}

/**
 * Remove all references to a task name from all transitions.
 * Cleans up transitions that become empty (no `do` and no `publish`).
 */
export function removeTaskFromTransitions(
  next: TaskTransition[] | undefined,
  taskName: string,
): TaskTransition[] | undefined {
  if (!next) return undefined;

  const cleaned = next
    .map((t) => {
      if (!t.do || !t.do.includes(taskName)) return t;
      const newDo = t.do.filter((name) => name !== taskName);
      // Also clean up waypoint/label entries for the removed target
      const updatedWaypoints = t.edge_waypoints
        ? Object.fromEntries(
            Object.entries(t.edge_waypoints).filter(([k]) => k !== taskName),
          )
        : undefined;
      const updatedLabelPos = t.label_positions
        ? Object.fromEntries(
            Object.entries(t.label_positions).filter(([k]) => k !== taskName),
          )
        : undefined;
      return {
        ...t,
        do: newDo.length > 0 ? newDo : undefined,
        edge_waypoints:
          updatedWaypoints && Object.keys(updatedWaypoints).length > 0
            ? updatedWaypoints
            : undefined,
        label_positions:
          updatedLabelPos && Object.keys(updatedLabelPos).length > 0
            ? updatedLabelPos
            : undefined,
      };
    })
    // Keep transitions that still have `do` targets or `publish` directives
    .filter(
      (t) => (t.do && t.do.length > 0) || (t.publish && t.publish.length > 0),
    );

  return cleaned.length > 0 ? cleaned : undefined;
}

/**
 * Rename a task in all transition `do` lists.
 * Returns a new array (or undefined) only when something changed;
 * otherwise returns the original reference so callers can cheaply
 * detect a no-op via `===`.
 */
export function renameTaskInTransitions(
  next: TaskTransition[] | undefined,
  oldName: string,
  newName: string,
): TaskTransition[] | undefined {
  if (!next) return undefined;

  let changed = false;
  const updated = next.map((t) => {
    const hasDo = t.do && t.do.includes(oldName);
    const hasWaypoint = t.edge_waypoints && oldName in t.edge_waypoints;
    const hasLabelPos = t.label_positions && oldName in t.label_positions;

    if (!hasDo && !hasWaypoint && !hasLabelPos) return t;
    changed = true;

    const result = { ...t };

    if (hasDo) {
      result.do = t.do!.map((name) => (name === oldName ? newName : name));
    }

    if (hasWaypoint && t.edge_waypoints) {
      const entries = Object.entries(t.edge_waypoints).map(([k, v]) => [
        k === oldName ? newName : k,
        v,
      ]);
      result.edge_waypoints = Object.fromEntries(entries);
    }

    if (hasLabelPos && t.label_positions) {
      const entries = Object.entries(t.label_positions).map(([k, v]) => [
        k === oldName ? newName : k,
        v,
      ]);
      result.label_positions = Object.fromEntries(entries);
    }

    return result;
  });

  return changed ? updated : next;
}

/**
 * Find "starting" tasks — those whose name does not appear in any
 * transition `do` list (i.e. no other task transitions into them).
 * Returns a Set of task IDs.
 */
export function findStartingTaskIds(tasks: WorkflowTask[]): Set<string> {
  // Collect every task name that is referenced as a transition target
  const targeted = new Set<string>();
  for (const task of tasks) {
    if (!task.next) continue;
    for (const t of task.next) {
      if (t.do) {
        for (const name of t.do) {
          targeted.add(name);
        }
      }
    }
  }

  const startIds = new Set<string>();
  for (const task of tasks) {
    if (!targeted.has(task.name)) {
      startIds.add(task.id);
    }
  }
  return startIds;
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/**
 * Generate a unique task ID
 */
export function generateTaskId(): string {
  return `task-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Create a new empty task
 */
export function createEmptyTask(
  name: string,
  position: NodePosition,
): WorkflowTask {
  return {
    id: generateTaskId(),
    name,
    action: "",
    input: {},
    position,
  };
}

/**
 * Generate a unique task name that doesn't conflict with existing tasks
 */
export function generateUniqueTaskName(
  existingTasks: WorkflowTask[],
  baseName: string = "task",
): string {
  const existingNames = new Set(existingTasks.map((t) => t.name));
  let counter = existingTasks.length + 1;
  let name = `${baseName}_${counter}`;
  while (existingNames.has(name)) {
    counter++;
    name = `${baseName}_${counter}`;
  }
  return name;
}

/**
 * Validate a workflow builder state and return any errors
 */
export function validateWorkflow(state: WorkflowBuilderState): string[] {
  const errors: string[] = [];

  if (!state.name.trim()) {
    errors.push("Workflow name is required");
  }

  if (!state.label.trim()) {
    errors.push("Workflow label is required");
  }

  if (!state.version.trim()) {
    errors.push("Workflow version is required");
  }

  if (!state.packRef) {
    errors.push("Pack reference is required");
  }

  if (state.tasks.length === 0) {
    errors.push("Workflow must have at least one task");
  }

  // Check for duplicate task names
  const taskNames = new Set<string>();
  for (const task of state.tasks) {
    if (taskNames.has(task.name)) {
      errors.push(`Duplicate task name: "${task.name}"`);
    }
    taskNames.add(task.name);
  }

  // Check that tasks have an action reference
  for (const task of state.tasks) {
    if (!task.action) {
      errors.push(`Task "${task.name}" must have an action assigned`);
    }
  }

  // Check that all transition targets reference existing tasks
  for (const task of state.tasks) {
    if (!task.next) continue;

    for (let ti = 0; ti < task.next.length; ti++) {
      const transition = task.next[ti];
      if (!transition.do) continue;

      for (const targetName of transition.do) {
        if (!taskNames.has(targetName)) {
          const whenLabel = transition.when
            ? ` (when: ${transition.when})`
            : " (always)";
          errors.push(
            `Task "${task.name}" transition${whenLabel} references non-existent task "${targetName}"`,
          );
        }
      }
    }
  }

  return errors;
}
