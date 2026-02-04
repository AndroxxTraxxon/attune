# API Completion Plan - Executive Summary

## Overview
During the zero-warnings cleanup, we preserved 20+ API methods marked with `#[allow(dead_code)]` that represent planned but unimplemented features. These aren't dead code—they're the foundation for important functionality.

## High-Priority Features Ready to Implement

### 1. Token Refresh Mechanism (4-6 hours) ⭐
**Problem:** CLI sessions expire after 1 hour, requiring manual re-login  
**Solution:** Automatic token refresh using stored refresh tokens  
**Impact:** Dramatically improves CLI user experience for long sessions  
**Files:** `crates/api/src/routes/auth.rs`, `crates/cli/src/client.rs`

### 2. Complete CRUD Operations (8-12 hours) ⭐⭐
**Problem:** CLI can only create/read resources, not update/delete  
**Solution:** Implement PUT/DELETE commands for all resources  
**Impact:** Full resource lifecycle management from CLI  
**Suppressed APIs:** `ApiClient::put()`, `ApiClient::delete()`

### 3. Multi-Profile Support (2-3 hours) ⭐
**Problem:** `--profile` flag declared but not wired up  
**Solution:** Enable `attune --profile prod action list` workflows  
**Impact:** Seamless multi-environment operations  
**Suppressed APIs:** `CliConfig::load_with_profile()`

### 4. Advanced Search/Filtering (6-8 hours)
**Problem:** No filtering on list commands  
**Solution:** Add query parameters: `attune execution list --status=running --limit=10`  
**Suppressed APIs:** `ApiClient::get_with_query()`

### 5. Executor Monitoring (6-10 hours)
**Problem:** No visibility into queue depths or policy enforcement  
**Solution:** Admin API endpoints + CLI commands to inspect executor state  
**Suppressed APIs:** `QueueManager::get_all_queue_stats()`, policy methods

## Implementation Phases

```
Phase 1 (Weeks 1-2): Token Refresh          [CRITICAL]
Phase 2 (Weeks 3-4): CRUD Completion        [HIGH PRIORITY]
Phase 3 (Week 5):    Profile Management     [NICE TO HAVE]
Phase 4 (Weeks 6-8): Executor Monitoring    [OPERATIONAL]
```

**Total Effort:** 26-39 hours (3-5 weeks part-time)

## Quick Wins (Can Start Immediately)

1. **Token Refresh** - High impact, no dependencies
2. **--profile Flag** - Low effort, immediate value for multi-env workflows
3. **Delete Commands** - Complete the CRUD story

## What We're NOT Doing

- **Test helpers:** Keep suppressed—they're infrastructure
- **Redundant methods:** Remove `set_api_url()` (use `set_value()` instead)
- **Service internal fields:** Keep for future features

## Success Metrics

After completion:
- ✅ CLI sessions last >1 hour without re-auth
- ✅ Full CRUD on all resources from CLI
- ✅ `--profile` flag works seamlessly
- ✅ Can monitor executor queues in production
- ✅ Zero `#[allow(dead_code)]` on implemented features

## Next Actions

1. Review detailed plan in `docs/api-completion-plan.md`
2. Decide which phase(s) to prioritize
3. Create GitHub issues for selected phases
4. Start with Phase 1 (token refresh) - highest ROI

## Questions to Answer

1. **Phase 4 Architecture:** Should executor monitoring use HTTP API or pub/sub via RabbitMQ?
2. **Scope:** Implement all phases or stop after Phase 3?
3. **Timeline:** Target completion date?

---

**See:** `attune/docs/api-completion-plan.md` for full implementation details
