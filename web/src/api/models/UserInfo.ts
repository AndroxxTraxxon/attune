/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * User information included in token response
 */
export type UserInfo = {
    /**
     * Permission set refs assigned to this identity, including role-derived assignments.
     */
    assigned_permission_set_refs?: Array<string> | null;
    /**
     * Authentication provider backing this identity.
     */
    auth_provider?: string | null;
    /**
     * Whether this identity can change its password through Attune.
     */
    can_change_password?: boolean | null;
    /**
     * Display name
     */
    display_name?: string | null;
    /**
     * Effective resource-level permissions assigned to this identity.
     */
    effective_permissions?: Array<{
        actions: Array<string>;
        resource: string;
    }> | null;
    /**
     * Identity ID
     */
    id: number;
    /**
     * Identity login
     */
    login: string;
    /**
     * Sanitized user information supplied by the external identity provider.
     */
    provider_profile?: ({
        distinguished_name?: string | null;
        display_name?: string | null;
        email?: string | null;
        email_verified?: boolean | null;
        groups: Array<string>;
        issuer?: string | null;
        login?: string | null;
        provider: string;
        subject?: string | null;
    }) | null;
    /**
     * Whether this identity is managed locally by Attune.
     */
    is_local?: boolean | null;
};
