# Documentation Organization

## Overview

The Attune project documentation has been reorganized into logical subdirectories to improve discoverability and maintainability. This document describes the new structure and rationale.

## Documentation Structure

### `docs/` Directory

#### `docs/api/`
**Purpose**: REST API endpoint documentation and OpenAPI specifications

**Contents**:
- API endpoint documentation (`api-*.md`)
- OpenAPI client generation guides
- API completion plans and specifications

**When to use**: Creating or documenting REST API endpoints, working with OpenAPI specs

#### `docs/architecture/`
**Purpose**: System architecture and service design documentation

**Contents**:
- Service architecture documents (`*-service.md`)
- System architecture overviews (`*-architecture.md`)
- Queue and message broker architecture
- Inter-service communication patterns

**When to use**: Understanding system design, planning new services, architectural decisions

#### `docs/authentication/`
**Purpose**: Authentication, authorization, and security documentation

**Contents**:
- Authentication mechanisms (JWT, tokens)
- Secrets management
- RBAC and permissions
- Service accounts
- Security reviews and guidelines

**When to use**: Implementing auth features, managing secrets, security audits

#### `docs/cli/`
**Purpose**: Command-line interface documentation

**Contents**:
- CLI command reference
- Profile management
- CLI usage examples

**When to use**: Using or extending the `attune` CLI tool

#### `docs/configuration/`
**Purpose**: Configuration system documentation

**Contents**:
- Configuration file formats (YAML)
- Environment variable overrides
- Configuration troubleshooting
- Migration guides (e.g., env to YAML)

**When to use**: Configuring services, troubleshooting config issues

#### `docs/dependencies/`
**Purpose**: Dependency management and refactoring documentation

**Contents**:
- Dependency upgrade guides
- Deduplication efforts
- HTTP client consolidation
- Crate migration documentation (e.g., sea-query removal, serde-yaml migration)
- Workspace dependency compliance

**When to use**: Managing Rust dependencies, understanding dependency decisions

#### `docs/deployment/`
**Purpose**: Production deployment and operations documentation

**Contents**:
- Production deployment guides
- Operations runbooks
- Infrastructure setup

**When to use**: Deploying to production, handling operational issues

#### `docs/development/`
**Purpose**: Developer workflow and tooling documentation

**Contents**:
- Workspace setup guides
- Compilation notes
- Code cleanup procedures
- Documentation organization (this file)
- AGENTS.md index generation

**When to use**: Setting up dev environment, understanding dev tooling

#### `docs/examples/`
**Purpose**: Example configurations and workflows

**Contents**:
- Workflow YAML examples
- Pack registry examples
- Rule parameter examples
- Demo scripts

**When to use**: Learning by example, testing features

#### `docs/guides/`
**Purpose**: Getting started guides and tutorials

**Contents**:
- Quick start guides
- Feature-specific quickstarts (timers, workflows, sensors)
- Step-by-step tutorials

**When to use**: First-time users, learning new features

#### `docs/migrations/`
**Purpose**: Database and schema migration documentation

**Contents**:
- Migration decision records
- Schema change documentation
- Data migration guides

**When to use**: Understanding database schema evolution

#### `docs/packs/`
**Purpose**: Pack system documentation

**Contents**:
- Pack structure and creation
- Pack testing framework
- Pack registry specification
- Core pack integration

**When to use**: Creating packs, understanding pack architecture

#### `docs/performance/`
**Purpose**: Performance optimization documentation

**Contents**:
- Performance analysis reports
- Optimization guides
- Benchmarking results
- Resource limits (e.g., log size limits)

**When to use**: Performance tuning, understanding bottlenecks

#### `docs/plans/`
**Purpose**: Future planning and design documents

**Contents**:
- Refactoring plans
- Feature proposals
- Technical debt tracking

**When to use**: Planning major changes, understanding project direction

#### `docs/sensors/`
**Purpose**: Sensor system documentation

**Contents**:
- Sensor interface and lifecycle
- Sensor authentication
- Runtime configuration
- Sensor service setup

**When to use**: Creating sensors, debugging sensor issues

#### `docs/testing/`
**Purpose**: Testing documentation and strategies

**Contents**:
- Test execution guides
- Testing strategies (e2e, integration, unit)
- Schema-per-test architecture
- Test troubleshooting

**When to use**: Writing tests, debugging test failures

#### `docs/web-ui/`
**Purpose**: Web UI documentation

**Contents**:
- Web UI architecture
- Component documentation
- Testing guides

**When to use**: Frontend development, UI feature work

#### `docs/webhooks/`
**Purpose**: Webhook system documentation

**Contents**:
- Webhook architecture
- Testing webhooks
- Manual testing procedures

**When to use**: Implementing webhook triggers, debugging webhook issues

#### `docs/workflows/`
**Purpose**: Workflow engine documentation

**Contents**:
- Workflow execution engine
- Orchestration patterns
- Workflow implementation plans
- Rule and trigger mapping
- Parameter handling
- Inquiry (human-in-the-loop) system

**When to use**: Building workflows, understanding execution flow

---

### `work-summary/` Directory

#### `work-summary/status/`
**Purpose**: Current project status and TODO tracking

**Contents**:
- Status documents (`*STATUS*.md`)
- TODO lists
- Progress tracking
- Accomplishments

**When to use**: Understanding current project state, tracking work items

#### `work-summary/phases/`
**Purpose**: Development phase completion summaries and planning

**Contents**:
- Phase completion documents (`phase-*.md`)
- Analysis documents
- Problem statements
- Planning documents
- StackStorm lessons learned

**When to use**: Understanding project history, learning from past phases

#### `work-summary/sessions/`
**Purpose**: Daily development session notes

**Contents**:
- Dated session summaries (`YYYY-MM-DD-*.md`)
- Session-specific work logs
- Daily progress notes

**When to use**: Reviewing recent work, understanding context of changes

**Note**: This is the largest directory (155+ files) - use grep or find to locate specific sessions

#### `work-summary/features/`
**Purpose**: Feature implementation summaries

**Contents**:
- Feature-specific implementation notes
- Testing documentation
- Feature completion reports

**When to use**: Understanding how features were implemented

#### `work-summary/migrations/`
**Purpose**: Migration and refactoring work summaries

**Contents**:
- Migration completion summaries
- Refactoring session notes
- Migration status and next steps

**When to use**: Understanding migration history, planning migrations

#### `work-summary/changelogs/`
**Purpose**: Changelogs and major completion summaries

**Contents**:
- CHANGELOG.md
- API completion summaries
- Feature completion reports
- Cleanup summaries

**When to use**: Understanding what changed and when

---

## Finding Documentation

### Quick Reference

| What you need | Where to look |
|---------------|---------------|
| API endpoint details | `docs/api/` |
| How to deploy | `docs/deployment/` |
| Getting started | `docs/guides/` |
| Service architecture | `docs/architecture/` |
| How to test | `docs/testing/` |
| Authentication/security | `docs/authentication/` |
| Configuration | `docs/configuration/` |
| Pack creation | `docs/packs/` |
| Workflow building | `docs/workflows/` |
| Recent work | `work-summary/sessions/` |
| Current status | `work-summary/status/` |

### Search Tips

**Use grep for content search**:
```bash
# Find all docs mentioning "sensor"
grep -r "sensor" docs/

# Find API docs about executions
grep -r "execution" docs/api/

# Find recent work on workflows
grep -r "workflow" work-summary/sessions/
```

**Use find for path-based search**:
```bash
# Find all testing documentation
find docs/testing/ -name "*.md"

# Find all phase summaries
find work-summary/phases/ -name "phase-*.md"
```

**Use the AGENTS.md index**:
```bash
# Regenerate the index after adding new docs
make generate-agents-index

# View the minified index
cat AGENTS.md
```

---

## Maintenance Guidelines

### Adding New Documentation

1. **Determine the category**: Match your doc to one of the existing categories above
2. **Use descriptive names**: `feature-name-guide.md` or `component-architecture.md`
3. **Update AGENTS.md**: Run `make generate-agents-index` after adding docs
4. **Cross-reference**: Link to related docs in other categories

### Moving Documentation

When reorganizing docs:
1. Use `git mv` to preserve history
2. Update any hardcoded paths in other docs
3. Check for broken links
4. Regenerate AGENTS.md

### Work Summary Guidelines

- **Daily work**: Save to `work-summary/sessions/` with date prefix `YYYY-MM-DD-description.md`
- **Phase completions**: Save to `work-summary/phases/`
- **Status updates**: Update files in `work-summary/status/`
- **Feature summaries**: Save to `work-summary/features/` (for thematic, non-dated summaries)

---

## Reorganization History

**Date**: 2026-01-30

**Changes**:
- Created 16 subdirectories in `docs/`
- Created 7 subdirectories in `work-summary/`
- Organized 102 documentation files
- Organized 213 work summary files
- Updated AGENTS.md to reflect new structure

**Rationale**:
- Improved discoverability: Easier to find related documentation
- Logical grouping: Similar topics grouped together
- Reduced clutter: Root directories now clean and organized
- Better navigation: AI agents and developers can quickly locate relevant docs

**Benefits**:
- Faster documentation lookup
- Clearer project organization
- Better AI agent performance with minified index
- Easier onboarding for new developers