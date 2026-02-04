/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for updating a pack
 */
export type UpdatePackRequest = {
    /**
     * Configuration schema
     */
    conf_schema: any | null;
    /**
     * Pack configuration values
     */
    config: any | null;
    /**
     * Pack description
     */
    description?: string | null;
    /**
     * Whether this is a standard pack
     */
    is_standard?: boolean | null;
    /**
     * Human-readable label
     */
    label?: string | null;
    /**
     * Pack metadata
     */
    meta: any | null;
    /**
     * Runtime dependencies
     */
    runtime_deps?: any[] | null;
    /**
     * Tags for categorization
     */
    tags?: any[] | null;
    /**
     * Pack version
     */
    version?: string | null;
};

