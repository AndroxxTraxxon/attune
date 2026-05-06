# Worker placement overrides

Implemented Kubernetes-style worker placement across action defaults, manual execution overrides, and workflow task overrides.

- Added nullable execution-level placement overrides for `worker_selector`, `worker_tolerations`, and `worker_affinity`.
- Manual executions can provide placement overrides; omitted fields inherit action defaults, while explicit empty objects/arrays clear the field.
- Workflow tasks can declare templatable placement overrides, rendered per child execution and per `with_items` item.
- Scheduler worker selection now computes effective placement from execution overrides plus action defaults before applying hard filters and preferred-affinity scoring.
- Added e2e-style scheduler tests covering label selection, taint avoidance/toleration, execution overrides, and workflow task child-execution placement.
