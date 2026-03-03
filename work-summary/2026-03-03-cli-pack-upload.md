# CLI Pack Upload Command

**Date**: 2026-03-03  
**Scope**: `crates/cli`, `crates/api`

## Problem

The `attune pack register` command requires the API server to be able to reach the pack directory at the specified filesystem path. When the API runs inside Docker, this means the path must be inside a known container mount (e.g. `/opt/attune/packs.dev/...`). There was no way to install a pack from an arbitrary local path on the developer's machine into a Dockerized Attune system.

## Solution

Added a new `pack upload` CLI command and a corresponding `POST /api/v1/packs/upload` API endpoint. The CLI creates a `.tar.gz` archive of the local pack directory in memory and streams it to the API via `multipart/form-data`. The API extracts the archive and calls the existing `register_pack_internal` function, so all normal registration logic (component loading, workflow sync, MQ notifications) still applies.

## Changes

### New API endpoint: `POST /api/v1/packs/upload`
- **File**: `crates/api/src/routes/packs.rs`
- Accepts `multipart/form-data` with:
  - `pack` (required) — `.tar.gz` archive of the pack directory
  - `force` (optional) — `"true"` to overwrite an existing pack
  - `skip_tests` (optional) — `"true"` to skip test execution
- Extracts the archive to a temp directory using `flate2` + `tar`
- Locates `pack.yaml` at the archive root or one level deep (handles GitHub-style tarballs)
- Reads the pack `ref`, moves the directory to permanent storage, then calls `register_pack_internal`
- Added helper: `find_pack_root()` walks up to one level to find `pack.yaml`

### New CLI command: `attune pack upload <path>`
- **File**: `crates/cli/src/commands/pack.rs`
- Validates the local path exists and contains `pack.yaml`
- Reads `pack.yaml` to extract the pack ref for display messages
- Builds an in-memory `.tar.gz` using `tar::Builder` + `flate2::GzEncoder`
- Helper `append_dir_to_tar()` recursively archives directory contents with paths relative to the pack root (symlinks are skipped)
- Calls `ApiClient::multipart_post()` with the archive bytes
- Flags: `--force` / `--skip-tests`

### New `ApiClient::multipart_post()` method
- **File**: `crates/cli/src/client.rs`
- Accepts a file field (name, bytes, filename, MIME type) plus a list of extra text fields
- Follows the same 401-refresh-then-error pattern as other methods
- HTTP client timeout increased from 30s to 300s for uploads

### `pack register` UX improvement
- **File**: `crates/cli/src/commands/pack.rs`
- Emits a warning when the supplied path looks like a local filesystem path (not under `/opt/attune/`, `/app/`, etc.), suggesting `pack upload` instead

### New workspace dependencies
- **Workspace** (`Cargo.toml`): `tar = "0.4"`, `flate2 = "1.0"`, `tempfile` moved from testing to runtime
- **API** (`crates/api/Cargo.toml`): added `tar`, `flate2`, `tempfile`
- **CLI** (`crates/cli/Cargo.toml`): added `tar`, `flate2`; `reqwest` gains `multipart` + `stream` features

## Usage

```bash
# Log in to the dockerized system
attune --api-url http://localhost:8080 auth login \
  --username test@attune.local --password 'TestPass123!'

# Upload and register a local pack (works from any machine)
attune --api-url http://localhost:8080 pack upload ./packs.external/python_example \
  --skip-tests --force
```

## Verification

Tested against a live Docker Compose stack:
- Pack archive created (~13 KB for `python_example`)
- API received, extracted, and stored the pack at `/opt/attune/packs/python_example`
- All 5 actions, 1 trigger, and 1 sensor were registered
- `pack.registered` MQ event published to trigger worker environment setup
- `attune action list` confirmed all components were visible