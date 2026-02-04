# Rule Management API

This document describes the Rule Management API endpoints for the Attune automation platform.

## Overview

Rules are the core automation logic in Attune that connect triggers to actions. When a trigger fires an event that matches a rule's conditions, the associated action is executed. Rules enable powerful event-driven automation workflows.

**Base Path:** `/api/v1/rules`

## Data Model

### Rule

```json
{
  "id": 1,
  "ref": "mypack.notify_on_error",
  "pack": 1,
  "pack_ref": "mypack",
  "label": "Notify on Error",
  "description": "Send notification when error event is detected",
  "action": 5,
  "action_ref": "slack.send_message",
  "trigger": 3,
  "trigger_ref": "core.error_event",
  "conditions": {
    "and": [
      {"var": "event.severity", ">=": 3},
      {"var": "event.status", "==": "error"}
    ]
  },
  "action_params": {
    "channel": "#alerts",
    "message": "Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}",
    "severity": "{{ trigger.payload.severity }}"
  },
  "enabled": true,
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

**Field Descriptions:**

- `id` (integer) - Unique identifier
- `ref` (string) - Unique reference in format `pack.name`
- `pack` (integer) - Pack ID this rule belongs to
- `pack_ref` (string) - Pack reference
- `label` (string) - Human-readable name
- `description` (string) - Rule description
- `action` (integer) - Action ID to execute
- `action_ref` (string) - Action reference
- `trigger` (integer) - Trigger ID that activates this rule
- `trigger_ref` (string) - Trigger reference
- `conditions` (object) - JSON Logic conditions for rule evaluation
- `action_params` (object) - Parameters to pass to the action (supports dynamic templates)
- `enabled` (boolean) - Whether the rule is active
- `created` (timestamp) - Creation time
- `updated` (timestamp) - Last update time

**Action Parameters:**

The `action_params` field supports both static values and dynamic templates:

- **Static values**: `"channel": "#alerts"`
- **Dynamic from trigger payload**: `"message": "{{ trigger.payload.message }}"`
- **Dynamic from pack config**: `"token": "{{ pack.config.api_token }}"`
- **System variables**: `"timestamp": "{{ system.timestamp }}"`

See [Rule Parameter Mapping](./rule-parameter-mapping.md) for complete documentation.

### Rule Summary (List View)

```json
{
  "id": 1,
  "ref": "mypack.notify_on_error",
  "pack_ref": "mypack",
  "label": "Notify on Error",
  "description": "Send notification when error event is detected",
  "action_ref": "slack.send_message",
  "trigger_ref": "core.error_event",
  "enabled": true,
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

## Endpoints

### List All Rules

Retrieve a paginated list of all rules.

**Endpoint:** `GET /api/v1/rules`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "ref": "mypack.notify_on_error",
      "pack_ref": "mypack",
      "label": "Notify on Error",
      "description": "Send notification when error event is detected",
      "action_ref": "slack.send_message",
      "trigger_ref": "core.error_event",
      "enabled": true,
      "created": "2024-01-13T10:00:00Z",
      "updated": "2024-01-13T10:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

---

### List Enabled Rules

Retrieve only rules that are currently enabled.

**Endpoint:** `GET /api/v1/rules/enabled`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "ref": "mypack.notify_on_error",
      "pack_ref": "mypack",
      "label": "Notify on Error",
      "description": "Send notification when error event is detected",
      "action_ref": "slack.send_message",
      "trigger_ref": "core.error_event",
      "enabled": true,
      "created": "2024-01-13T10:00:00Z",
      "updated": "2024-01-13T10:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

---

### List Rules by Pack

Retrieve all rules belonging to a specific pack.

**Endpoint:** `GET /api/v1/packs/:pack_ref/rules`

**Path Parameters:**
- `pack_ref` (string): Pack reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Pack with the specified ref does not exist

---

### List Rules by Action

Retrieve all rules that execute a specific action.

**Endpoint:** `GET /api/v1/actions/:action_ref/rules`

**Path Parameters:**
- `action_ref` (string): Action reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Action with the specified ref does not exist

---

### List Rules by Trigger

Retrieve all rules that are activated by a specific trigger.

**Endpoint:** `GET /api/v1/triggers/:trigger_ref/rules`

**Path Parameters:**
- `trigger_ref` (string): Trigger reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---

### Get Rule by Reference

Retrieve a single rule by its reference identifier.

**Endpoint:** `GET /api/v1/rules/:ref`

**Path Parameters:**
- `ref` (string): Rule reference identifier (e.g., "mypack.notify_on_error")

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.notify_on_error",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Notify on Error",
    "description": "Send notification when error event is detected",
    "action": 5,
    "action_ref": "slack.send_message",
    "trigger": 3,
    "trigger_ref": "core.error_event",
    "conditions": {
      "and": [
        {"var": "event.severity", ">=": 3},
        {"var": "event.status", "==": "error"}
      ]
    },
    "enabled": true,
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  }
}
```

**Errors:**
- `404 Not Found`: Rule with the specified ref does not exist

---



### Create Rule

Create a new rule in the system.

**Endpoint:** `POST /api/v1/rules`

**Request Body:**

```json
{
  "ref": "mypack.notify_on_error",
  "pack_ref": "mypack",
  "label": "Notify on Error",
  "description": "Send notification when error event is detected",
  "action_ref": "slack.send_message",
  "trigger_ref": "core.error_event",
  "conditions": {
    "and": [
      {"var": "event.severity", ">=": 3},
      {"var": "event.status", "==": "error"}
    ]
  },
  "action_params": {
    "channel": "#alerts",
    "message": "Error detected: {{ trigger.payload.message }}",
    "severity": "{{ trigger.payload.severity }}"
  },
  "enabled": true
}
```

**Required Fields:**
- `ref`: Unique reference identifier (alphanumeric, dots, underscores, hyphens)
- `pack_ref`: Reference to the parent pack (must exist)
- `label`: Human-readable name (1-255 characters)
- `description`: Rule description (min 1 character)
- `action_ref`: Reference to action to execute (must exist)
- `trigger_ref`: Reference to trigger that activates rule (must exist)

**Optional Fields:**
- `conditions`: JSON Logic conditions for rule evaluation (default: `{}`)
- `action_params`: Parameters to pass to the action (default: `{}`)
  - Supports static values: `"channel": "#alerts"`
  - Supports dynamic templates: `"message": "{{ trigger.payload.message }}"`
  - Supports pack config: `"token": "{{ pack.config.api_token }}"`
- `enabled`: Whether the rule is active (default: `true`)

**Response:** `201 Created`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.notify_on_error",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Notify on Error",
    "description": "Send notification when error event is detected",
    "action": 5,
    "action_ref": "slack.send_message",
    "trigger": 3,
    "trigger_ref": "core.error_event",
    "conditions": {
      "and": [
        {"var": "event.severity", ">=": 3},
        {"var": "event.status", "==": "error"}
      ]
    },
    "action_params": {
      "channel": "#alerts",
      "message": "Error detected: {{ trigger.payload.message }}",
      "severity": "{{ trigger.payload.severity }}"
    },
    "enabled": true,
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  },
  "message": "Rule created successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Referenced pack, action, or trigger does not exist
- `409 Conflict`: Rule with the same ref already exists

---

### Update Rule

Update an existing rule's properties.

**Endpoint:** `PUT /api/v1/rules/:ref`

**Path Parameters:**
- `ref` (string): Rule reference identifier

**Request Body:**

All fields are optional. Only provided fields will be updated.

```json
{
  "label": "Notify on Critical Errors",
  "description": "Enhanced error notification with filtering",
  "conditions": {
    "and": [
      {"var": "event.severity", ">=": 4},
      {"var": "event.status", "==": "error"}
    ]
  },
  "action_params": {
    "channel": "#critical-alerts",
    "message": "CRITICAL: {{ trigger.payload.service }} - {{ trigger.payload.message }}",
    "priority": "high"
  },
  "enabled": false
}
```

**Note:** You cannot change `pack_ref`, `action_ref`, or `trigger_ref` via update. Create a new rule instead.

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.notify_on_error",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Updated Notify on Error",
    "description": "Updated description",
    "action": 5,
    "action_ref": "slack.send_message",
    "trigger": 3,
    "trigger_ref": "core.error_event",
    "conditions": {
      "and": [
        {"var": "event.severity", ">=": 4},
        {"var": "event.status", "==": "error"}
      ]
    },
    "action_params": {
      "channel": "#critical-alerts",
      "message": "CRITICAL: {{ trigger.payload.service }} - {{ trigger.payload.message }}",
      "priority": "high"
    },
    "enabled": false,
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T12:00:00Z"
  },
  "message": "Rule updated successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Rule with the specified ref does not exist

---

### Enable Rule

Enable a rule to activate it for event processing.

**Endpoint:** `POST /api/v1/rules/:ref/enable`

**Path Parameters:**
- `ref` (string): Rule reference identifier

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.notify_on_error",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Notify on Error",
    "description": "Send notification when error event is detected",
    "action": 5,
    "action_ref": "slack.send_message",
    "trigger": 3,
    "trigger_ref": "core.error_event",
    "conditions": { ... },
    "enabled": true,
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T12:00:00Z"
  },
  "message": "Rule enabled successfully"
}
```

**Errors:**
- `404 Not Found`: Rule with the specified ref does not exist

---

### Disable Rule

Disable a rule to prevent it from processing events.

**Endpoint:** `POST /api/v1/rules/:ref/disable`

**Path Parameters:**
- `ref` (string): Rule reference identifier

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.notify_on_error",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Notify on Error",
    "description": "Send notification when error event is detected",
    "action": 5,
    "action_ref": "slack.send_message",
    "trigger": 3,
    "trigger_ref": "core.error_event",
    "conditions": { ... },
    "enabled": false,
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T12:00:00Z"
  },
  "message": "Rule disabled successfully"
}
```

**Errors:**
- `404 Not Found`: Rule with the specified ref does not exist

---

### Delete Rule

Delete a rule from the system.

**Endpoint:** `DELETE /api/v1/rules/:ref`

**Path Parameters:**
- `ref` (string): Rule reference identifier

**Response:** `200 OK`

```json
{
  "success": true,
  "message": "Rule 'mypack.notify_on_error' deleted successfully"
}
```

**Errors:**
- `404 Not Found`: Rule with the specified ref does not exist

---

## Rule Conditions

Rules use conditions to determine whether an action should be executed when a trigger fires. Conditions are evaluated against the event payload.

### Condition Format

Attune supports JSON Logic format for conditions:

```json
{
  "and": [
    {"var": "event.severity", ">=": 3},
    {"var": "event.status", "==": "error"}
  ]
}
```

### Common Operators

- **Comparison:** `==`, `!=`, `<`, `<=`, `>`, `>=`
- **Logical:** `and`, `or`, `not`
- **Membership:** `in`, `contains`
- **Existence:** `var` (check if variable exists)

### Condition Examples

**Simple equality check:**
```json
{
  "var": "event.status",
  "==": "error"
}
```

**Multiple conditions (AND):**
```json
{
  "and": [
    {"var": "event.severity", ">=": 3},
    {"var": "event.type", "==": "alert"}
  ]
}
```

**Multiple conditions (OR):**
```json
{
  "or": [
    {"var": "event.status", "==": "error"},
    {"var": "event.status", "==": "critical"}
  ]
}
```

**Nested conditions:**
```json
{
  "and": [
    {"var": "event.severity", ">=": 3},
    {
      "or": [
        {"var": "event.type", "==": "alert"},
        {"var": "event.type", "==": "warning"}
      ]
    }
  ]
}
```

**Always match (empty conditions):**
```json
{}
```

---

## Examples

### Creating a Simple Rule

```bash
curl -X POST http://localhost:3000/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "mypack.alert_on_high_cpu",
    "pack_ref": "mypack",
    "label": "Alert on High CPU",
    "description": "Send alert when CPU usage exceeds 90%",
    "action_ref": "slack.send_message",
    "trigger_ref": "system.cpu_monitor",
    "conditions": {
      "var": "cpu_percent",
      ">": 90
    },
    "enabled": true
  }'
```

### Creating a Rule with Complex Conditions

```bash
curl -X POST http://localhost:3000/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "security.failed_login_alert",
    "pack_ref": "security",
    "label": "Failed Login Alert",
    "description": "Alert on multiple failed login attempts",
    "action_ref": "pagerduty.create_incident",
    "trigger_ref": "auth.login_failure",
    "conditions": {
      "and": [
        {"var": "failed_attempts", ">=": 5},
        {"var": "time_window_minutes", "<=": 10},
        {"var": "user.is_admin", "==": true}
      ]
    }
  }'
```

### Listing Enabled Rules

```bash
curl http://localhost:3000/api/v1/rules/enabled
```

### Listing Rules by Pack

```bash
curl http://localhost:3000/api/v1/packs/mypack/rules
```

### Updating Rule Conditions

```bash
curl -X PUT http://localhost:3000/api/v1/rules/mypack.alert_on_high_cpu \
  -H "Content-Type: application/json" \
  -d '{
    "conditions": {
      "var": "cpu_percent",
      ">": 95
    }
  }'
```

### Disabling a Rule

```bash
curl -X POST http://localhost:3000/api/v1/rules/mypack.alert_on_high_cpu/disable
```

### Enabling a Rule

```bash
curl -X POST http://localhost:3000/api/v1/rules/mypack.alert_on_high_cpu/enable
```

### Deleting a Rule

```bash
curl -X DELETE http://localhost:3000/api/v1/rules/mypack.alert_on_high_cpu
```

---

## Validation Rules

### Rule Reference (`ref`)
- Must be unique across all rules
- Can contain alphanumeric characters, dots (.), underscores (_), and hyphens (-)
- Typically follows the pattern: `pack_name.rule_name`
- Example: `mypack.notify_on_error`, `security.failed_login_alert`

### Pack Reference (`pack_ref`)
- Must reference an existing pack
- The pack must exist before creating rules for it

### Action Reference (`action_ref`)
- Must reference an existing action
- The action will be executed when rule conditions match

### Trigger Reference (`trigger_ref`)
- Must reference an existing trigger
- The trigger determines when the rule is evaluated

### Conditions
- Must be valid JSON
- Typically follows JSON Logic format
- Empty object `{}` means rule always matches
- Conditions are evaluated against event payload

---

## Rule Evaluation Flow

1. **Trigger Fires:** An event occurs that activates a trigger
2. **Find Rules:** System finds all enabled rules for that trigger
3. **Evaluate Conditions:** Each rule's conditions are evaluated against the event payload
4. **Execute Action:** If conditions match, the associated action is executed
5. **Record Enforcement:** Execution is logged as an enforcement record

```
Event → Trigger → Rule Evaluation → Condition Match? → Execute Action
                      ↓                     ↓
                   (conditions)          (yes/no)
```

---

## Best Practices

1. **Naming Conventions**
   - Use descriptive, hierarchical names: `pack.purpose`
   - Keep names concise but meaningful
   - Use lowercase with dots as separators

2. **Condition Design**
   - Start simple, add complexity as needed
   - Test conditions with sample event data
   - Document complex condition logic
   - Consider edge cases and null values

3. **Rule Organization**
   - Group related rules in the same pack
   - One rule per specific automation task
   - Avoid overly complex conditions (split into multiple rules)

4. **Performance**
   - Keep conditions efficient
   - Disable unused rules rather than deleting
   - Use specific conditions to reduce unnecessary action executions

5. **Testing**
   - Test rules in development environment first
   - Start with rules disabled, enable after testing
   - Monitor enforcement records to verify behavior
   - Use `/rules/:ref/disable` to quickly stop problematic rules

6. **Maintenance**
   - Document rule purpose and expected behavior
   - Review and update conditions regularly
   - Clean up obsolete rules
   - Version your condition logic in comments

---

## Common Patterns

### Alert on Threshold

```json
{
  "ref": "monitoring.disk_space_alert",
  "trigger_ref": "system.disk_check",
  "action_ref": "slack.notify",
  "conditions": {
    "var": "disk_usage_percent",
    ">": 85
  }
}
```

### Multi-Condition Filter

```json
{
  "ref": "security.suspicious_activity",
  "trigger_ref": "security.access_log",
  "action_ref": "security.investigate",
  "conditions": {
    "and": [
      {"var": "request.method", "==": "POST"},
      {"var": "response.status", "==": 401},
      {"var": "ip_reputation_score", "<": 50}
    ]
  }
}
```

### Time-Based Rule

```json
{
  "ref": "backup.daily_backup",
  "trigger_ref": "schedule.daily",
  "action_ref": "backup.full_backup",
  "conditions": {
    "var": "hour",
    "==": 2
  }
}
```

---

## Related Documentation

- [Pack Management API](./api-packs.md)
- [Action Management API](./api-actions.md)
- [Trigger Management API](./api-triggers.md)
- [Execution API](./api-executions.md)
- [Event System](./events.md)

---

**Last Updated:** January 13, 2026