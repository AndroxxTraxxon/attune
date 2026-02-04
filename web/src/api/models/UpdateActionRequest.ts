/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
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
     * Parameter schema
     */
    param_schema: any | null;
    /**
     * Runtime ID
     */
    runtime?: number | null;
};

