/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Simplified action response (for list endpoints)
 */
export type ActionSummary = {
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
     * Human-readable label
     */
    label: string;
    /**
     * Pack reference
     */
    pack_ref: string;
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

