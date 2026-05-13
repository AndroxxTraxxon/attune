/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Simplified sensor response (for list endpoints)
 */
export type SensorSummary = {
    artifact_retention_limit?: number | null;
    artifact_retention_policy?: 'versions' | 'days' | 'hours' | 'minutes' | null;
    /**
     * Creation timestamp
     */
    created: string;
    /**
     * Sensor description
     */
    description?: string | null;
    /**
     * Whether the sensor is enabled
     */
    enabled: boolean;
    /**
     * Sensor ID
     */
    id: number;
    /**
     * Human-readable label
     */
    label: string;
    log_retention_limit?: number | null;
    log_retention_policy?: 'versions' | 'days' | 'hours' | 'minutes' | null;
    /**
     * Pack reference (optional)
     */
    pack_ref?: string | null;
    /**
     * Unique reference identifier
     */
    ref: string;
    /**
     * Last update timestamp
     */
    updated: string;
};
