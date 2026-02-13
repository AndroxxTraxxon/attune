import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  PacksService,
  RulesService,
  ActionsService,
  TriggersService,
} from "@/api";
import type {
  PaginatedResponse_PackSummary,
  PaginatedResponse_RuleSummary,
  PaginatedResponse_ActionSummary,
  PaginatedResponse_TriggerSummary,
} from "@/api";

/**
 * Fetches all packs, rules, actions, and triggers and returns sorted
 * arrays of their refs for use as autocomplete suggestions.
 *
 * Data is cached with a long staleTime (5 minutes) since entity definitions
 * change infrequently.  Individual pages can augment these base suggestions
 * with refs discovered via WebSocket notifications.
 */
export function useFilterSuggestions() {
  const { data: packsData } = useQuery<PaginatedResponse_PackSummary>({
    queryKey: ["filter-suggestions", "packs"],
    queryFn: () => PacksService.listPacks({ page: 1, pageSize: 200 }),
    staleTime: 5 * 60 * 1000,
  });

  const { data: rulesData } = useQuery<PaginatedResponse_RuleSummary>({
    queryKey: ["filter-suggestions", "rules"],
    queryFn: () => RulesService.listRules({ page: 1, pageSize: 200 }),
    staleTime: 5 * 60 * 1000,
  });

  const { data: actionsData } = useQuery<PaginatedResponse_ActionSummary>({
    queryKey: ["filter-suggestions", "actions"],
    queryFn: () => ActionsService.listActions({ page: 1, pageSize: 200 }),
    staleTime: 5 * 60 * 1000,
  });

  const { data: triggersData } = useQuery<PaginatedResponse_TriggerSummary>({
    queryKey: ["filter-suggestions", "triggers"],
    queryFn: () => TriggersService.listTriggers({ page: 1, pageSize: 200 }),
    staleTime: 5 * 60 * 1000,
  });

  const packNames = useMemo(() => {
    const refs = packsData?.data?.map((p) => p.ref) || [];
    return [...new Set(refs)].sort();
  }, [packsData]);

  const ruleRefs = useMemo(() => {
    const refs = rulesData?.data?.map((r) => r.ref) || [];
    return [...new Set(refs)].sort();
  }, [rulesData]);

  const actionRefs = useMemo(() => {
    const refs = actionsData?.data?.map((a) => a.ref) || [];
    return [...new Set(refs)].sort();
  }, [actionsData]);

  const triggerRefs = useMemo(() => {
    const refs = triggersData?.data?.map((t) => t.ref) || [];
    return [...new Set(refs)].sort();
  }, [triggersData]);

  return { packNames, ruleRefs, actionRefs, triggerRefs };
}

/**
 * Merge base suggestion arrays with additional refs discovered at runtime
 * (e.g. from WebSocket notifications or loaded page data).
 * Returns a new sorted, deduplicated array only when the inputs change.
 */
export function useMergedSuggestions(
  base: string[],
  ...additionalSets: string[][]
): string[] {
  return useMemo(() => {
    const hasAdditional = additionalSets.some((s) => s.length > 0);
    if (!hasAdditional) return base;
    const merged = new Set(base);
    for (const set of additionalSets) {
      for (const item of set) merged.add(item);
    }
    return [...merged].sort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [base, ...additionalSets]);
}
