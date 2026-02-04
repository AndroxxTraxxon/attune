/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for creating a new trigger
 */
export type CreateTriggerRequest = {
    /**
     * Trigger description
     */
    description?: string | null;
    /**
     * Whether the trigger is enabled
     */
    enabled?: boolean;
    /**
     * Human-readable label
     */
    label: string;
    /**
     * Output schema (JSON Schema) defining event data structure
     */
    out_schema?: any | null;
    /**
     * Optional pack reference this trigger belongs to
     */
    pack_ref?: string | null;
    /**
     * Parameter schema (JSON Schema) defining event payload structure
     */
    param_schema?: any | null;
    /**
     * Unique reference identifier (e.g., "core.webhook", "system.timer")
     */
    ref: string;
};

