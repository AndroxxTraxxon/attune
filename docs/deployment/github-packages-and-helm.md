# GitHub Publishing And Nexus Linux Packages

This repository now includes:

- A GitHub Actions publish workflow at `.github/workflows/publish.yml`
- OCI-published container images for the Kubernetes deployment path
- A Helm chart at `charts/attune`
- Nexus-published Linux packages plus Docker distribution, Helm chart, and binary bundle archives

## What Gets Published

The workflow publishes these images to GitHub Container Registry by default:

- `attune/api`
- `attune/executor`
- `attune/notifier`
- `attune/supervisor`
- `attune/agent`
- `attune/web`
- `attune/migrations`
- `attune/init-user`
- `attune/init-packs`

The Helm chart is pushed as an OCI chart:

- `oci://ghcr.io/<namespace>/attune/charts`

Linux packages are published to Nexus Repository Manager 3. GitHub Packages
supports ecosystems such as OCI containers, npm, Maven, NuGet, RubyGems, and
Cargo, but it does not provide native Debian/RPM/Arch repository hosting.

Binary bundles are uploaded as per-architecture workflow artifacts named
`attune-binaries-amd64` and `attune-binaries-arm64`. Tag builds attach those
`attune-binaries-{arch}.tar.gz` files directly to the GitHub Release.

## Required GitHub Repository Configuration

Set these variables:

- `CONTAINER_REGISTRY_HOST`: Optional registry hostname override. If omitted, the workflow uses `ghcr.io`.
- `CONTAINER_REGISTRY_NAMESPACE`: Optional override for the registry namespace. If omitted, the workflow uses the repository owner lowercased. GHCR publishes with a lowercased namespace.
- `NEXUS_URL`: Base URL for Nexus, for example `https://nexus.example.com`.
- `NEXUS_APT_REPOSITORY`: Optional hosted apt repository name. Defaults to `attune-apt`.
- `NEXUS_YUM_REPOSITORY`: Optional hosted yum/RPM repository name. Defaults to `attune-yum`.
- `NEXUS_RAW_REPOSITORY`: Optional raw repository for Arch `.pkg.tar.zst` packages. If omitted, Arch package upload is skipped.
- `NEXUS_APT_COMPONENT`: Optional Debian component path segment. Defaults to `main`.

Set one of these container registry authentication options:

- Preferred: `CONTAINER_REGISTRY_USERNAME` and `CONTAINER_REGISTRY_PASSWORD`
- Fallback: allow the workflow `GITHUB_TOKEN` to push packages and release assets

Set these Nexus credentials as repository secrets:

- `NEXUS_USERNAME`
- `NEXUS_PASSWORD`

## Publish Behavior

The workflow runs on:

- pushes to `main`
- pushes to `master`
- tags matching `v*`
- manual dispatch

Tag behavior:

- branch pushes publish `edge` and `sha-<12-char-sha>`
- release tags like `v0.3.0` publish `0.3.0`, `latest`, and `sha-<12-char-sha>`

Linux package branch builds use package-manager-safe versions such as
`0.0.0.git.<run-number>.sha.<short-sha>`.

The Linux package set includes split packages for individual components and an
all-in-one `attune` installer package. The all-in-one package is self-contained:
it installs the API, executor, worker, sensor, notifier, supervisor, CLI, MCP,
and agent binaries together under `/opt/attune-system/`, installs service units
that run from that directory, and symlinks the interactive `attune` and
`attune-mcp` commands into `/usr/bin`. It conflicts with the split `attune-*`
packages so the same files are not owned by multiple packages. Use `attune-cli`
for a CLI-only install, or `attune` for a cohesive local service install.

Chart packaging behavior:

- branch pushes package the chart as `0.0.0-edge.<run_number>`
- release tags package the chart with the tag version, for example `0.3.0`

## Helm Install Flow

Log in to the registry:

```bash
helm registry login ghcr.io --username <user>
```

Install the chart:

```bash
helm install attune oci://ghcr.io/<namespace>/attune/charts/attune \
  --version 0.3.0 \
  --set global.imageRegistry=ghcr.io \
  --set global.imageNamespace=<namespace> \
  --set global.imageTag=0.3.0 \
  --set web.config.apiUrl=https://attune.example.com/api \
  --set web.config.wsUrl=wss://attune.example.com/ws
```

For a branch build:

```bash
helm install attune oci://ghcr.io/<namespace>/attune/charts/attune \
  --version 0.0.0-edge.<run_number> \
  --set global.imageRegistry=ghcr.io \
  --set global.imageNamespace=<namespace> \
  --set global.imageTag=edge
```

## Chart Expectations

The chart defaults to deploying:

- PostgreSQL via TimescaleDB
- RabbitMQ
- Redis
- Attune API, executor, worker, sensor, notifier, and web services
- Migration, test-user bootstrap, and built-in pack bootstrap jobs

Important constraints:

- The shared `packs`, `runtime_envs`, and `artifacts` claims default to `ReadWriteMany`
- Your cluster storage class must support RWX for the default values to work as written
- `web.config.apiUrl` and `web.config.wsUrl` must be browser-reachable URLs, not cluster-internal service DNS names
- The default security and bootstrap values in `charts/attune/values.yaml` are placeholders and should be overridden

## Suggested First Release Sequence

1. Push the workflow and chart changes to `main`.
2. Verify that the workflow publishes the `edge` images and dev chart package.
3. Create a release tag such as `v0.1.0`.
4. Install the chart using that exact image tag and chart version.
