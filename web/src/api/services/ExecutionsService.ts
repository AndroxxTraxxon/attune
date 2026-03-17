/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { ExecutionStatus } from '../models/ExecutionStatus';
import type { PaginatedResponse_ExecutionSummary } from '../models/PaginatedResponse_ExecutionSummary';
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';
export class ExecutionsService {
    /**
     * List all executions with pagination and optional filters
     * @returns PaginatedResponse_ExecutionSummary List of executions
     * @throws ApiError
     */
    public static listExecutions({
        status,
        actionRef,
        packName,
        ruleRef,
        triggerRef,
        executor,
        resultContains,
        enforcement,
        parent,
        topLevelOnly,
        page,
        perPage,
    }: {
        /**
         * Filter by execution status
         */
        status?: (null | ExecutionStatus),
        /**
         * Filter by action reference
         */
        actionRef?: string | null,
        /**
         * Filter by pack name
         */
        packName?: string | null,
        /**
         * Filter by rule reference
         */
        ruleRef?: string | null,
        /**
         * Filter by trigger reference
         */
        triggerRef?: string | null,
        /**
         * Filter by executor ID
         */
        executor?: number | null,
        /**
         * Search in result JSON (case-insensitive substring match)
         */
        resultContains?: string | null,
        /**
         * Filter by enforcement ID
         */
        enforcement?: number | null,
        /**
         * Filter by parent execution ID
         */
        parent?: number | null,
        /**
         * If true, only return top-level executions (those without a parent).
         * Useful for the "By Workflow" view where child tasks are loaded separately.
         */
        topLevelOnly?: boolean | null,
        /**
         * Page number (for pagination)
         */
        page?: number,
        /**
         * Items per page (for pagination)
         */
        perPage?: number,
    }): CancelablePromise<PaginatedResponse_ExecutionSummary> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/executions',
            query: {
                'status': status,
                'action_ref': actionRef,
                'pack_name': packName,
                'rule_ref': ruleRef,
                'trigger_ref': triggerRef,
                'executor': executor,
                'result_contains': resultContains,
                'enforcement': enforcement,
                'parent': parent,
                'top_level_only': topLevelOnly,
                'page': page,
                'per_page': perPage,
            },
        });
    }
    /**
     * List executions by enforcement ID
     * @returns PaginatedResponse_ExecutionSummary List of executions for enforcement
     * @throws ApiError
     */
    public static listExecutionsByEnforcement({
        enforcementId,
        page,
        pageSize,
    }: {
        /**
         * Enforcement ID
         */
        enforcementId: number,
        /**
         * Page number (1-based)
         */
        page?: number,
        /**
         * Number of items per page
         */
        pageSize?: number,
    }): CancelablePromise<PaginatedResponse_ExecutionSummary> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/executions/enforcement/{enforcement_id}',
            path: {
                'enforcement_id': enforcementId,
            },
            query: {
                'page': page,
                'page_size': pageSize,
            },
            errors: {
                500: `Internal server error`,
            },
        });
    }
    /**
     * Get execution statistics
     * @returns any Execution statistics
     * @throws ApiError
     */
    public static getExecutionStats(): CancelablePromise<Record<string, any>> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/executions/stats',
            errors: {
                500: `Internal server error`,
            },
        });
    }
    /**
     * List executions by status
     * @returns PaginatedResponse_ExecutionSummary List of executions with specified status
     * @throws ApiError
     */
    public static listExecutionsByStatus({
        status,
        page,
        pageSize,
    }: {
        /**
         * Execution status (requested, scheduling, scheduled, running, completed, failed, canceling, cancelled, timeout, abandoned)
         */
        status: string,
        /**
         * Page number (1-based)
         */
        page?: number,
        /**
         * Number of items per page
         */
        pageSize?: number,
    }): CancelablePromise<PaginatedResponse_ExecutionSummary> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/executions/status/{status}',
            path: {
                'status': status,
            },
            query: {
                'page': page,
                'page_size': pageSize,
            },
            errors: {
                400: `Invalid status`,
                500: `Internal server error`,
            },
        });
    }
    /**
     * Get a single execution by ID
     * @returns any Execution details
     * @throws ApiError
     */
    public static getExecution({
        id,
    }: {
        /**
         * Execution ID
         */
        id: number,
    }): CancelablePromise<{
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
        /**
         * Optional message
         */
        message?: string | null;
    }> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/executions/{id}',
            path: {
                'id': id,
            },
            errors: {
                404: `Execution not found`,
            },
        });
    }
}
