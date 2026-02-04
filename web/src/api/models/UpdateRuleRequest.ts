/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for updating a rule
 */
export type UpdateRuleRequest = {
    /**
     * Parameters to pass to the action when rule is triggered
     */
    action_params: any | null;
    /**
     * Conditions for rule evaluation
     */
    conditions: any | null;
    /**
     * Rule description
     */
    description?: string | null;
    /**
     * Whether the rule is enabled
     */
    enabled?: boolean | null;
    /**
     * Human-readable label
     */
    label?: string | null;
    /**
     * Parameters for trigger configuration and event filtering
     */
    trigger_params: any | null;
};

