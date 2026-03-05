/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
 
/**
 * Standard API response wrapper
 */
export type ApiResponse_CurrentUserResponse = {
    /**
     * Current user response
     */
    data: {
        /**
         * Display name
         */
        display_name?: string | null;
        /**
         * Identity ID
         */
        id: number;
        /**
         * Identity login
         */
        login: string;
    };
    /**
     * Optional message
     */
    message?: string | null;
};

