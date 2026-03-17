/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { ExecutionStatus } from './ExecutionStatus';
/**
 * Response DTO for execution information
 */
export type ExecutionResponse = {
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
     * Identity ID that initiated this execution
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
     * When the execution actually started running (worker picked it up).
     * Null if the execution hasn't started running yet.
     */
    started_at?: string | null;
    /**
     * Execution status
     */
    status: ExecutionStatus;
    /**
     * Last update timestamp
     */
    updated: string;
    /**
     * Worker ID currently assigned to this execution
     */
    worker?: number | null;
    /**
     * Workflow task metadata (only populated for workflow task executions)
     */
    workflow_task?: any | null;
};

