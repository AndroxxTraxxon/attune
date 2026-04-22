/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for updating a rule
 */
export type UpdateRuleRequest = {
    /**
     * Action reference to execute when rule matches
     */
    action_ref?: string | null;
    /**
     * Parameters to pass to the action when rule is triggered
     */
    action_params?: any | null;
    /**
     * Conditions for rule evaluation
     */
    conditions?: any | null;
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
     * Trigger reference that activates this rule
     */
    trigger_ref?: string | null;
    /**
     * Parameters for trigger configuration and event filtering
     */
    trigger_params?: any | null;
};
