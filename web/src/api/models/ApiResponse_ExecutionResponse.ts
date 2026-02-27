/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { ExecutionStatus } from "./ExecutionStatus";
/**
 * Standard API response wrapper
 */
export type ApiResponse_ExecutionResponse = {
  /**
   * Response DTO for execution information
   */
  data: {
    /**
     * Action ID (optional, may be null for ad-hoc executions)
     */
    action?: number | null;
    /**
     * Action reference
     */
    action_ref: string;
    /**
     * Execution configuration/parameters
     */
    config: Record<string, any>;
    /**
     * Creation timestamp
     */
    created: string;
    /**
     * Enforcement ID (rule enforcement that triggered this)
     */
    enforcement?: number | null;
    /**
     * Executor ID (worker/executor that ran this)
     */
    executor?: number | null;
    /**
     * Execution ID
     */
    id: number;
    /**
     * Parent execution ID (for nested/child executions)
     */
    parent?: number | null;
    /**
     * Execution result/output
     */
    result: Record<string, any>;
    /**
     * Execution status
     */
    status: ExecutionStatus;
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
  /**
   * Optional message
   */
  message?: string | null;
};
