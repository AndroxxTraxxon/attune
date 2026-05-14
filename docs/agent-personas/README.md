# Attune Pack Developer Agent Personas

This directory contains reusable, self-contained AI agent personas for developing Attune pack contents. Use these prompts when you want an agent to specialize in one part of pack authoring instead of giving generic automation advice. Each persona includes the core Attune conventions it needs inline, so it can be copied into a workspace that does not contain this repository.

## Persona set

| Persona | File | Use when |
| --- | --- | --- |
| Attune Pack Architect | [pack-architect.md](pack-architect.md) | Planning a complete pack, directory layout, refs, runtime/config strategy, and delegation to specialist personas. |
| Attune Action Author | [action-author.md](action-author.md) | Writing conventional action YAML and entrypoint scripts with correct schemas, stdin parameters, runtimes, and outputs. |
| Attune MCP Agent Action Author | [ai-agent-action-author.md](ai-agent-action-author.md) | Writing AI agent actions that use execution-scoped Attune MCP access through `attune-mcp`. |
| Attune Rule Author | [rule-author.md](rule-author.md) | Connecting trigger events to action executions with correct rules, conditions, and `action_params` mappings. |
| Attune Workflow Author | [workflow-author.md](workflow-author.md) | Writing workflow actions and graph-only workflow YAML using current Attune workflow conventions. |
| Attune Sensor Author | [sensor-author.md](sensor-author.md) | Writing triggers, sensor YAML, and sensor implementations that emit events through the supported sensor interface. |
| Attune CLI Guide | [cli-guide.md](cli-guide.md) | Using the `attune` CLI and `attune-mcp` for pack upload/register, workflow upload, execution, artifacts, keys, auth, and MCP launch. |
| Attune Pack Test Reviewer | [pack-test-reviewer.md](pack-test-reviewer.md) | Reviewing and testing pack contents before upload, registration, or publishing. |

## Suggested routing

Start with **Attune Pack Architect** for a new pack or large change. Use the more focused personas when the scope is already clear. Run **Attune Pack Test Reviewer** before publishing or when several component types changed together.
