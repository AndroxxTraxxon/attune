/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { RuntimeVersionConstraintPatch } from './RuntimeVersionConstraintPatch';
/**
 * Request DTO for updating an action
 */
export type UpdateActionRequest = {
    /**
     * Hint that this action may invoke the Attune MCP server and spawn child executions.
     */
    accesses_mcp?: boolean | null;
    /**
     * Default permission set refs for execution-scoped API tokens.
     */
    default_execution_permission_set_refs?: Array<string> | null;
    /**
     * Action description
     */
    description?: string | null;
    /**
     * Entry point for action execution
     */
    entrypoint?: string | null;
    /**
     * Human-readable label
     */
    label?: string | null;
    /**
     * Output schema
     */
    out_schema: any | null;
    /**
     * Parameter schema (StackStorm-style with inline required/secret)
     */
    param_schema: any | null;
    /**
     * Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
     */
    required_worker_runtimes: any | null;
    /**
     * Runtime ID
     */
    runtime?: number | null;
    runtime_version_constraint?: (null | RuntimeVersionConstraintPatch);
};
