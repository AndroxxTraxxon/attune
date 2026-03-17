/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { Value } from './Value';
/**
 * Standard API response wrapper
 */
export type ApiResponse_IdentitySummary = {
    data: {
        attributes: Value;
        display_name?: string | null;
        id: number;
        login: string;
    };
    /**
     * Optional message
     */
    message?: string | null;
};

