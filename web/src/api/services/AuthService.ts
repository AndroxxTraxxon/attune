/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
 
import type { ChangePasswordRequest } from '../models/ChangePasswordRequest';
import type { LoginRequest } from '../models/LoginRequest';
import type { RefreshTokenRequest } from '../models/RefreshTokenRequest';
import type { RegisterRequest } from '../models/RegisterRequest';
import type { UserInfo } from '../models/UserInfo';
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';
export class AuthService {
    /**
     * Change password endpoint
     * POST /auth/change-password
     * @returns any Password changed successfully
     * @throws ApiError
     */
    public static changePassword({
        requestBody,
    }: {
        requestBody: ChangePasswordRequest,
    }): CancelablePromise<{
        /**
         * Success message response (for operations that don't return data)
         */
        data: {
            /**
             * Message describing the operation
             */
            message: string;
            /**
             * Success indicator
             */
            success: boolean;
        };
        /**
         * Optional message
         */
        message?: string | null;
    }> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/auth/change-password',
            body: requestBody,
            mediaType: 'application/json',
            errors: {
                400: `Validation error`,
                401: `Invalid current password or unauthorized`,
                404: `Identity not found`,
            },
        });
    }
    /**
     * Login endpoint
     * POST /auth/login
     * @returns any Successfully logged in
     * @throws ApiError
     */
    public static login({
        requestBody,
    }: {
        requestBody: LoginRequest,
    }): CancelablePromise<{
        /**
         * Token response
         */
        data: {
            /**
             * Access token (JWT)
             */
            access_token: string;
            /**
             * Access token expiration in seconds
             */
            expires_in: number;
            /**
             * Refresh token
             */
            refresh_token: string;
            /**
             * Token type (always "Bearer")
             */
            token_type: string;
            user?: (null | UserInfo);
        };
        /**
         * Optional message
         */
        message?: string | null;
    }> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/auth/login',
            body: requestBody,
            mediaType: 'application/json',
            errors: {
                400: `Validation error`,
                401: `Invalid credentials`,
            },
        });
    }
    /**
     * Get current user endpoint
     * GET /auth/me
     * @returns any Current user information
     * @throws ApiError
     */
    public static getCurrentUser(): CancelablePromise<{
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
    }> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/auth/me',
            errors: {
                401: `Unauthorized`,
                404: `Identity not found`,
            },
        });
    }
    /**
     * Refresh token endpoint
     * POST /auth/refresh
     * @returns any Successfully refreshed token
     * @throws ApiError
     */
    public static refreshToken({
        requestBody,
    }: {
        requestBody: RefreshTokenRequest,
    }): CancelablePromise<{
        /**
         * Token response
         */
        data: {
            /**
             * Access token (JWT)
             */
            access_token: string;
            /**
             * Access token expiration in seconds
             */
            expires_in: number;
            /**
             * Refresh token
             */
            refresh_token: string;
            /**
             * Token type (always "Bearer")
             */
            token_type: string;
            user?: (null | UserInfo);
        };
        /**
         * Optional message
         */
        message?: string | null;
    }> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/auth/refresh',
            body: requestBody,
            mediaType: 'application/json',
            errors: {
                400: `Validation error`,
                401: `Invalid or expired refresh token`,
            },
        });
    }
    /**
     * Register endpoint
     * POST /auth/register
     * @returns any Successfully registered
     * @throws ApiError
     */
    public static register({
        requestBody,
    }: {
        requestBody: RegisterRequest,
    }): CancelablePromise<{
        /**
         * Token response
         */
        data: {
            /**
             * Access token (JWT)
             */
            access_token: string;
            /**
             * Access token expiration in seconds
             */
            expires_in: number;
            /**
             * Refresh token
             */
            refresh_token: string;
            /**
             * Token type (always "Bearer")
             */
            token_type: string;
            user?: (null | UserInfo);
        };
        /**
         * Optional message
         */
        message?: string | null;
    }> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/auth/register',
            body: requestBody,
            mediaType: 'application/json',
            errors: {
                400: `Validation error`,
                409: `User already exists`,
            },
        });
    }
}
