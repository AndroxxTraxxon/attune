# Node.js Runtime Fix

**Date**: 2026-02
**Scope**: Worker runtime loading, environment setup, action execution

## Problem

Node.js actions from installed packs (e.g., `nodejs_example`) were stuck in "In Progress" status and never completing. Three root causes were identified:

### 1. Runtime Name Mismatch (Blocking)

The `ATTUNE_WORKER_RUNTIMES` env var for the node worker is `shell,node`, but the database runtime name is "Node.js" which lowercases to `"node.js"`. The worker's filter used exact string matching (`filter.contains(&rt_name)`), so `"node.js"` was never found in `["shell", "node"]`. **The Node.js ProcessRuntime was never registered**, meaning no worker could handle Node.js executions.

### 2. Broken Environment Setup

The Node.js `execution_config` in `packs/core/runtimes/nodejs.yaml` had:
- `create_command: ["npm", "init", "-y"]` — ran in the pack directory (read-only in Docker), didn't create anything at `{env_dir}`
- `install_command: ["npm", "install", "--prefix", "{pack_dir}"]` — tried to write `node_modules` into the read-only pack directory

### 3. Missing NODE_PATH

Even with correct installation, Node.js couldn't find modules installed at `{env_dir}/node_modules` because Node resolves modules relative to the script location, not the environment directory. No mechanism existed to set `NODE_PATH` during execution.

## Solution

### Runtime Name Normalization

Added `normalize_runtime_name()` and `runtime_in_filter()` to `crates/common/src/runtime_detection.rs`. These functions map common aliases to canonical names:
- `node` / `nodejs` / `node.js` → `node`
- `python` / `python3` → `python`
- `shell` / `bash` / `sh` → `shell`
- `native` / `builtin` / `standalone` → `native`

Updated `crates/worker/src/service.rs` and `crates/worker/src/env_setup.rs` to use `runtime_in_filter()` instead of exact string matching.

### RuntimeExecutionConfig.env_vars

Added an `env_vars: HashMap<String, String>` field to `RuntimeExecutionConfig` in `crates/common/src/models.rs`. Values support the same template variables as other fields (`{env_dir}`, `{pack_dir}`, `{interpreter}`, `{manifest_path}`).

In `ProcessRuntime::execute` (`crates/worker/src/runtime/process.rs`), runtime env_vars are resolved and injected into the action's environment before building the command.

### Fixed Node.js Runtime Config

Updated `packs/core/runtimes/nodejs.yaml` and `scripts/seed_runtimes.sql`:

```yaml
execution_config:
  interpreter:
    binary: node
    args: []
    file_extension: ".js"
  environment:
    env_type: node_modules
    dir_name: node_modules
    create_command:
      - sh
      - "-c"
      - "mkdir -p {env_dir} && cp {manifest_path} {env_dir}/ 2>/dev/null || true"
    interpreter_path: null
  dependencies:
    manifest_file: package.json
    install_command:
      - npm
      - install
      - "--prefix"
      - "{env_dir}"
  env_vars:
    NODE_PATH: "{env_dir}/node_modules"
```

## Files Changed

| File | Change |
|------|--------|
| `crates/common/src/models.rs` | Added `env_vars` field to `RuntimeExecutionConfig` |
| `crates/common/src/runtime_detection.rs` | Added `normalize_runtime_name()`, `runtime_matches_filter()`, `runtime_in_filter()` with tests |
| `crates/worker/src/service.rs` | Use `runtime_in_filter()` for ATTUNE_WORKER_RUNTIMES matching |
| `crates/worker/src/env_setup.rs` | Use `runtime_in_filter()` for runtime filter matching in env setup |
| `crates/worker/src/runtime/process.rs` | Inject `env_vars` into action environment during execution |
| `crates/worker/src/runtime/local.rs` | Added `env_vars` field to fallback config |
| `packs/core/runtimes/nodejs.yaml` | Fixed `create_command`, `install_command`, added `env_vars` |
| `scripts/seed_runtimes.sql` | Fixed Node.js execution_config to match YAML |
| `crates/worker/tests/*.rs` | Added `env_vars` field to test configs |
| `AGENTS.md` | Documented runtime name normalization, env_vars, and env setup requirements |

## Testing

- All 424 unit tests pass
- Zero compiler warnings
- New tests for `normalize_runtime_name`, `runtime_matches_filter`, `runtime_in_filter`

## Deployment Notes

After deploying, the Node.js runtime config in the database needs to be updated. This happens automatically when packs are re-registered (the pack loader reads the updated YAML). Alternatively, run the updated `seed_runtimes.sql` script.