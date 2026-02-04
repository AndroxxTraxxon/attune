import { useQuery } from "@tanstack/react-query";
import { EventsService, EnforcementsService, EnforcementStatus } from "@/api";
import type { i64 } from "@/api";

interface EventsQueryParams {
  page?: number;
  pageSize?: number;
  trigger?: i64 | null;
  triggerRef?: string | null;
  source?: i64 | null;
}

interface EnforcementsQueryParams {
  page?: number;
  pageSize?: number;
  status?: EnforcementStatus | null;
  rule?: i64 | null;
  event?: i64 | null;
  triggerRef?: string | null;
}

// Fetch all events with pagination and filters
export function useEvents(params?: EventsQueryParams) {
  return useQuery({
    queryKey: ["events", params],
    queryFn: async () => {
      return await EventsService.listEvents({
        page: params?.page || 1,
        perPage: params?.pageSize || 50,
        trigger: params?.trigger,
        triggerRef: params?.triggerRef,
        source: params?.source,
      });
    },
    staleTime: 30000, // 30 seconds
  });
}

// Fetch single event by ID
export function useEvent(id: number) {
  return useQuery({
    queryKey: ["events", id],
    queryFn: async () => {
      return await EventsService.getEvent({ id });
    },
    enabled: !!id,
    staleTime: 30000,
  });
}

// Fetch all enforcements with pagination and filters
export function useEnforcements(params?: EnforcementsQueryParams) {
  return useQuery({
    queryKey: ["enforcements", params],
    queryFn: async () => {
      return await EnforcementsService.listEnforcements({
        page: params?.page || 1,
        perPage: params?.pageSize || 50,
        status: params?.status,
        rule: params?.rule,
        event: params?.event,
        triggerRef: params?.triggerRef,
      });
    },
    staleTime: 30000,
  });
}

// Fetch single enforcement by ID
export function useEnforcement(id: number) {
  return useQuery({
    queryKey: ["enforcements", id],
    queryFn: async () => {
      return await EnforcementsService.getEnforcement({ id });
    },
    enabled: !!id,
    staleTime: 30000,
  });
}

// Fetch enforcements by rule ID
export function useRuleEnforcements(ruleId: number) {
  return useQuery({
    queryKey: ["rules", ruleId, "enforcements"],
    queryFn: async () => {
      return await EnforcementsService.listEnforcements({
        page: 1,
        perPage: 100,
        rule: ruleId,
      });
    },
    enabled: !!ruleId,
    staleTime: 30000,
  });
}

// Fetch enforcements by event ID
export function useEventEnforcements(eventId: number) {
  return useQuery({
    queryKey: ["events", eventId, "enforcements"],
    queryFn: async () => {
      return await EnforcementsService.listEnforcements({
        page: 1,
        perPage: 100,
        event: eventId,
      });
    },
    enabled: !!eventId,
    staleTime: 30000,
  });
}
