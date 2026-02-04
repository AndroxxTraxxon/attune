/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for creating a new workflow
 */
export type CreateWorkflowRequest = {
    /**
     * Workflow definition (complete workflow YAML structure as JSON)
     */
    definition: Record<string, any>;
    /**
     * Workflow description
     */
    description?: string | null;
    /**
     * Whether the workflow is enabled
     */
    enabled?: boolean | null;
    /**
     * Human-readable label
     */
    label: string;
    /**
     * Output schema (JSON Schema) defining expected outputs
     */
    out_schema: Record<string, any>;
    /**
     * Pack reference this workflow belongs to
     */
    pack_ref: string;
    /**
     * Parameter schema (JSON Schema) defining expected inputs
     */
    param_schema: Record<string, any>;
    /**
     * Unique reference identifier (e.g., "core.notify_on_failure", "slack.incident_workflow")
     */
    ref: string;
    /**
     * Tags for categorization and search
     */
    tags?: any[] | null;
    /**
     * Workflow version (semantic versioning recommended)
     */
    version: string;
};

