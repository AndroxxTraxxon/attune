/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Standard API response wrapper
 */
export type ApiResponse_SensorResponse = {
    /**
     * Response DTO for sensor information
     */
    data: {
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
         * Entry point
         */
        entrypoint: string;
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
         * Pack ID (optional)
         */
        pack?: number | null;
        /**
         * Pack reference (optional)
         */
        pack_ref?: string | null;
        /**
         * Parameter schema (StackStorm-style with inline required/secret)
         */
        param_schema: any | null;
        /**
         * Unique reference identifier
         */
        ref: string;
        /**
         * Runtime ID
         */
        runtime: number;
        /**
         * Runtime reference
         */
        runtime_ref: string;
        /**
         * Last update timestamp
         */
        updated: string;
    };
    /**
     * Optional message
     */
    message?: string | null;
};
