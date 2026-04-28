# Examples Pack

**Demonstration actions and workflows for learning Attune**

## Overview

The Examples pack provides reference implementations that demonstrate various Attune features and best practices. These examples are designed for learning and can be used as templates for building your own actions.

## Contents

### Actions

#### `list_example` - JSON Lines Output Demo

Demonstrates the JSON Lines (JSONL) output format for streaming results.

**Features:**
- Streams multiple JSON objects as output
- Each line is a separate JSON object
- Results are collected into an array
- Useful for processing lists or progress updates

**Usage:**
```bash
attune action execute examples.list_example --param count=10
```

**Parameters:**
- `count` (integer): Number of items to generate (default: 5, range: 1-100)

**Output Format:** JSONL - Each line is parsed as JSON and collected into an array

**Example Output:**
```json
[
  {"id": 0, "value": "item-0", "timestamp": "2024-01-20T10:30:00Z"},
  {"id": 1, "value": "item-1", "timestamp": "2024-01-20T10:30:01Z"},
  {"id": 2, "value": "item-2", "timestamp": "2024-01-20T10:30:02Z"}
]
```

#### `prompt_copilot` - General-Purpose GitHub Copilot + Attune MCP Action

Runs a one-shot GitHub Copilot CLI prompt with the local `attune-mcp` binary wired into Copilot's MCP config so the agent can call Attune tools (`actions.list`, `actions.execute`, `executions.get`, `artifacts.*`, etc.) using the **current execution's scoped API token**.

**Features:**
- Builds a temporary Copilot MCP config on the fly pointing at `/opt/attune/agent/attune-mcp`
- Passes the current execution's `ATTUNE_API_TOKEN` to `attune-mcp`
- Uses the examples pack's isolated Node.js environment to provide the `copilot` CLI via `package.json`
- Keeps the action itself as a **shell script**
- Requires a worker that advertises **Node.js capability** via `required_worker_runtimes: { node: ">=20" }`
- Disables Copilot's built-in GitHub MCP server by default so prompts focus on Attune tools
- Uses `copilot -p ... --additional-mcp-config @file` for scripted execution
- Emits a structured JSON envelope on stdout (`output_format: json`) so workflow tasks can read `task.<name>.result.final_output.*`

**Output envelope (`execution.result`):**
```json
{
  "final_output": { /* parsed JSON if Copilot emitted JSON, else { "text": "<raw stdout>" } */ },
  "raw_text": false,
  "exit_code": 0
}
```

**Requirements:**
- The action runs on the **shell runtime**, but it also requires a **Node-capable worker** so it can use the pack-scoped Copilot CLI install
- The examples pack environment must be built so `@github/copilot` is installed (first execution can also create it lazily on the worker)
- GitHub Copilot authentication must be available through `copilot_token`, `GH_TOKEN`, or `GITHUB_TOKEN`
- Or store the Copilot token in an Attune key and pass `copilot_token_key_ref`
- The worker must have access to `/opt/attune/agent/attune-mcp`
- For key-backed token lookup, the worker should also have `/opt/attune/agent/attune`

Optional prebuild of the examples pack Node.js environment:
```bash
attune action execute core.build_pack_envs --param pack_ref=examples
```

**Usage:**
```bash
attune action execute examples.prompt_copilot \
  --param prompt="List which Attune actions are visible via MCP and summarize them." \
  --param copilot_token="$GITHUB_TOKEN" --wait
```

Using an Attune key instead of a worker env var:
```bash
attune key create \
  --ref github_copilot_token \
  --name "GitHub Copilot Token" \
  --value "$GITHUB_TOKEN" \
  --encrypt

attune action execute examples.prompt_copilot \
  --param prompt="..." \
  --param copilot_token_key_ref=github_copilot_token --wait
```

#### `news_via_copilot` - AI-Driven News Workflow Demo

End-to-end demo that exercises the full Copilot ↔ Attune MCP loop **inside an Attune workflow**:

1. **`prompt`** task — calls `examples.prompt_copilot` with a prompt instructing Copilot to:
   - call `actions.list` (Attune MCP) to discover available actions,
   - call `actions.execute` on `core.http_request` against a public news endpoint (default: Hacker News Algolia front-page API),
   - summarize the top story and return `{"headline","url","summary"}` as JSON.
2. **`announce`** task — `core.echo` of the headline / URL / summary parsed out of the prompt task's JSON envelope.

The graph lives in `actions/workflows/news_via_copilot.workflow.yaml` and is referenced from `actions/news_via_copilot.yaml` via the `workflow_file` field.

**Setup (one-time):**
```bash
# 1. Build the examples pack Node env so @github/copilot is available
attune action execute core.build_pack_envs --param pack_ref=examples --wait

# 2. Store a Copilot-enabled GitHub PAT in the Attune key store
attune key create \
  --ref github_copilot_token \
  --name "GitHub Copilot Token" \
  --value "$GITHUB_TOKEN" \
  --encrypt
```

**Run the workflow:**
```bash
attune action execute examples.news_via_copilot \
  --param copilot_token_key_ref=github_copilot_token --wait
```

You can also override the news endpoint or the prompt:
```bash
attune action execute examples.news_via_copilot \
  --param copilot_token_key_ref=github_copilot_token \
  --param news_endpoint="https://hn.algolia.com/api/v1/search?tags=front_page&hitsPerPage=1" \
  --wait
```

**What this demonstrates:**
- The new `attune-mcp` server functioning **inside** an action execution context
- An external AI agent (Copilot CLI) discovering and invoking Attune actions via MCP using only the execution's scoped token
- Multi-task workflow plumbing: structured JSON output from one task feeding into the inputs of the next
- The agent binary injection pattern (`/opt/attune/agent/{attune-mcp,attune}`) being consumed end-to-end

## Use Cases

### Learning Attune
- Study action structure and metadata
- Understand parameter schemas
- Learn about different output formats
- See working implementations
- See how an AI agent can connect to Attune through MCP from inside an action execution

### Templates
- Copy and modify examples for your own actions
- Reference implementations for common patterns
- Starting point for new packs

## Installation

The examples pack is not installed by default but can be easily added:

```bash
# Via pack registry (if published)
attune pack install examples

# Via local directory
attune pack install --local ./packs/examples
```

## Development

### Adding New Examples

When adding new example actions:

1. Create action metadata in `actions/<name>.yaml`
2. Implement the action script in `actions/<name>.sh` (or .py, .js)
3. Use ref format: `examples.<action_name>`
4. Add documentation to this README
5. Include clear comments in the code
6. Demonstrate a specific feature or pattern

### Guidelines

- **Keep it simple** - Examples should be easy to understand
- **One concept per example** - Focus on demonstrating one feature clearly
- **Well-commented** - Explain what the code does and why
- **Self-contained** - Minimize external dependencies
- **Documented** - Update this README with usage examples

## Related Documentation

- [Action Development Guide](../../docs/action-development-guide.md)
- [Pack Structure](../../docs/packs/pack-structure.md)
- [Parameter Configuration](../../docs/action-development-guide.md#parameter-configuration)
- [Output Formats](../../docs/action-development-guide.md#output-configuration)

## Contributing

Have an idea for a useful example? Contributions are welcome! Please ensure:

- Examples are educational and demonstrate best practices
- Code is well-commented and easy to follow
- Documentation is updated
- Examples are tested and working

## License

This pack is part of the Attune project and follows the same license terms.
