/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for updating a trigger
 */
export type UpdateTriggerRequest = {
    /**
     * Trigger description
     */
    description?: string | null;
    /**
     * Whether the trigger is enabled
     */
    enabled?: boolean | null;
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
};

