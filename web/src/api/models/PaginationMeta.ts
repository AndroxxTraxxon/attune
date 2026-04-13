/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Pagination metadata
 */
export type PaginationMeta = {
    /**
     * Current page number (1-based)
     */
    page: number;
    /**
     * Number of items per page
     */
    page_size: number;
    /**
     * Total number of items, when an exact count was requested.
     */
    total_items?: number | null;
    /**
     * Total number of pages, when an exact count was requested.
     */
    total_pages?: number | null;
    /**
     * Whether a previous page exists.
     */
    has_previous: boolean;
    /**
     * Whether a next page exists.
     */
    has_next: boolean;
};
