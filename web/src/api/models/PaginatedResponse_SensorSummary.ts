/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
 
import type { PaginationMeta } from './PaginationMeta';
/**
 * Paginated response wrapper
 */
export type PaginatedResponse_SensorSummary = {
    /**
     * The data items
     */
    data: Array<{
        /**
         * Creation timestamp
         */
        created: string;
        /**
         * Sensor description
         */
        description: string;
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
         * Trigger reference
         */
        trigger_ref: string;
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

