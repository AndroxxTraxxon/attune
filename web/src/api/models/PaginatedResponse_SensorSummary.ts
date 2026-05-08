/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { PaginationMeta } from './PaginationMeta';
/**
 * Paginated response wrapper
 */
export type PaginatedResponse_SensorSummary = {
    /**
     * The page items
     */
    items: Array<{
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
    }>;
    /**
     * Pagination metadata
     */
    pagination: PaginationMeta;
};

