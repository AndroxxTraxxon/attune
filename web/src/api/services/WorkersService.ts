/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { PaginatedResponse_WorkerSummary } from '../models/PaginatedResponse_WorkerSummary';
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';
export class WorkersService {
    /**
     * @returns PaginatedResponse_WorkerSummary List workers with runtime support and current load
     * @throws ApiError
     */
    public static listWorkers({
        page,
        pageSize,
    }: {
        page?: number,
        pageSize?: number,
    }): CancelablePromise<PaginatedResponse_WorkerSummary> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/workers',
            query: {
                'page': page,
                'page_size': pageSize,
            },
        });
    }
}
