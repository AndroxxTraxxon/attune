/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for creating a new sensor
 */
export type CreateSensorRequest = {
    artifact_retention_limit?: number | null;
    artifact_retention_policy?: 'versions' | 'days' | 'hours' | 'minutes' | null;
    /**
     * Configuration values for this sensor instance (conforms to param_schema)
     */
    config?: any | null;
    /**
     * Sensor description
     */
    description?: string | null;
    /**
     * Whether the sensor is enabled
     */
    enabled?: boolean;
    /**
     * Entry point for sensor execution (e.g., path to script, function name)
     */
    entrypoint: string;
    /**
     * Human-readable label
     */
    label: string;
    log_retention_limit?: number | null;
    log_retention_policy?: 'versions' | 'days' | 'hours' | 'minutes' | null;
    /**
     * Pack reference this sensor belongs to
     */
    pack_ref: string;
    /**
     * Parameter schema (flat format) for sensor configuration
     */
    param_schema?: any | null;
    /**
     * Unique reference identifier (e.g., "mypack.cpu_monitor")
     */
    ref: string;
    /**
     * Runtime reference for this sensor
     */
    runtime_ref: string;
    /**
     * Trigger reference this sensor monitors for
     */
    trigger_ref: string;
};
