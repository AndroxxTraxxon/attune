/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for creating a new action
 */
export type CreateActionRequest = {
    /**
     * Action description
     */
    description: string;
    /**
     * Entry point for action execution (e.g., path to script, function name)
     */
    entrypoint: string;
    /**
     * Human-readable label
     */
    label: string;
    /**
     * Output schema (JSON Schema) defining expected outputs
     */
    out_schema?: any | null;
    /**
     * Pack reference this action belongs to
     */
    pack_ref: string;
    /**
     * Parameter schema (JSON Schema) defining expected inputs
     */
    param_schema?: any | null;
    /**
     * Unique reference identifier (e.g., "core.http", "aws.ec2.start_instance")
     */
    ref: string;
    /**
     * Optional runtime ID for this action
     */
    runtime?: number | null;
};

