# Gitea Registry And Helm Publishing

This repository now includes:

- A Gitea Actions publish workflow at `.gitea/workflows/publish.yml`
- OCI-published container images for the Kubernetes deployment path
- A Helm chart at `charts/attune`

## What Gets Published

The workflow publishes these images to the Gitea OCI registry:

- `attune/api`
- `attune/executor`
- `attune/notifier`
- `attune/agent`
- `attune/web`
- `attune/migrations`
- `attune/init-user`
- `attune/init-packs`

The Helm chart is pushed to Gitea's Helm package registry:

- `https://<gitea-host>/api/packages/<namespace>/helm`

## Required Gitea Repository Configuration

Set these variables:

- `CLUSTER_GITEA_HOST`: Registry hostname only, for example `gitea.example.com`
- `CONTAINER_REGISTRY_NAMESPACE`: Optional override for the registry namespace. If omitted, the workflow uses the repository owner.
- `CONTAINER_REGISTRY_INSECURE`: Optional boolean override for plain HTTP registry access. If omitted, the workflow auto-detects `*.svc.cluster.local` registry hosts and treats them as insecure/plain HTTP. Set this explicitly to force either behavior.

Set one of these authentication options:

- Preferred: `CONTAINER_REGISTRY_USERNAME` and `CONTAINER_REGISTRY_PASSWORD`
- Fallback: allow the workflow `GITHUB_TOKEN` or Gitea-provided token to push packages

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
`0.0.0.git.<run-number>.sha.<short-sha>`. If a broken publish ever leaves
legacy `sha-*` package versions in the Debian/RPM/Arch registries, remove them
with:

```bash
GITEA_USERNAME=<user> GITEA_TOKEN=<token> \
  scripts/delete-legacy-gitea-linux-packages.sh --execute
```

Run the script without `--execute` first to preview the package versions and
delete URLs. It discovers legacy versions from Debian, RPM, and Arch metadata,
including RPM/Arch release-suffixed versions such as `sha-...-1`. It defaults
to `https://git.rdrx.app`, namespace `attune-system`, Debian `stable/main`,
RPM group `el9`, and Arch repository `core`.

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
helm registry login gitea.example.com --username <user>
```

For a plain HTTP internal registry:

```bash
helm registry login gitea-http.gitea.svc.cluster.local --username <user> --plain-http
```

Add the Helm package repository:

```bash
helm repo add attune https://gitea.example.com/api/packages/<namespace>/helm \
  --username <user> \
  --password <token>
helm repo update
```

Install the chart:

```bash
helm install attune attune/attune \
  --version 0.3.0 \
  --set global.imageRegistry=gitea.example.com \
  --set global.imageNamespace=<namespace> \
  --set global.imageTag=0.3.0 \
  --set web.config.apiUrl=https://attune.example.com/api \
  --set web.config.wsUrl=wss://attune.example.com/ws
```

For a branch build:

```bash
helm install attune attune/attune \
  --version 0.0.0-edge.<run_number> \
  --set global.imageRegistry=gitea.example.com \
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
