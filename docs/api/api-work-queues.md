# Work Queue API

## Enqueue permissions

Creating a work queue item requires a `queues:create` permission grant for the target queue. The enqueue endpoint authorizes against the queue itself, so grant constraints follow the same resource-scope semantics used elsewhere in RBAC:

| Scope | Grant shape | Meaning |
|-------|-------------|---------|
| Queue-scoped | `{"resource":"queues","actions":["create"],"constraints":{"refs":["ops.review"]}}` | Can enqueue items only in queue `ops.review`. |
| Queue ID-scoped | `{"resource":"queues","actions":["create"],"constraints":{"ids":[42]}}` | Can enqueue items only in queue id `42`. |
| Pack-scoped | `{"resource":"queues","actions":["create"],"constraints":{"pack_refs":["ops"]}}` | Can enqueue items in any queue owned by pack `ops`. |
| System-scoped | `{"resource":"queues","actions":["create"]}` | Can enqueue items in any queue. |

Use the most specific grant possible. For pack-owned queues, prefer `refs` when granting access to a single queue and `pack_refs` when granting access to all queues in a pack. Omitting constraints grants access across all queues.

## Dispatch execution permissions

Work queues can also define `permission_set_refs` to control the execution-scoped API token granted to executions dispatched from that queue. Omit or set `permission_set_refs` to `null` to inherit the dispatch action's defaults, set it to `[]` to force no execution API token, or set one or more permission-set refs to grant that exact execution access. API-created queue overrides must be delegable by the caller.
