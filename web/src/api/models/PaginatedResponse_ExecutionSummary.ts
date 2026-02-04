/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { ExecutionStatus } from './ExecutionStatus';
import type { PaginationMeta } from './PaginationMeta';
/**
 * Paginated response wrapper
 */
export type PaginatedResponse_ExecutionSummary = {
    /**
     * The data items
     */
    data: Array<{
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
         * Trigger reference (if triggered by a trigger)
         */
        trigger_ref?: string | null;
        /**
         * Last update timestamp
         */
        updated: string;
    }>;
    /**
     * Pagination metadata
     */
    pagination: PaginationMeta;
};

