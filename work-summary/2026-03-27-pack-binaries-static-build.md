# Pack Binaries: Cross-Architecture Static Build

**Date**: 2026-03-27

## Problem

The `attune-core-timer-sensor` native sensor binary failed to execute in Docker containers on Apple Silicon (arm64) Macs with the error:

```
rosetta error: failed to open elf at /lib64/ld-linux-x86-64.so.2
```

**Two root causes**:

1. **Wrong build toolchain**: `docker/Dockerfile.pack-binaries` used a plain `cargo build` which produced a **dynamically-linked, host-architecture** binary. On arm64 Docker hosts, this created an aarch64 binary linked against glibc. When the sensor container tried to execute it, the required dynamic linker (`ld-linux-x86-64.so.2`) was absent. This contrasted with `docker/Dockerfile.agent`, which already used `cargo-zigbuild` + musl for fully static binaries.

2. **init-packs overwrote the static binary**: Even after fixing the Dockerfile, the `init-packs.sh` script did `cp -rf` from `./packs/` (host bind mount) into the packs volume, **overwriting** the freshly-placed static binary from `init-pack-binaries` with the old dynamically-linked binary from the host's `packs/core/sensors/` directory.

## Changes

### `docker/Dockerfile.pack-binaries` — Rewritten for static cross-compilation

- Added `RUST_TARGET` build arg (default: `x86_64-unknown-linux-musl`)
- Installed `musl-tools`, `ziglang`, and `cargo-zigbuild` (matching agent Dockerfile pattern)
- Replaced `cargo build --release` with `cargo zigbuild --release --target ${RUST_TARGET}`
- Added `cargo fetch` dependency caching layer with proper workspace stubs (including sensor `agent_main.rs`)
- Added `SQLX_OFFLINE=true` for compile-time query checking without a live database
- Added strip-with-fallback for cross-arch scenarios
- Added **Stage 3: `pack-binaries-init`** — busybox-based image for Docker Compose volume population (analogous to `agent-init`)
- Updated cache ID to `target-pack-binaries-static` with `sharing=locked` for zigbuild exclusivity

### `docker/init-packs.sh` — Preserve static binaries during pack copy

- Before copying host pack files, detects ELF binaries already present in the target `sensors/` directory using the 4-byte ELF magic number (`\x7fELF` = `7f454c46`) via `od` (available in python:3.11-slim, unlike `file`)
- Backs up detected ELF binaries to a temp directory before the `cp -rf` overwrites them
- Restores the backed-up static binaries after the copy completes
- Logs each preserved binary for visibility

### `docker-compose.yaml` — Added `init-pack-binaries` service

- New `init-pack-binaries` service builds from `Dockerfile.pack-binaries` (target: `pack-binaries-init`) and copies the static binary into the `packs_data` volume
- Accepts `PACK_BINARIES_RUST_TARGET` env var (default: `x86_64-unknown-linux-musl`)
- `init-packs` now depends on `init-pack-binaries` to ensure binaries are in the volume before pack files are copied
- `docker compose up` now automatically builds and deploys pack binaries — no manual script run required

### `docker/distributable/docker-compose.yaml` — Same pattern for distributable

- Added `init-pack-binaries` service using pre-built registry image
- Updated `init-packs` dependencies

### `scripts/build-pack-binaries.sh` — Updated for static builds

- Passes `RUST_TARGET` build arg to Docker build
- Accepts `RUST_TARGET` env var (default: `x86_64-unknown-linux-musl`)
- Updated verification output to expect statically-linked binary

### `Makefile` — New targets

- `PACK_BINARIES_RUST_TARGET` variable (default: `x86_64-unknown-linux-musl`)
- `docker-build-pack-binaries` — build for default architecture
- `docker-build-pack-binaries-arm64` — build for aarch64
- `docker-build-pack-binaries-all` — build both architectures

### `.gitignore` — Exclude compiled pack binary

- Added `packs/core/sensors/attune-core-timer-sensor` to `.gitignore`
- Removed the stale dynamically-linked binary from git tracking

## Architecture

The fix follows the same proven pattern as `Dockerfile.agent`:

```
cargo-zigbuild + musl → statically-linked binary → zero runtime dependencies
```

Since the binary has no dynamic library dependencies (no glibc, no libssl, no dynamic linker), it runs on **any** Linux container of the matching CPU architecture, regardless of the base image (Debian, Alpine, scratch, etc.).

### Init sequence

1. **`init-pack-binaries`**: Builds static musl binary → copies to `packs_data` volume at `core/sensors/`
2. **`init-packs`** (depends on `init-pack-binaries`): Copies host `./packs/core/` to volume → detects existing ELF binary → backs it up → copies host files → restores static binary
3. **`sensor`**: Spawns the static `attune-core-timer-sensor` → works on any architecture

## Usage

```bash
# Default (x86_64) — works on amd64 containers and arm64 via Rosetta
docker compose up -d

# For native arm64 containers
PACK_BINARIES_RUST_TARGET=aarch64-unknown-linux-musl docker compose up -d

# Standalone build
make docker-build-pack-binaries          # amd64
make docker-build-pack-binaries-arm64    # arm64
make docker-build-pack-binaries-all      # both

# Manual script
RUST_TARGET=aarch64-unknown-linux-musl ./scripts/build-pack-binaries.sh
```
