# Remove `---ATTUNE_PARAMS_END---` Delimiter Antipattern

**Date:** 2026-02-10

## Summary

Removed all instances of the `---ATTUNE_PARAMS_END---` stdin delimiter from the entire project — source code, shell scripts, and documentation. This was an antipattern from the old two-phase stdin protocol where parameters and secrets were delivered as separate documents separated by this delimiter. The current protocol merges secrets into parameters as a single JSON document delivered via one `readline()`, making the delimiter unnecessary.

## Background

The original stdin protocol wrote parameters and secrets in two phases:
1. Parameters JSON + `\n---ATTUNE_PARAMS_END---\n`
2. Secrets JSON + `\n`

This was already fixed in `process_executor.rs` and `shell.rs` (which write a single merged document followed by `\n`), but `native.rs` still had the old protocol, and all shell scripts and documentation still referenced it.

## Changes Made

### Source Code (1 file)

**`crates/worker/src/runtime/native.rs`**:
- Removed the `---ATTUNE_PARAMS_END---` delimiter write from `execute_binary()`
- Removed the separate secrets-writing block (matching the fix already applied to `shell.rs` and `process_executor.rs`)
- Added secrets-into-parameters merge in `execute()` before `prepare_parameters()` is called
- Now passes `&std::collections::HashMap::new()` for secrets to `execute_binary()`
- Stdin protocol is now: `{merged_params}\n` then close — consistent across all runtimes

### Shell Scripts (9 files)

Updated all pack action scripts to read stdin until EOF instead of looking for the delimiter:

- `packs/core/actions/echo.sh`
- `packs/core/actions/sleep.sh`
- `packs/core/actions/noop.sh`
- `packs/core/actions/http_request.sh`
- `packs/core/actions/build_pack_envs.sh`
- `packs/core/actions/download_packs.sh`
- `packs/core/actions/get_pack_dependencies.sh`
- `packs/core/actions/register_packs.sh`
- `packs/examples/actions/list_example.sh`

In each script, removed the `*"---ATTUNE_PARAMS_END---"*) break ;;` case pattern. The `while IFS= read -r line` loop now terminates naturally at EOF when stdin is closed.

### Documentation (9 files)

- `docs/QUICKREF-dotenv-shell-actions.md` — Updated template, format spec, and parsing examples
- `docs/action-development-guide.md` — Updated stdin protocol description, all Python/Node.js/Shell examples, troubleshooting section
- `docs/actions/QUICKREF-parameter-delivery.md` — Updated copy-paste templates and design change section
- `docs/actions/README.md` — Updated quick start Python example
- `docs/actions/parameter-delivery.md` — Updated protocol description, stdin content example, all code examples
- `docs/packs/pack-structure.md` — Updated Python stdin example
- `docs/parameters/dotenv-parameter-format.md` — Updated parsing examples, secret handling docs, troubleshooting

### Work Summaries (5 files)

- `work-summary/2025-02-05-FINAL-secure-parameters.md`
- `work-summary/2025-02-05-secure-parameter-delivery.md`
- `work-summary/2026-02-09-core-pack-jq-elimination.md`
- `work-summary/2026-02-09-dotenv-parameter-flattening.md`
- `work-summary/2026-02-action-execution-fixes.md`
- `work-summary/changelogs/CHANGELOG.md`

## Verification

- `cargo check --all-targets --workspace` — zero errors, zero warnings
- `cargo test -p attune-worker` — all 15 tests pass (including 7 security tests)
- Manual shell script testing — `echo.sh`, `sleep.sh`, `noop.sh` all work correctly with EOF-based reading
- `grep -r ATTUNE_PARAMS_END` — zero matches remaining in entire project

## Current Stdin Protocol (All Runtimes)

```
{merged_parameters_json}\n
<stdin closed>
```

- Secrets are merged into the parameters map by the caller before formatting
- Actions receive a single document via `readline()` or `read()`
- Shell scripts using DOTENV format read `key='value'` lines until EOF
- No delimiter, no two-phase protocol, no separate secrets document