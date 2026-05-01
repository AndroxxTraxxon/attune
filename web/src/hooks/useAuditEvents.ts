import { useQuery, keepPreviousData } from "@tanstack/react-query";
import {
    AuditLogService,
    type ListAuditEventsParams,
} from "@/api/services/AuditLogService";

export function useAuditEvents(params: ListAuditEventsParams) {
    return useQuery({
        queryKey: ["audit-events", params],
        queryFn: () => AuditLogService.listAuditEvents(params),
        placeholderData: keepPreviousData,
        staleTime: 15_000,
    });
}

export function useAuditEvent(id: number | null | undefined) {
    return useQuery({
        queryKey: ["audit-event", id],
        queryFn: () => AuditLogService.getAuditEvent({ id: id! }),
        enabled: id !== null && id !== undefined,
    });
}

export function useAuditEventsByRequest(requestId: string | null | undefined) {
    return useQuery({
        queryKey: ["audit-event-chain", requestId],
        queryFn: () =>
            AuditLogService.getAuditEventsByRequest({ requestId: requestId! }),
        enabled: !!requestId,
    });
}
