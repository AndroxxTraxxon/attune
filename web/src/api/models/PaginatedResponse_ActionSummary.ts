/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { PaginationMeta } from './PaginationMeta';
/**
 * Paginated response wrapper
 */
export type PaginatedResponse_ActionSummary = {
    /**
     * The page items
     */
    items: Array<{
        /**
         * Hint that this action may invoke the Attune MCP server and spawn child executions.
         */
        accesses_mcp: boolean;
        /**
         * Creation timestamp
         */
        created: string;
        /**
         * Default permission set refs used when executions do not explicitly override token permissions.
         */
        default_execution_permission_set_refs?: Array<string>;
        /**
         * Action description
         */
        description?: string | null;
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
         * Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
         */
        required_worker_runtimes?: Record<string, any>;
        /**
         * Runtime ID
         */
        runtime?: number | null;
        /**
         * Runtime reference (stable identifier, e.g., "core.python")
         */
        runtime_ref?: string | null;
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
