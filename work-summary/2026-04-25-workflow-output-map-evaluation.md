# Workflow `output_map` evaluation at parent completion

## Problem
`WorkflowDefinition.output_map` was parsed and stored but never applied. The
parent execution of a workflow always had its `result` set to a hard-coded
`{"succeeded": true}`, so user-defined outputs (markdown summaries, structured
fields composed from task results, etc.) were silently dropped.

## Fix
In `crates/executor/src/scheduler.rs`:
- Added free fn `build_output_map_result(definition_json, &wf_ctx)` that parses
  the workflow definition, iterates `output_map`, and renders each value via
  `WorkflowContext::render_json` (type-preserving for pure `{{ expr }}` strings).
  Render errors are logged and the offending key is omitted; the workflow does
  not fail.
- Added free fn `build_workflow_result_payload(success, error_msg, output_override)`
  that constructs the parent execution result. On success with an output_map:
  user outputs become the top-level fields, with `succeeded: true` merged in
  only if the user didn't already define a `succeeded` key. On failure: legacy
  `{"error": ..., "succeeded": false}` shape.
- Threaded an `Option<JsonValue>` `result_override` parameter through both
  `complete_workflow` and `complete_workflow_with_conn`.
- The success-case caller in `advance_workflow_serialized` now evaluates
  `output_map` against the in-scope `wf_ctx` and passes the result down.
- The "no entry-points" early-exit path passes `None` (no context, no outputs).

## Demo
Workflow `examples.news_via_copilot` now returns:

```json
{
  "headline": "...",
  "markdown": "# ...\n\n[Read the original article](...)\n\n...",
  "raw_final_output": { "headline": "...", "summary": "...", "url": "..." },
  "succeeded": true,
  "summary": "...",
  "url": "..."
}
```

Multi-line markdown templates with multiple `{{ ... }}` substitutions render as
a single string; pure object expressions (`raw_final_output`) preserve their
type.
