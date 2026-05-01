/* hand-written; mirrors the auto-generated services in this directory.
   regenerated copies will be overwritten by `npm run generate:api`,
   but until the codegen step runs against an updated OpenAPI spec we
   maintain this manually. */
/* eslint-disable */
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';

export type AuditCategory =
    | 'api'
    | 'auth'
    | 'rbac'
    | 'secret'
    | 'admin'
    | 'execution'
    | 'pack';

export type AuditOutcome = 'success' | 'failure' | 'denied';

export type AuditEventSummary = {
    id: number;
    created: string;
    category: string;
    event_type: string;
    outcome: string;
    actor_login?: string | null;
    actor_ip?: string | null;
    resource_type?: string | null;
    resource_ref?: string | null;
    http_method?: string | null;
    http_path?: string | null;
    http_status?: number | null;
    request_id?: string | null;
};

export type AuditEventResponse = {
    id: number;
    created: string;
    category: string;
    event_type: string;
    outcome: string;
    actor_identity?: number | null;
    actor_login?: string | null;
    actor_token_type?: string | null;
    actor_ip?: string | null;
    actor_user_agent?: string | null;
    resource_type?: string | null;
    resource_id?: number | null;
    resource_ref?: string | null;
    pack_ref?: string | null;
    http_method?: string | null;
    http_path?: string | null;
    http_status?: number | null;
    request_id?: string | null;
    parent_event_id?: number | null;
    details?: unknown | null;
    error_message?: string | null;
};

export type PaginatedAuditEvents = {
    data: AuditEventSummary[];
    pagination: {
        page: number;
        per_page: number;
        total?: number | null;
        total_pages?: number | null;
        has_next: boolean;
        has_previous: boolean;
    };
};

export type ApiResponse_AuditEventResponse = {
    data: AuditEventResponse;
};

export type ApiResponse_VecAuditEventResponse = {
    data: AuditEventResponse[];
};

export type ListAuditEventsParams = {
    category?: AuditCategory;
    eventType?: string;
    outcome?: AuditOutcome;
    actorIdentity?: number;
    actorLogin?: string;
    resourceType?: string;
    resourceId?: number;
    resourceRef?: string;
    requestId?: string;
    httpMethod?: string;
    httpStatus?: number;
    httpPath?: string;
    createdAfter?: string;
    createdBefore?: string;
    includeTotal?: boolean;
    page?: number;
    perPage?: number;
};

export class AuditLogService {
    public static listAuditEvents(params: ListAuditEventsParams = {}): CancelablePromise<PaginatedAuditEvents> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/audit-events',
            query: {
                category: params.category,
                event_type: params.eventType,
                outcome: params.outcome,
                actor_identity: params.actorIdentity,
                actor_login: params.actorLogin,
                resource_type: params.resourceType,
                resource_id: params.resourceId,
                resource_ref: params.resourceRef,
                request_id: params.requestId,
                http_method: params.httpMethod,
                http_status: params.httpStatus,
                http_path: params.httpPath,
                created_after: params.createdAfter,
                created_before: params.createdBefore,
                include_total: params.includeTotal,
                page: params.page,
                per_page: params.perPage,
            },
            errors: {
                401: `Unauthorized`,
                403: `Forbidden`,
                500: `Internal server error`,
            },
        });
    }

    public static getAuditEvent({ id }: { id: number }): CancelablePromise<ApiResponse_AuditEventResponse> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/audit-events/{id}',
            path: { id },
            errors: {
                401: `Unauthorized`,
                403: `Forbidden`,
                404: `Audit event not found`,
                500: `Internal server error`,
            },
        });
    }

    public static getAuditEventsByRequest({
        requestId,
    }: {
        requestId: string;
    }): CancelablePromise<ApiResponse_VecAuditEventResponse> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/api/v1/audit-events/by-request/{request_id}',
            path: { request_id: requestId },
            errors: {
                401: `Unauthorized`,
                403: `Forbidden`,
                500: `Internal server error`,
            },
        });
    }
}
