/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Standard API response wrapper
 */
export type ApiResponse_ActionResponse = {
    /**
     * Response DTO for action information
     */
    data: {
        /**
         * Creation timestamp
         */
        created: string;
        /**
         * Action description
         */
        description: string;
        /**
         * Entry point
         */
        entrypoint: string;
        /**
         * Action ID
         */
        id: number;
        /**
         * Whether this is an ad-hoc action (not from pack installation)
         */
        is_adhoc: boolean;
        /**
         * Human-readable label
         */
        label: string;
        /**
         * Output schema
         */
        out_schema: any | null;
        /**
         * Pack ID
         */
        pack: number;
        /**
         * Pack reference
         */
        pack_ref: string;
        /**
         * Parameter schema
         */
        param_schema: any | null;
        /**
         * Unique reference identifier
         */
        ref: string;
        /**
         * Runtime ID
         */
        runtime?: number | null;
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

