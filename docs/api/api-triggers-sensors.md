# Trigger and Sensor Management API

This document describes the Trigger and Sensor Management API endpoints for the Attune automation platform.

## Overview

Triggers and Sensors form the event detection layer of the Attune automation platform:

- **Triggers** define event types that can activate rules (e.g., "webhook received", "timer expired", "file changed")
- **Sensors** are the active monitoring components that watch for events and fire triggers

Together, they complete the automation chain: **Sensor → Trigger → Rule → Action → Execution**

**Base Path:** `/api/v1/triggers` and `/api/v1/sensors`

## Triggers

### Trigger Data Model

```json
{
  "id": 1,
  "ref": "core.webhook",
  "pack": 1,
  "pack_ref": "core",
  "label": "Webhook Event",
  "description": "Triggered when a webhook is received",
  "enabled": true,
  "param_schema": {
    "type": "object",
    "properties": {
      "url_path": { "type": "string" },
      "method": { "type": "string", "enum": ["GET", "POST", "PUT", "DELETE"] }
    }
  },
  "out_schema": {
    "type": "object",
    "properties": {
      "headers": { "type": "object" },
      "body": { "type": "object" },
      "query": { "type": "object" }
    }
  },
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

### Trigger Summary (List View)

```json
{
  "id": 1,
  "ref": "core.webhook",
  "pack_ref": "core",
  "label": "Webhook Event",
  "description": "Triggered when a webhook is received",
  "enabled": true,
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

## Trigger Endpoints

### List All Triggers

Retrieve a paginated list of all triggers.

**Endpoint:** `GET /api/v1/triggers`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "ref": "core.webhook",
      "pack_ref": "core",
      "label": "Webhook Event",
      "description": "Triggered when a webhook is received",
      "enabled": true,
      "created": "2024-01-13T10:00:00Z",
      "updated": "2024-01-13T10:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "page_size": 50,
    "total_items": 1,
    "total_pages": 1
  }
}
```

---

### List Enabled Triggers

Retrieve only triggers that are currently enabled.

**Endpoint:** `GET /api/v1/triggers/enabled`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

---

### List Triggers by Pack

Retrieve all triggers belonging to a specific pack.

**Endpoint:** `GET /api/v1/packs/:pack_ref/triggers`

**Path Parameters:**
- `pack_ref` (string): Pack reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Pack with the specified ref does not exist

---

### Get Trigger by Reference

Retrieve a single trigger by its reference identifier.

**Endpoint:** `GET /api/v1/triggers/:ref`

**Path Parameters:**
- `ref` (string): Trigger reference identifier (e.g., "core.webhook")

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "core.webhook",
    "pack": 1,
    "pack_ref": "core",
    "label": "Webhook Event",
    "description": "Triggered when a webhook is received",
    "enabled": true,
    "param_schema": { ... },
    "out_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  }
}
```

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---



### Create Trigger

Create a new trigger in the system.

**Endpoint:** `POST /api/v1/triggers`

**Request Body:**

```json
{
  "ref": "mypack.custom_event",
  "pack_ref": "mypack",
  "label": "Custom Event",
  "description": "A custom event trigger for my pack",
  "param_schema": {
    "type": "object",
    "properties": {
      "event_type": { "type": "string" },
      "severity": { "type": "integer" }
    }
  },
  "out_schema": {
    "type": "object",
    "properties": {
      "timestamp": { "type": "string", "format": "date-time" },
      "data": { "type": "object" }
    }
  },
  "enabled": true
}
```

**Required Fields:**
- `ref`: Unique reference identifier (alphanumeric, dots, underscores, hyphens)
- `label`: Human-readable name (1-255 characters)

**Optional Fields:**
- `pack_ref`: Reference to the parent pack (if null, trigger is system-wide)
- `description`: Trigger description
- `param_schema`: JSON Schema defining trigger parameters
- `out_schema`: JSON Schema defining event data structure
- `enabled`: Whether the trigger is active (default: `true`)

**Response:** `201 Created`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.custom_event",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Custom Event",
    "description": "A custom event trigger for my pack",
    "enabled": true,
    "param_schema": { ... },
    "out_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  },
  "message": "Trigger created successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Referenced pack does not exist
- `409 Conflict`: Trigger with the same ref already exists

---

### Update Trigger

Update an existing trigger's properties.

**Endpoint:** `PUT /api/v1/triggers/:ref`

**Path Parameters:**
- `ref` (string): Trigger reference identifier

**Request Body:**

All fields are optional. Only provided fields will be updated.

```json
{
  "label": "Updated Custom Event",
  "description": "Updated description",
  "enabled": false,
  "param_schema": { ... },
  "out_schema": { ... }
}
```

**Note:** You cannot change `ref` or `pack_ref` via update.

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "mypack.custom_event",
    "pack": 1,
    "pack_ref": "mypack",
    "label": "Updated Custom Event",
    "description": "Updated description",
    "enabled": false,
    "param_schema": { ... },
    "out_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T12:00:00Z"
  },
  "message": "Trigger updated successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Trigger with the specified ref does not exist

---

### Enable Trigger

Enable a trigger to activate it for event processing.

**Endpoint:** `POST /api/v1/triggers/:ref/enable`

**Path Parameters:**
- `ref` (string): Trigger reference identifier

**Response:** `200 OK`

```json
{
  "data": { ... },
  "message": "Trigger enabled successfully"
}
```

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---

### Disable Trigger

Disable a trigger to prevent it from processing events.

**Endpoint:** `POST /api/v1/triggers/:ref/disable`

**Path Parameters:**
- `ref` (string): Trigger reference identifier

**Response:** `200 OK`

```json
{
  "data": { ... },
  "message": "Trigger disabled successfully"
}
```

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---

### Delete Trigger

Delete a trigger from the system.

**Endpoint:** `DELETE /api/v1/triggers/:ref`

**Path Parameters:**
- `ref` (string): Trigger reference identifier

**Response:** `200 OK`

```json
{
  "success": true,
  "message": "Trigger 'mypack.custom_event' deleted successfully"
}
```

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---

## Sensors

### Sensor Data Model

```json
{
  "id": 1,
  "ref": "monitoring.cpu_sensor",
  "pack": 1,
  "pack_ref": "monitoring",
  "label": "CPU Usage Monitor",
  "description": "Monitors CPU usage and fires trigger when threshold exceeded",
  "entrypoint": "/sensors/cpu_monitor.py",
  "runtime": 2,
  "runtime_ref": "python3",
  "trigger": 5,
  "trigger_ref": "system.cpu_alert",
  "enabled": true,
  "param_schema": {
    "type": "object",
    "properties": {
      "threshold": { "type": "number", "default": 80 },
      "interval_seconds": { "type": "integer", "default": 60 }
    }
  },
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

### Sensor Summary (List View)

```json
{
  "id": 1,
  "ref": "monitoring.cpu_sensor",
  "pack_ref": "monitoring",
  "label": "CPU Usage Monitor",
  "description": "Monitors CPU usage and fires trigger when threshold exceeded",
  "trigger_ref": "system.cpu_alert",
  "enabled": true,
  "created": "2024-01-13T10:00:00Z",
  "updated": "2024-01-13T10:00:00Z"
}
```

## Sensor Endpoints

### List All Sensors

Retrieve a paginated list of all sensors.

**Endpoint:** `GET /api/v1/sensors`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

---

### List Enabled Sensors

Retrieve only sensors that are currently enabled.

**Endpoint:** `GET /api/v1/sensors/enabled`

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

---

### List Sensors by Pack

Retrieve all sensors belonging to a specific pack.

**Endpoint:** `GET /api/v1/packs/:pack_ref/sensors`

**Path Parameters:**
- `pack_ref` (string): Pack reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Pack with the specified ref does not exist

---

### List Sensors by Trigger

Retrieve all sensors that monitor for a specific trigger.

**Endpoint:** `GET /api/v1/triggers/:trigger_ref/sensors`

**Path Parameters:**
- `trigger_ref` (string): Trigger reference identifier

**Query Parameters:**
- `page` (integer, optional): Page number (default: 1)
- `page_size` (integer, optional): Items per page (default: 50, max: 100)

**Response:** `200 OK`

**Errors:**
- `404 Not Found`: Trigger with the specified ref does not exist

---

### Get Sensor by Reference

Retrieve a single sensor by its reference identifier.

**Endpoint:** `GET /api/v1/sensors/:ref`

**Path Parameters:**
- `ref` (string): Sensor reference identifier (e.g., "monitoring.cpu_sensor")

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "monitoring.cpu_sensor",
    "pack": 1,
    "pack_ref": "monitoring",
    "label": "CPU Usage Monitor",
    "description": "Monitors CPU usage and fires trigger when threshold exceeded",
    "entrypoint": "/sensors/cpu_monitor.py",
    "runtime": 2,
    "runtime_ref": "python3",
    "trigger": 5,
    "trigger_ref": "system.cpu_alert",
    "enabled": true,
    "param_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  }
}
```

**Errors:**
- `404 Not Found`: Sensor with the specified ref does not exist

---



### Create Sensor

Create a new sensor in the system.

**Endpoint:** `POST /api/v1/sensors`

**Request Body:**

```json
{
  "ref": "monitoring.cpu_sensor",
  "pack_ref": "monitoring",
  "label": "CPU Usage Monitor",
  "description": "Monitors CPU usage and fires trigger when threshold exceeded",
  "entrypoint": "/sensors/cpu_monitor.py",
  "runtime_ref": "python3",
  "trigger_ref": "system.cpu_alert",
  "param_schema": {
    "type": "object",
    "properties": {
      "threshold": { "type": "number", "default": 80 },
      "interval_seconds": { "type": "integer", "default": 60 }
    }
  },
  "enabled": true
}
```

**Required Fields:**
- `ref`: Unique reference identifier (alphanumeric, dots, underscores, hyphens)
- `pack_ref`: Reference to the parent pack (must exist)
- `label`: Human-readable name (1-255 characters)
- `description`: Sensor description (min 1 character)
- `entrypoint`: Path or identifier for the sensor code (1-1024 characters)
- `runtime_ref`: Reference to runtime environment (must exist)
- `trigger_ref`: Reference to trigger this sensor fires (must exist)

**Optional Fields:**
- `param_schema`: JSON Schema defining sensor configuration parameters
- `enabled`: Whether the sensor is active (default: `true`)

**Response:** `201 Created`

```json
{
  "data": {
    "id": 1,
    "ref": "monitoring.cpu_sensor",
    "pack": 1,
    "pack_ref": "monitoring",
    "label": "CPU Usage Monitor",
    "description": "Monitors CPU usage and fires trigger when threshold exceeded",
    "entrypoint": "/sensors/cpu_monitor.py",
    "runtime": 2,
    "runtime_ref": "python3",
    "trigger": 5,
    "trigger_ref": "system.cpu_alert",
    "enabled": true,
    "param_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T10:00:00Z"
  },
  "message": "Sensor created successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Referenced pack, runtime, or trigger does not exist
- `409 Conflict`: Sensor with the same ref already exists

---

### Update Sensor

Update an existing sensor's properties.

**Endpoint:** `PUT /api/v1/sensors/:ref`

**Path Parameters:**
- `ref` (string): Sensor reference identifier

**Request Body:**

All fields are optional. Only provided fields will be updated.

```json
{
  "label": "Updated CPU Monitor",
  "description": "Updated description",
  "entrypoint": "/sensors/cpu_monitor_v2.py",
  "enabled": false,
  "param_schema": { ... }
}
```

**Note:** You cannot change `ref`, `pack_ref`, `runtime_ref`, or `trigger_ref` via update.

**Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "ref": "monitoring.cpu_sensor",
    "pack": 1,
    "pack_ref": "monitoring",
    "label": "Updated CPU Monitor",
    "description": "Updated description",
    "entrypoint": "/sensors/cpu_monitor_v2.py",
    "runtime": 2,
    "runtime_ref": "python3",
    "trigger": 5,
    "trigger_ref": "system.cpu_alert",
    "enabled": false,
    "param_schema": { ... },
    "created": "2024-01-13T10:00:00Z",
    "updated": "2024-01-13T12:00:00Z"
  },
  "message": "Sensor updated successfully"
}
```

**Errors:**
- `400 Bad Request`: Invalid request data or validation failure
- `404 Not Found`: Sensor with the specified ref does not exist

---

### Enable Sensor

Enable a sensor to activate it for monitoring.

**Endpoint:** `POST /api/v1/sensors/:ref/enable`

**Path Parameters:**
- `ref` (string): Sensor reference identifier

**Response:** `200 OK`

```json
{
  "data": { ... },
  "message": "Sensor enabled successfully"
}
```

**Errors:**
- `404 Not Found`: Sensor with the specified ref does not exist

---

### Disable Sensor

Disable a sensor to stop it from monitoring.

**Endpoint:** `POST /api/v1/sensors/:ref/disable`

**Path Parameters:**
- `ref` (string): Sensor reference identifier

**Response:** `200 OK`

```json
{
  "data": { ... },
  "message": "Sensor disabled successfully"
}
```

**Errors:**
- `404 Not Found`: Sensor with the specified ref does not exist

---

### Delete Sensor

Delete a sensor from the system.

**Endpoint:** `DELETE /api/v1/sensors/:ref`

**Path Parameters:**
- `ref` (string): Sensor reference identifier

**Response:** `200 OK`

```json
{
  "success": true,
  "message": "Sensor 'monitoring.cpu_sensor' deleted successfully"
}
```

**Errors:**
- `404 Not Found`: Sensor with the specified ref does not exist

---

## Event Flow

Understanding how triggers and sensors work together:

```
1. Sensor runs continuously/periodically
2. Sensor detects event condition
3. Sensor fires Trigger with event data
4. Trigger activates all associated Rules
5. Rules evaluate conditions against event data
6. Matching Rules execute their Actions
7. Actions create Executions
```

### Example Flow

```
CPU Sensor (monitoring.cpu_sensor)
    ↓
Detects CPU > 90%
    ↓
Fires Trigger (system.cpu_alert)
    ↓
Activates Rules:
  - Rule: "Alert on high CPU" → Action: Send Slack message
  - Rule: "Scale up on load" → Action: Add server instance
    ↓
Creates Executions for each Action
```

---

## Trigger Types

### Built-in Trigger Types

Common trigger types included in the core pack:

- **`core.webhook`** - HTTP webhook received
- **`core.timer`** - Timer/schedule-based trigger
- **`core.event`** - Generic event trigger
- **`core.manual`** - Manual trigger via API

### Custom Triggers

Create custom triggers for your use cases:

```bash
curl -X POST http://localhost:3000/api/v1/triggers \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "myapp.user_signup",
    "pack_ref": "myapp",
    "label": "User Signup",
    "description": "Triggered when a new user signs up",
    "param_schema": {
      "type": "object",
      "properties": {
        "user_id": { "type": "string" },
        "email": { "type": "string", "format": "email" },
        "plan": { "type": "string" }
      },
      "required": ["user_id", "email"]
    }
  }'
```

---

## Sensor Patterns

### Polling Sensor

Periodically checks a condition:

```json
{
  "ref": "monitoring.disk_space",
  "label": "Disk Space Monitor",
  "description": "Checks disk space every 5 minutes",
  "entrypoint": "/sensors/disk_monitor.py",
  "runtime_ref": "python3",
  "trigger_ref": "system.disk_alert",
  "param_schema": {
    "type": "object",
    "properties": {
      "interval_seconds": { "type": "integer", "default": 300 },
      "threshold_percent": { "type": "number", "default": 90 }
    }
  }
}
```

### Webhook Sensor

Listens for incoming HTTP requests:

```json
{
  "ref": "integration.github_webhook",
  "label": "GitHub Webhook",
  "description": "Receives GitHub webhook events",
  "entrypoint": "/sensors/github_webhook.py",
  "runtime_ref": "python3",
  "trigger_ref": "github.push_event",
  "param_schema": {
    "type": "object",
    "properties": {
      "secret": { "type": "string" },
      "events": { "type": "array", "items": { "type": "string" } }
    }
  }
}
```

### Stream Sensor

Monitors a continuous data stream:

```json
{
  "ref": "logs.error_detector",
  "label": "Error Log Detector",
  "description": "Monitors log stream for errors",
  "entrypoint": "/sensors/log_monitor.py",
  "runtime_ref": "python3",
  "trigger_ref": "logs.error_found",
  "param_schema": {
    "type": "object",
    "properties": {
      "log_path": { "type": "string" },
      "error_pattern": { "type": "string" }
    }
  }
}
```

---

## Examples

### Create System Timer Trigger

```bash
curl -X POST http://localhost:3000/api/v1/triggers \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "core.timer",
    "pack_ref": "core",
    "label": "Timer",
    "description": "Schedule-based trigger",
    "param_schema": {
      "type": "object",
      "properties": {
        "cron": { "type": "string" }
      }
    }
  }'
```

### Create Monitoring Sensor

```bash
curl -X POST http://localhost:3000/api/v1/sensors \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "monitoring.memory_sensor",
    "pack_ref": "monitoring",
    "label": "Memory Monitor",
    "description": "Monitors memory usage",
    "entrypoint": "/sensors/memory_monitor.py",
    "runtime_ref": "python3",
    "trigger_ref": "system.memory_alert",
    "param_schema": {
      "type": "object",
      "properties": {
        "threshold_percent": { "type": "number", "default": 85 },
        "interval_seconds": { "type": "integer", "default": 120 }
      }
    }
  }'
```

### List Sensors for a Trigger

```bash
curl http://localhost:3000/api/v1/triggers/system.cpu_alert/sensors
```

### Disable a Sensor

```bash
curl -X POST http://localhost:3000/api/v1/sensors/monitoring.cpu_sensor/disable
```

---

## Best Practices

### Triggers

1. **Naming Convention**
   - Use hierarchical names: `pack.category.trigger_name`
   - Keep names descriptive: `github.push_event` not `gh.pe`
   - Use lowercase with underscores or dots

2. **Schema Design**
   - Always define `param_schema` for clarity
   - Use `out_schema` to document event data structure
   - Include descriptions in schemas
   - Validate event data against schemas

3. **Enable/Disable**
   - Disable unused triggers to reduce overhead
   - Test new triggers in disabled state first
   - Use enable/disable for maintenance windows

4. **Pack Organization**
   - Group related triggers in the same pack
   - Use pack-specific prefixes: `mypack.my_trigger`
   - System-wide triggers can omit pack_ref

### Sensors

1. **Naming Convention**
   - Use descriptive names: `cpu_monitor` not `cm`
   - Include pack prefix: `monitoring.cpu_sensor`
   - Indicate what is monitored: `disk_space_sensor`

2. **Configuration**
   - Use `param_schema` for all configurable values
   - Provide sensible defaults
   - Document intervals, thresholds, paths

3. **Entry Points**
   - Use consistent paths: `/sensors/category/name.ext`
   - Version sensor code: `/sensors/v1/cpu_monitor.py`
   - Keep entry points simple and focused

4. **Runtime Selection**
   - Match runtime to sensor needs
   - Python for complex logic
   - Shell for simple checks
   - Consider performance and resources

5. **Enable/Disable**
   - Start sensors disabled for testing
   - Disable during maintenance
   - Monitor sensor health and disable if failing

---

## Validation Rules

### Trigger Reference (`ref`)
- Must be unique across all triggers
- Alphanumeric, dots (.), underscores (_), hyphens (-)
- Pattern: `pack_name.trigger_name`
- Example: `core.webhook`, `github.push_event`

### Sensor Reference (`ref`)
- Must be unique across all sensors
- Alphanumeric, dots (.), underscores (_), hyphens (-)
- Pattern: `pack_name.sensor_name`
- Example: `monitoring.cpu_sensor`

### Pack Reference
- Optional for triggers (can be system-wide)
- Required for sensors
- Must reference existing pack

### Runtime Reference
- Required for sensors
- Must reference existing runtime
- Determines sensor execution environment

### Trigger Reference (for sensors)
- Required for sensors
- Must reference existing trigger
- Defines what event type the sensor fires

---

## Common Patterns

### Alert on Threshold

```
1. Create Trigger: "system.cpu_alert"
2. Create Sensor: Monitors CPU usage
3. Create Rule: Match CPU > 90%
4. Create Action: Send alert
```

### Webhook Integration

```
1. Create Trigger: "github.push_event"
2. Create Sensor: GitHub webhook listener
3. Create Rule: Match branch = "main"
4. Create Action: Deploy to staging
```

### Scheduled Tasks

```
1. Create Trigger: "core.timer"
2. Create Sensor: Cron-based scheduler
3. Create Rule: Match time condition
4. Create Action: Run backup
```

---

## Related Documentation

- [Pack Management API](./api-packs.md)
- [Action Management API](./api-actions.md)
- [Rule Management API](./api-rules.md)
- [Execution API](./api-executions.md)
- [Sensor Service](./sensor-service.md)

---

**Last Updated:** January 13, 2026