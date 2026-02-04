/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Standard API response wrapper
 */
export type ApiResponse_RuleResponse = {
    /**
     * Response DTO for rule information
     */
    data: {
        /**
         * Action ID
         */
        action: number;
        /**
         * Parameters to pass to the action when rule is triggered
         */
        action_params: Record<string, any>;
        /**
         * Action reference
         */
        action_ref: string;
        /**
         * Conditions for rule evaluation
         */
        conditions: Record<string, any>;
        /**
         * Creation timestamp
         */
        created: string;
        /**
         * Rule description
         */
        description: string;
        /**
         * Whether the rule is enabled
         */
        enabled: boolean;
        /**
         * Rule ID
         */
        id: number;
        /**
         * Whether this is an ad-hoc rule (not from pack installation)
         */
        is_adhoc: boolean;
        /**
         * Human-readable label
         */
        label: string;
        /**
         * Pack ID
         */
        pack: number;
        /**
         * Pack reference
         */
        pack_ref: string;
        /**
         * Unique reference identifier
         */
        ref: string;
        /**
         * Trigger ID
         */
        trigger: number;
        /**
         * Parameters for trigger configuration and event filtering
         */
        trigger_params: Record<string, any>;
        /**
         * Trigger reference
         */
        trigger_ref: string;
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

