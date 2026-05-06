/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Standard API response wrapper
 */
export type ApiResponse_CurrentUserResponse = {
    /**
     * Current user response
     */
    data: {
        /**
         * Permission set refs assigned to this identity, including role-derived assignments.
         */
        assigned_permission_set_refs: Array<string>;
        /**
         * Authentication provider backing this identity.
         */
        auth_provider: string;
        /**
         * Whether this identity can change its password through Attune.
         */
        can_change_password: boolean;
        /**
         * Display name
         */
        display_name?: string | null;
        /**
         * Effective resource-level permissions assigned to this identity.
         */
        effective_permissions: Array<{
            /**
             * Actions allowed for the resource.
             */
            actions: Array<string>;
            /**
             * RBAC resource name.
             */
            resource: string;
        }>;
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
            /**
             * LDAP distinguished name, when available.
             */
            distinguished_name?: string | null;
            /**
             * Provider-issued display name.
             */
            display_name?: string | null;
            /**
             * Provider-issued email address.
             */
            email?: string | null;
            /**
             * Whether the provider reported the email address as verified.
             */
            email_verified?: boolean | null;
            /**
             * Provider groups associated with this identity.
             */
            groups: Array<string>;
            /**
             * OIDC issuer URL, when available.
             */
            issuer?: string | null;
            /**
             * Provider-issued login or preferred username.
             */
            login?: string | null;
            /**
             * Provider backing this identity.
             */
            provider: string;
            /**
             * OIDC subject identifier, when available.
             */
            subject?: string | null;
        }) | null;
        /**
         * Whether this identity is managed locally by Attune.
         */
        is_local: boolean;
    };
    /**
     * Optional message
     */
    message?: string | null;
};
