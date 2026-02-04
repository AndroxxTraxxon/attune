# Pack Management Architecture

**Last Updated**: 2026-01-19  
**Status**: Architectural Guidelines

---

## Overview

Attune uses a **pack-based architecture** where most automation components (actions, sensors, triggers) are defined as code and bundled into packs. This document clarifies which entities are code-based vs. UI-configurable and explains the rationale behind this design.

---

## Core Concepts

### Pack-Based Components (Code-Defined)

Components that are **implemented as code** and registered when a pack is loaded/installed:

1. **Actions** - Executable tasks with entry points (Python/Node.js/Shell scripts)
2. **Sensors** - Event monitoring code with poll intervals and trigger generation
3. **Triggers** (Pack-Based) - Event type definitions associated with sensors

**Key Characteristics**:
- Defined in pack manifest/metadata files
- Implemented as executable code (Python, Node.js, Shell, etc.)
- Registered during pack installation/loading
- **Not editable** through the Web UI
- Managed through pack lifecycle (install, update, uninstall)

### UI-Configurable Components (Data-Defined)

Components that are **configured through the UI** and stored as data:

1. **Rules** - Connect triggers to actions with criteria and parameters
2. **Packs (Ad-Hoc)** - User-created packs for custom automation
3. **Triggers (Ad-Hoc)** - Custom event type definitions for ad-hoc packs
4. **Workflows** - Multi-step automation sequences (future)

**Key Characteristics**:
- Defined through Web UI forms or API calls
- Stored as data in PostgreSQL
- Editable at runtime
- No code deployment required

---

## Pack Types

### 1. System Packs

**Definition**: Pre-built, standard packs that ship with Attune or are installed from a registry.

**Characteristics**:
- `system: true` flag in database
- Contain code-based actions, sensors, and triggers
- Installed via pack management tools
- **Not editable** through Web UI (code-based)
- Examples: `core`, `slack`, `aws`, `github`

**Components**:
- ✅ Actions (code-based)
- ✅ Sensors (code-based)
- ✅ Triggers (pack-defined)
- ❌ Rules (configured separately)

### 2. Ad-Hoc Packs

**Definition**: User-created packs for custom automation without deploying code.

**Characteristics**:
- `system: false` flag in database
- Registered through Web UI (`/packs/new`)
- May contain only triggers (no actions/sensors)
- Configuration schema for pack-level settings
- Examples: Custom webhook handlers, third-party integrations

**Components**:
- ✅ Triggers (UI-configurable)
- ❌ Actions (requires code, use system pack actions)
- ❌ Sensors (requires code, use system pack sensors)
- ❌ Rules (configured separately)

---

## Entity Management Matrix

| Entity Type | System Packs | Ad-Hoc Packs | Standalone | UI Editable |
|-------------|--------------|--------------|------------|-------------|
| Pack        | Code Install | ✅ UI Form   | N/A        | ✅ Ad-Hoc Only |
| Action      | Code Install | ❌ Not Allowed | ❌ No     | ❌ No       |
| Sensor      | Code Install | ❌ Not Allowed | ❌ No     | ❌ No       |
| Trigger     | Pack Manifest | ✅ UI Form   | ❌ No     | ✅ Ad-Hoc Only |
| Rule        | N/A          | N/A          | ✅ Yes    | ✅ Yes      |
| Workflow    | N/A          | N/A          | ✅ Yes    | ✅ Future   |

---

## Rationale

### Why Are Actions/Sensors Code-Based?

**Security**:
- Actions execute arbitrary code; UI-based code editing would be a security risk
- Sensors run continuously; code quality and safety is critical

**Complexity**:
- Actions may have complex dependencies (Python packages, Node modules)
- Sensors require event loop integration and error handling
- Runtime selection (Python vs Node.js vs Shell) requires proper sandboxing

**Testing and Quality**:
- Code-based components can be version-controlled
- Automated testing in CI/CD pipelines
- Code review processes before deployment

**Performance**:
- Compiled/optimized code runs faster
- Dependency management is cleaner (requirements.txt, package.json)

### Why Are Triggers Mixed?

**Pack-Based Triggers**:
- Tightly coupled to sensors that generate them
- Schema definitions for event payloads
- Example: `slack.message_received` trigger from `slack` pack

**Ad-Hoc Triggers**:
- Allow custom event types for external systems
- Webhook handlers that generate custom events
- Integration with third-party services without writing code
- Example: `custom.payment_received` for Stripe webhooks

### Why Are Rules Always UI-Configurable?

**Purpose**:
- Rules are **glue logic** connecting triggers to actions
- Users need to configure conditions and parameters dynamically
- No executable code required (just data mapping)

**Flexibility**:
- Business logic changes frequently
- Non-developers should be able to create rules
- Testing and iteration is easier with UI configuration

---

## Web UI Form Requirements

Based on this architecture, the Web UI should provide:

### ✅ Required Forms

1. **Rule Form** (`/rules/new`, `/rules/:id/edit`)
   - Select trigger (from any pack)
   - Define match criteria (JSON conditions)
   - Select action (from any pack)
   - Configure action parameters

2. **Pack Registration Form** (`/packs/new`, `/packs/:name/edit`)
   - Register ad-hoc pack
   - Define configuration schema (JSON Schema)
   - Set pack metadata

3. **Trigger Form** (`/triggers/new`, `/triggers/:id/edit`) - **Future**
   - Only for ad-hoc packs (`system: false`)
   - Define parameters schema
   - Define payload schema
   - Associate with ad-hoc pack

4. **Workflow Form** (`/workflows/new`, `/workflows/:ref/edit`) - **Future**
   - Visual workflow editor (React Flow)
   - Configure workflow actions (special type of action)
   - Define task dependencies and transitions

### ❌ NOT Required Forms

1. **Action Form** - Actions are code-based, registered via pack installation
2. **Sensor Form** - Sensors are code-based, registered via pack installation

---

## Pack Installation Process

### System Pack Installation (Future)

```bash
# Install from registry
attune pack install slack

# Install from local directory
attune pack install ./my-custom-pack

# Install from Git repository
attune pack install git+https://github.com/org/attune-pack-aws.git
```

**What Gets Registered**:
1. Pack metadata (name, version, description)
2. Actions (code files, entry points, parameter schemas)
3. Sensors (code files, poll intervals, trigger types)
4. Triggers (event type definitions, payload schemas)

### Ad-Hoc Pack Registration (Current)

```
Web UI: /packs/new
- Enter pack name
- Define config schema
- Save (no code required)
```

**What Gets Registered**:
1. Pack metadata (name, version, description)
2. Configuration schema (for pack-level settings)

**Then Add Triggers**:
```
Web UI: /triggers/new (Future)
- Select ad-hoc pack
- Define trigger name and schemas
- Save
```

---

## Example Workflows

### Scenario 1: Using System Pack

**Goal**: Send Slack notification when error event occurs

**Steps**:
1. Install `core` pack (provides `core.error_event` trigger)
2. Install `slack` pack (provides `slack.send_message` action)
3. Create rule via UI:
   - Trigger: `core.error_event`
   - Criteria: `{ "var": "payload.severity", ">=": 3 }`
   - Action: `slack.send_message`
   - Parameters: `{ "channel": "#alerts", "message": "..." }`

**No code required** - both packs are pre-built.

### Scenario 2: Custom Webhook Integration

**Goal**: Trigger automation from Stripe webhook

**Steps**:
1. Register ad-hoc pack via UI (`/packs/new`):
   - Name: `stripe-integration`
   - Config schema: `{ "webhook_secret": { "type": "string" } }`
2. Create ad-hoc trigger via UI (`/triggers/new`):
   - Pack: `stripe-integration`
   - Name: `payment.succeeded`
   - Payload schema: `{ "amount": "number", "customer": "string" }`
3. Configure webhook sensor (system pack provides generic webhook sensor)
4. Create rule via UI:
   - Trigger: `stripe-integration.payment.succeeded`
   - Action: `slack.send_message` (from system pack)

**Minimal code** - leverage existing webhook sensor, only define trigger schema.

### Scenario 3: Custom Action (Requires Code)

**Goal**: Custom Python action for proprietary API

**Steps**:
1. Create pack directory structure:
   ```
   my-company-pack/
   ├── pack.yaml
   ├── actions/
   │   └── send_alert.py
   └── requirements.txt
   ```
2. Install pack: `attune pack install ./my-company-pack`
3. Create rule via UI using `my-company.send_alert` action

**Code required** - custom business logic needs implementation.

---

## Future Enhancements

### Pack Registry (Phase 1)

- Central repository of Attune packs
- Version management and updates
- Pack discovery and browsing
- Dependency resolution

### Visual Workflow Editor (Phase 2)

- Drag-and-drop workflow designer
- Workflow actions (special configurable actions)
- Conditional logic and branching
- Sub-workflows and reusable components

### Pack Marketplace (Phase 3)

- Community-contributed packs
- Rating and reviews
- Documentation and examples
- Automated testing and validation

---

## Summary

**Key Principles**:

1. **Code for execution** - Actions and sensors are implemented as code for security, performance, and maintainability
2. **Data for configuration** - Rules and workflows are UI-configurable for flexibility
3. **Hybrid for triggers** - Pack-based for sensors, ad-hoc for custom integrations
4. **Pack-centric design** - Components are bundled and versioned together
5. **Progressive enhancement** - Start with system packs, extend with ad-hoc components

This architecture balances **flexibility** (users can configure automation without code) with **safety** (executable code is version-controlled and reviewed).

---

## Related Documentation

- [Pack Management API](./api-packs.md)
- [Rule Management API](./api-rules.md)
- [Trigger and Sensor Architecture](./trigger-sensor-architecture.md)
- [Web UI Architecture](./web-ui-architecture.md)