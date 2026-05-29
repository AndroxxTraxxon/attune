# Secret Execution Parameter RBAC Plan

## Problem

Attune currently has partial secret management, but secret values that become execution parameters are not protected by a distinct RBAC boundary.

Encrypted keys use separate `keys:read` and `keys:decrypt` checks, but execution and enforcement APIs return stored parameter JSON directly. This means a user who can generally read executions or enforcements for an action can also see any secret values that were resolved into those records.

There is also no sensitivity-aware template flow. Values from encrypted keys, secret pack configuration fields, or secret execution outputs can be templated into non-secret action parameters without enforcement.

## Current State

- `GET /api/v1/keys/{ref}` separately checks `keys:decrypt` before returning decrypted encrypted-key values.
- Manual execution parameters are stored in `execution.config`.
- Rule-triggered resolved action parameters are stored in `enforcement.config`, then copied into `execution.config`.
- `ExecutionResponse.config` returns execution parameters directly.
- `EnforcementResponse.config` and `EnforcementResponse.payload` return enforcement data directly.
- Action, trigger, pack, and workflow schemas allow `secret: true`, but parameter validation strips that marker before JSON Schema validation and does not use it for authorization or data-flow checks.
- The worker loads all system, pack, and action keys for an action, decrypts them, and merges them into the action parameter document delivered to stdin.

## Goals

- Let users read execution/enforcement metadata without seeing secret parameter values.
- Make viewing secret execution/enforcement values a separate grant.
- Preserve enough information for workers to receive required secrets securely.
- Prevent secret template sources from flowing into action parameters that are not marked `secret: true`.
- Keep the model consistent with existing RBAC concepts and constraints.

## Proposed RBAC Model

Reuse the existing `Action::Decrypt` permission for secret value disclosure:

- `executions:read`: read execution metadata, status, non-secret config, and non-secret result data.
- `executions:decrypt`: reveal secret execution parameter/result values.
- `enforcements:read`: read enforcement metadata and non-secret resolved parameters/payload.
- `enforcements:decrypt`: reveal secret enforcement resolved parameters/payload.

Both grants should support existing constraints such as `pack_refs`, `refs`, `ids`, owner/executor scope, and execution hierarchy scope.

Default API behavior should redact secret values. Endpoints that can reveal secrets should require an explicit request flag such as `include_secret_values=true`, then check the corresponding `*:decrypt` permission and emit a secret-access audit event.

## Storage Model

Do not store plaintext secret values inline in general-purpose JSON fields.

Recommended shape:

- Keep non-secret values in `execution.config`, `execution.result`, and `enforcement.config`.
- Replace secret paths in those JSON documents with a stable redaction marker, for example:

```json
{
  "api_token": {
    "$attune_secret": true,
    "redacted": true
  }
}
```

- Add a secret value table for encrypted per-entity values:

```sql
CREATE TABLE execution_secret_value (
    id BIGSERIAL PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id BIGINT NOT NULL,
    json_path TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_ref TEXT,
    encrypted_value JSONB NOT NULL,
    encryption_key_hash TEXT,
    created TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_type, entity_id, json_path)
);
```

`entity_type` can initially cover `execution_config`, `execution_result`, and `enforcement_config`. This can later be normalized into separate tables if query patterns justify it.

## Sensitivity-Aware Template Resolution

Template resolution should return both the resolved value and sensitivity metadata.

Secret sources:

- Encrypted keys from the key store.
- Pack configuration fields whose `conf_schema` path has `secret: true`.
- Execution or task outputs whose `out_schema` path has `secret: true`.

Rules:

- A single-template value inherits the source value type and secret marker.
- A string interpolation containing any secret source becomes secret.
- Objects and arrays are secret at the specific descendant paths that include secret values.
- Secret metadata should be tracked by JSON path so only affected fields are redacted/encrypted.

## Secret-To-Secret Parameter Enforcement

Before creating or updating rules, workflows, work queues, and executions that use templates:

- Resolve or statically analyze template sources where possible.
- Determine which destination action parameters receive secret values.
- Load the destination action `param_schema`.
- Reject any secret value assigned to an action parameter that is not marked `secret: true`.

This should apply to:

- Manual execution requests.
- Rule `action_params`.
- Workflow task `input`.
- Work queue `action_params`.
- Retry and child-execution creation paths that copy or render parameters.

Secret values should not be allowed in execution `env_vars`; environment variables are too easy to leak through process inspection, child processes, crash reports, and logs.

## Worker Delivery

The worker should receive decrypted secret values only for the execution it is running.

Replace implicit action-wide secret loading with explicit, per-execution secret delivery:

- Scheduler/API resolves authorized secret templates into encrypted `execution_secret_value` rows.
- Worker loads the execution's secret value rows, decrypts them using the server-side encryption key, reconstructs the full parameter document, and passes it through stdin or secure parameter files.
- The worker no longer automatically loads all system, pack, and action keys for an action.

This moves secret access control to execution creation time and avoids granting every execution of an action every key scoped to that action or pack.

## API Behavior

Default read:

- Return execution/enforcement JSON with secret paths redacted.
- Include metadata that a value is redacted, but not source refs unless source refs themselves are considered safe.

Privileged read:

- Require explicit opt-in, for example `include_secret_values=true`.
- Check `executions:decrypt` or `enforcements:decrypt` against the target entity context.
- Decrypt and splice secret values into the response.
- Emit an audit event including actor, entity, action, paths disclosed, and source refs, but never raw secret values.

## Migration Strategy

The project is pre-production, so breaking schema and API changes are acceptable.

Recommended sequence:

1. Add secret-value storage and shared redaction/splicing helpers.
2. Add `executions:decrypt` and `enforcements:decrypt` to permission metadata and UI/API docs.
3. Redact existing execution/enforcement responses by default.
4. Add explicit privileged reveal behavior and audit events.
5. Add sensitivity-aware template resolver output.
6. Enforce secret-to-secret parameter rules for rules, workflows, work queues, and manual executions.
7. Replace worker action-wide key loading with per-execution secret loading.
8. Update tests, docs, and UI labels for redacted values.

## Testing

Add tests for:

- A user with `executions:read` but not `executions:decrypt` sees redacted secret parameters.
- A user with constrained `executions:decrypt` sees secret parameters only for matching actions/packs/executions.
- Equivalent `enforcements:read` and `enforcements:decrypt` behavior.
- Encrypted key templates into non-secret action parameters are rejected.
- Secret pack config templates into non-secret action parameters are rejected.
- Secret execution output templates into non-secret workflow task inputs are rejected.
- Secret templates into `env_vars` are rejected.
- Worker execution reconstructs secret parameters from per-execution encrypted rows.
- Audit events never include raw secret values.
