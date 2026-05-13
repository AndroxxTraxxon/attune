/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for updating a sensor
 */
export type UpdateSensorRequest = {
    artifact_retention_limit?: any | null;
    artifact_retention_policy?: any | null;
    /**
     * Sensor description
     */
    description?: string | null;
    /**
     * Whether the sensor is enabled
     */
    enabled?: boolean | null;
    /**
     * Entry point for sensor execution
     */
    entrypoint?: string | null;
    /**
     * Human-readable label
     */
    label?: string | null;
    log_retention_limit?: any | null;
    log_retention_policy?: any | null;
    /**
     * Parameter schema (StackStorm-style with inline required/secret)
     */
    param_schema?: any | null;
};
