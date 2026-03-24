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
     * Runtime ID
     */
    runtime?: number | null;
    runtime_version_constraint?: (null | RuntimeVersionConstraintPatch);
};

