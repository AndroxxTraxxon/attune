/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
 
/**
 * Simplified trigger response (for list endpoints)
 */
export type TriggerSummary = {
    /**
     * Creation timestamp
     */
    created: string;
    /**
     * Trigger description
     */
    description?: string | null;
    /**
     * Whether the trigger is enabled
     */
    enabled: boolean;
    /**
     * Trigger ID
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
    /**
     * Whether webhooks are enabled for this trigger
     */
    webhook_enabled: boolean;
};

