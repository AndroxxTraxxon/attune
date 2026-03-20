# Gitea Registry And Helm Publishing

This repository now includes:

- A Gitea Actions publish workflow at `.gitea/workflows/publish.yml`
- OCI-published container images for the Kubernetes deployment path
- A Helm chart at `charts/attune`

## What Gets Published

The workflow publishes these images to the Gitea OCI registry:

- `attune-api`
- `attune-executor`
- `attune-worker`
- `attune-sensor`
- `attune-notifier`
- `attune-web`
- `attune-migrations`
- `attune-init-user`
- `attune-init-packs`

The Helm chart is pushed as an OCI chart to:

- `oci://<registry>/<namespace>/helm/attune`

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

Chart packaging behavior:

- branch pushes package the chart as `0.0.0-dev.<run_number>`
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

Install the chart:

```bash
helm install attune oci://gitea.example.com/<namespace>/helm/attune \
  --version 0.3.0 \
  --set global.imageRegistry=gitea.example.com \
  --set global.imageNamespace=<namespace> \
  --set global.imageTag=0.3.0 \
  --set web.config.apiUrl=https://attune.example.com/api \
  --set web.config.wsUrl=wss://attune.example.com/ws
```

For a branch build:

```bash
helm install attune oci://gitea.example.com/<namespace>/helm/attune \
  --version 0.0.0-dev.<run_number> \
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
