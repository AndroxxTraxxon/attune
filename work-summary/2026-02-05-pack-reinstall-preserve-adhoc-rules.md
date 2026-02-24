# Pack Reinstallation: Preserve Ad-Hoc Rules

**Date**: 2026-02-05

## Problem

When reinstalling a pack (force=true), user-created (ad-hoc) rules belonging to that pack were being permanently deleted. This happened because the reinstallation flow performed a hard `PackRepository::delete()` before recreating the pack, and the `rule.pack` foreign key uses `ON DELETE CASCADE` — destroying all rules owned by the pack, including custom ones created by users through the API or UI.

Additionally, rules from *other* packs that referenced triggers or actions from the reinstalled pack would have their `action` and `trigger` FK columns set to `NULL` (via `ON DELETE SET NULL`) when the old pack's entities were cascade-deleted, but were never re-linked after the new entities were created.

## Root Cause

In `register_pack_internal()` (`crates/api/src/routes/packs.rs`), the force-reinstall path was:

```
1. Delete existing pack (CASCADE deletes ALL rules, actions, triggers, sensors, runtimes)
2. Create new pack + components
```

No distinction was made between pack-defined rules (`is_adhoc = false`) and user-created rules (`is_adhoc = true`).

## Solution

### Repository Changes (`crates/common/src/repositories/rule.rs`)

Added four new methods/types:

- **`RestoreRuleInput`** — Like `CreateRuleInput` but with `Option<Id>` for action and trigger, since referenced entities may not exist after reinstallation.
- **`find_adhoc_by_pack()`** — Queries ad-hoc rules (`is_adhoc = true`) belonging to a specific pack.
- **`restore_rule()`** — Inserts a rule with optional action/trigger FK IDs, always setting `is_adhoc = true`.
- **`relink_action_by_ref()` / `relink_trigger_by_ref()`** — Updates rules with NULL action/trigger FKs, matching by the text `_ref` field to re-establish the link.

### Pack Registration Changes (`crates/api/src/routes/packs.rs`)

Modified `register_pack_internal()` to add two phases after component loading:

**Phase 1 — Save & Restore Ad-Hoc Rules:**
- Before deleting the old pack, queries and saves all ad-hoc rules
- After the new pack and components are created, restores each saved rule with the new pack ID
- Resolves action/trigger FKs by looking up entities by ref; if not found, the rule is preserved with NULL FKs (non-functional but not lost)

**Phase 2 — Re-link Orphaned Rules from Other Packs:**
- Iterates over all newly created actions and triggers
- For each, updates any rules (from any pack) that have a matching `_ref` but a NULL FK

## Files Changed

| File | Change |
|------|--------|
| `crates/common/src/repositories/rule.rs` | Added `RestoreRuleInput`, `find_adhoc_by_pack()`, `restore_rule()`, `relink_action_by_ref()`, `relink_trigger_by_ref()` |
| `crates/api/src/routes/packs.rs` | Save ad-hoc rules before pack deletion; restore them and re-link orphaned cross-pack rules after component loading |

## Testing

- Zero compiler warnings across the workspace
- All unit tests pass
- Integration test failures are pre-existing (no `attune_test` database configured)