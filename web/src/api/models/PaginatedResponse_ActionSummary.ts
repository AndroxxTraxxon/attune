/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { PaginationMeta } from "./PaginationMeta";
/**
 * Paginated response wrapper
 */
export type PaginatedResponse_ActionSummary = {
  /**
   * The data items
   */
  data: Array<{
    /**
     * Creation timestamp
     */
    created: string;
    /**
     * Action description
     */
    description: string | null;
    /**
     * Entry point
     */
    entrypoint: string;
    /**
     * Action ID
     */
    id: number;
    /**
     * Human-readable label
     */
    label: string;
    /**
     * Pack reference
     */
    pack_ref: string;
    /**
     * Unique reference identifier
     */
    ref: string;
    /**
     * Runtime ID
     */
    runtime?: number | null;
    /**
     * Semver version constraint for the runtime
     */
    runtime_version_constraint?: string | null;
    /**
     * Last update timestamp
     */
    updated: string;
    /**
     * Workflow definition ID (non-null if this action is a workflow)
     */
    workflow_def?: number | null;
  }>;
  /**
   * Pagination metadata
   */
  pagination: PaginationMeta;
};
