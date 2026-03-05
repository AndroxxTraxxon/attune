/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { ExecutionStatus } from "./ExecutionStatus";
/**
 * Simplified execution response (for list endpoints)
 */
export type ExecutionSummary = {
  /**
   * Action reference
   */
  action_ref: string;
  /**
   * Creation timestamp
   */
  created: string;
  /**
   * Enforcement ID
   */
  enforcement?: number | null;
  /**
   * Execution ID
   */
  id: number;
  /**
   * Parent execution ID
   */
  parent?: number | null;
  /**
   * Rule reference (if triggered by a rule)
   */
  rule_ref?: string | null;
  /**
   * Execution status
   */
  status: ExecutionStatus;
  /**
   * When the execution actually started running (worker picked it up).
   * Null if the execution hasn't started running yet.
   */
  started_at?: string | null;
  /**
   * Trigger reference (if triggered by a trigger)
   */
  trigger_ref?: string | null;
  /**
   * Last update timestamp
   */
  updated: string;
  /**
   * Workflow task metadata (only populated for workflow task executions)
   */
  workflow_task?: {
    workflow_execution: number;
    task_name: string;
    triggered_by?: string | null;
    task_index?: number | null;
    task_batch?: number | null;
    retry_count: number;
    max_retries: number;
    next_retry_at?: string | null;
    timeout_seconds?: number | null;
    timed_out: boolean;
    duration_ms?: number | null;
    started_at?: string | null;
    completed_at?: string | null;
  } | null;
};
