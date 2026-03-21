# Universal Worker Agent Phase 7: Kubernetes Support

**Date**: 2026-02-05

## Summary

Implemented Kubernetes support for agent-based workers in the Attune Helm chart, completing Phase 7 of the Universal Worker Agent plan. Users can now deploy the `attune-agent` binary into any container image on Kubernetes using the InitContainer pattern — the same approach used by Tekton and Argo.

## Changes

### Helm Chart (`charts/attune/`)

- **`templates/agent-workers.yaml`** (new): Helm template that iterates over `agentWorkers[]` values and creates a Deployment per entry. Each Deployment includes:
  - `agent-loader` init container — copies the statically-linked `attune-agent` binary from the `attune-agent` image into an `emptyDir` volume
  - `wait-for-schema` init container — polls PostgreSQL until the Attune schema is ready
  - `wait-for-packs` init container — waits for the core pack on the shared PVC
  - Worker container — runs the user's chosen image with the agent binary as entrypoint
  - Volumes: `agent-bin` (emptyDir), `config` (ConfigMap), `packs` (PVC, read-only), `runtime-envs` (PVC), `artifacts` (PVC)

- **`values.yaml`**: Added `images.agent` (repository, tag, pullPolicy) and `agentWorkers: []` with full documentation of supported fields: `name`, `image`, `replicas`, `runtimes`, `resources`, `env`, `imagePullPolicy`, `logLevel`, `runtimeClassName`, `nodeSelector`, `tolerations`, `stopGracePeriod`

- **`templates/NOTES.txt`**: Updated to list enabled agent workers on install/upgrade

### CI/CD (`.gitea/workflows/publish.yml`)

- Added `attune-agent` to the image build matrix (target: `agent-init`, dockerfile: `docker/Dockerfile.agent`) so the agent image is published alongside all other Attune images

### Documentation

- **`docs/QUICKREF-kubernetes-agent-workers.md`** (new): Quick-reference guide covering how agent workers work on Kubernetes, all supported Helm values fields, runtime auto-detection table, differences from the standard worker, and troubleshooting steps
- **`docs/deployment/gitea-registry-and-helm.md`**: Added `attune-agent` to the published images list
- **`docs/plans/universal-worker-agent.md`**: Marked Phase 7 as complete with implementation details

### AGENTS.md

- Moved Phase 7 from "In Progress" to "Complete" with a summary of what was implemented

## Design Decisions

1. **emptyDir volume** (not PVC) for the agent binary — each pod gets its own copy via the init container. This avoids needing a shared RWX volume just for a single static binary and follows the standard Kubernetes sidecar injection pattern used by Tekton, Argo, and Istio.

2. **Pod-level scheduling fields** — `runtimeClassName`, `nodeSelector`, and `tolerations` are exposed at the pod spec level (not container level) to support GPU scheduling via NVIDIA RuntimeClass and node affinity for specialized hardware.

3. **Runtime auto-detect by default** — when `runtimes` is empty (the default), the agent probes the container for interpreters. Users can override with an explicit list to skip detection and limit which runtimes are registered.

4. **Consistent patterns** — the template reuses the same `wait-for-schema` and `wait-for-packs` init containers, `envFrom` secret injection, and volume mount structure as the existing worker Deployment in `applications.yaml`.