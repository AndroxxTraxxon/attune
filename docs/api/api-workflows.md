# Workflow Management API

This document describes the Workflow Management API endpoints in Attune.

## Overview

Workflows are multi-step automation sequences that orchestrate multiple actions, handle conditional logic, and manage complex execution flows. The Workflow API provides endpoints for creating, managing, and querying workflow definitions.

## Endpoints

### List Workflows

List all workflows with optional filtering and pagination.

**Endpoint**: `GET /api/v1/workflows`

**Authentication**: Required (Bearer token)

**Query Parameters**:
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)
- `tags` (string, optional): Filter by tags (comma-separated list)
- `enabled` (boolean, optional): Filter by enabled status
- `search` (string, optional): Search term for label/description (case-insensitive)
- `pack_ref` (string, optional): Filter by pack reference

**Example Request**:
```bash
curl -X GET "http://localhost:8080/api/v1/workflows?page=1&per_page=20&enabled=true&tags=incident,approval" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}"
```

**Example Response** (200 OK):
```json
{
  "data": [
    {
      "id": 1,
      "ref": "slack.incident_workflow",
      "pack_ref": "slack",
      "label": "Incident Response Workflow",
      "description": "Automated incident response workflow",
      "version": "1.0.0",
      "tags": ["incident", "approval"],
      "enabled": true,
      "created": "2024-01-13T10:30:00Z",
      "updated": "2024-01-13T10:30:00Z"
    }
  ],
  "meta": {
    "page": 1,
    "per_page": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

---

### Get Workflow by Reference

Get detailed information about a specific workflow.

**Endpoint**: `GET /api/v1/workflows/{ref}`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string): Workflow reference identifier (e.g., "slack.incident_workflow")

**Example Request**:
```bash
curl -X GET "http://localhost:8080/api/v1/workflows/slack.incident_workflow" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}"
```

**Example Response** (200 OK):
```json
{
  "data": {
    "id": 1,
    "ref": "slack.incident_workflow",
    "pack": 1,
    "pack_ref": "slack",
    "label": "Incident Response Workflow",
    "description": "Automated incident response with notifications and approvals",
    "version": "1.0.0",
    "param_schema": {
      "type": "object",
      "properties": {
        "severity": {
          "type": "string",
          "enum": ["low", "medium", "high", "critical"]
        },
        "channel": {
          "type": "string"
        }
      },
      "required": ["severity", "channel"]
    },
    "out_schema": {
      "type": "object",
      "properties": {
        "incident_id": {
          "type": "string"
        },
        "resolved": {
          "type": "boolean"
        }
      }
    },
    "definition": {
      "tasks": [
        {
          "name": "notify_team",
          "action": "slack.post_message",
          "input": {
            "channel": "{{ channel }}",
            "message": "Incident detected: {{ severity }}"
          }
        },
        {
          "name": "create_ticket",
          "action": "jira.create_issue",
          "input": {
            "project": "INC",
            "summary": "Incident: {{ severity }}",
            "description": "Auto-generated incident"
          }
        },
        {
          "name": "await_approval",
          "action": "core.inquiry",
          "input": {
            "timeout": 3600,
            "approvers": ["oncall@company.com"]
          }
        }
      ]
    },
    "tags": ["incident", "approval", "slack"],
    "enabled": true,
    "created": "2024-01-13T10:30:00Z",
    "updated": "2024-01-13T10:30:00Z"
  }
}
```

**Error Responses**:
- `404 Not Found`: Workflow not found

---

### List Workflows by Pack

List all workflows belonging to a specific pack.

**Endpoint**: `GET /api/v1/packs/{pack_ref}/workflows`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `pack_ref` (string): Pack reference identifier

**Query Parameters**:
- `page` (integer, optional): Page number (default: 1)
- `per_page` (integer, optional): Items per page (default: 20, max: 100)

**Example Request**:
```bash
curl -X GET "http://localhost:8080/api/v1/packs/slack/workflows" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}"
```

**Example Response** (200 OK):
```json
{
  "data": [
    {
      "id": 1,
      "ref": "slack.incident_workflow",
      "pack_ref": "slack",
      "label": "Incident Response Workflow",
      "description": "Automated incident response workflow",
      "version": "1.0.0",
      "tags": ["incident"],
      "enabled": true,
      "created": "2024-01-13T10:30:00Z",
      "updated": "2024-01-13T10:30:00Z"
    }
  ],
  "meta": {
    "page": 1,
    "per_page": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

**Error Responses**:
- `404 Not Found`: Pack not found

---

### Create Workflow

Create a new workflow definition.

**Endpoint**: `POST /api/v1/workflows`

**Authentication**: Required (Bearer token)

**Request Body**:
```json
{
  "ref": "slack.incident_workflow",
  "pack_ref": "slack",
  "label": "Incident Response Workflow",
  "description": "Automated incident response with notifications",
  "version": "1.0.0",
  "param_schema": {
    "type": "object",
    "properties": {
      "severity": {
        "type": "string",
        "enum": ["low", "medium", "high", "critical"]
      },
      "channel": {
        "type": "string"
      }
    },
    "required": ["severity", "channel"]
  },
  "out_schema": {
    "type": "object",
    "properties": {
      "incident_id": {
        "type": "string"
      }
    }
  },
  "definition": {
    "tasks": [
      {
        "name": "notify_team",
        "action": "slack.post_message",
        "input": {
          "channel": "{{ channel }}",
          "message": "Incident: {{ severity }}"
        }
      }
    ]
  },
  "tags": ["incident", "approval"],
  "enabled": true
}
```

**Field Descriptions**:
- `ref` (string, required): Unique workflow reference (typically `pack_name.workflow_name`)
- `pack_ref` (string, required): Reference to the parent pack
- `label` (string, required): Human-readable workflow name
- `description` (string, optional): Workflow description
- `version` (string, required): Semantic version (e.g., "1.0.0")
- `param_schema` (object, optional): JSON Schema for workflow inputs
- `out_schema` (object, optional): JSON Schema for workflow outputs
- `definition` (object, required): Complete workflow definition (tasks, conditions, etc.)
- `tags` (array, optional): Tags for categorization and search
- `enabled` (boolean, optional): Whether workflow is enabled (default: true)

**Example Request**:
```bash
curl -X POST "http://localhost:8080/api/v1/workflows" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d @workflow.json
```

**Example Response** (201 Created):
```json
{
  "data": {
    "id": 1,
    "ref": "slack.incident_workflow",
    "pack": 1,
    "pack_ref": "slack",
    "label": "Incident Response Workflow",
    "description": "Automated incident response with notifications",
    "version": "1.0.0",
    "param_schema": { ... },
    "out_schema": { ... },
    "definition": { ... },
    "tags": ["incident", "approval"],
    "enabled": true,
    "created": "2024-01-13T10:30:00Z",
    "updated": "2024-01-13T10:30:00Z"
  },
  "message": "Workflow created successfully"
}
```

**Error Responses**:
- `400 Bad Request`: Validation error (invalid fields, missing required fields)
- `404 Not Found`: Pack not found
- `409 Conflict`: Workflow with same ref already exists

---

### Update Workflow

Update an existing workflow definition.

**Endpoint**: `PUT /api/v1/workflows/{ref}`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string): Workflow reference identifier

**Request Body** (all fields optional):
```json
{
  "label": "Updated Incident Response Workflow",
  "description": "Enhanced incident response with additional automation",
  "version": "1.1.0",
  "param_schema": { ... },
  "out_schema": { ... },
  "definition": { ... },
  "tags": ["incident", "approval", "automation"],
  "enabled": false
}
```

**Example Request**:
```bash
curl -X PUT "http://localhost:8080/api/v1/workflows/slack.incident_workflow" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Updated Incident Response",
    "version": "1.1.0",
    "enabled": false
  }'
```

**Example Response** (200 OK):
```json
{
  "data": {
    "id": 1,
    "ref": "slack.incident_workflow",
    "pack": 1,
    "pack_ref": "slack",
    "label": "Updated Incident Response",
    "description": "Automated incident response with notifications",
    "version": "1.1.0",
    "param_schema": { ... },
    "out_schema": { ... },
    "definition": { ... },
    "tags": ["incident", "approval"],
    "enabled": false,
    "created": "2024-01-13T10:30:00Z",
    "updated": "2024-01-13T11:45:00Z"
  },
  "message": "Workflow updated successfully"
}
```

**Error Responses**:
- `400 Bad Request`: Validation error
- `404 Not Found`: Workflow not found

---

### Delete Workflow

Delete a workflow definition.

**Endpoint**: `DELETE /api/v1/workflows/{ref}`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string): Workflow reference identifier

**Example Request**:
```bash
curl -X DELETE "http://localhost:8080/api/v1/workflows/slack.incident_workflow" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}"
```

**Example Response** (200 OK):
```json
{
  "success": true,
  "message": "Workflow 'slack.incident_workflow' deleted successfully"
}
```

**Error Responses**:
- `404 Not Found`: Workflow not found

**Note**: Deleting a workflow will cascade delete associated workflow executions and task executions.

---

## Workflow Definition Structure

The `definition` field contains the complete workflow specification. Here's the structure:

### Basic Structure

```json
{
  "tasks": [
    {
      "name": "task_name",
      "action": "pack.action_name",
      "input": {
        "param1": "value1",
        "param2": "{{ variable }}"
      },
      "on_success": "next_task",
      "on_failure": "error_handler",
      "retry": {
        "count": 3,
        "delay": 60
      },
      "timeout": 300
    }
  ],
  "variables": {
    "initial_var": "value"
  }
}
```

### Task Fields

- `name` (string, required): Unique task identifier within the workflow
- `action` (string, required): Action reference to execute
- `input` (object, optional): Input parameters for the action (supports Jinja2 templates)
- `on_success` (string, optional): Next task to execute on success
- `on_failure` (string, optional): Task to execute on failure
- `retry` (object, optional): Retry configuration
  - `count` (integer): Number of retry attempts
  - `delay` (integer): Delay between retries in seconds
- `timeout` (integer, optional): Task timeout in seconds

### Variable Templating

Use Jinja2 syntax to reference workflow variables:
- `{{ variable_name }}`: Reference workflow variable
- `{{ task_name.output.field }}`: Reference task output
- `{{ workflow.param.field }}`: Reference workflow input parameter

### Example: Complex Workflow

```json
{
  "tasks": [
    {
      "name": "fetch_data",
      "action": "core.http",
      "input": {
        "url": "{{ workflow.param.api_url }}",
        "method": "GET"
      },
      "on_success": "process_data",
      "on_failure": "notify_error"
    },
    {
      "name": "process_data",
      "action": "core.transform",
      "input": {
        "data": "{{ fetch_data.output.body }}"
      },
      "on_success": "store_result"
    },
    {
      "name": "store_result",
      "action": "database.insert",
      "input": {
        "table": "results",
        "data": "{{ process_data.output }}"
      }
    },
    {
      "name": "notify_error",
      "action": "slack.post_message",
      "input": {
        "channel": "#errors",
        "message": "Workflow failed: {{ error.message }}"
      }
    }
  ],
  "variables": {
    "max_retries": 3,
    "timeout": 300
  }
}
```

---

## Filtering and Search

### Filter by Tags

To find workflows with specific tags:

```bash
GET /api/v1/workflows?tags=incident,approval
```

This returns workflows that have **any** of the specified tags.

### Filter by Enabled Status

To find only enabled workflows:

```bash
GET /api/v1/workflows?enabled=true
```

To find disabled workflows:

```bash
GET /api/v1/workflows?enabled=false
```

### Search by Text

To search workflows by label or description:

```bash
GET /api/v1/workflows?search=incident
```

This performs a case-insensitive search across workflow labels and descriptions.

### Filter by Pack

To find workflows from a specific pack:

```bash
GET /api/v1/workflows?pack_ref=slack
```

This returns only workflows belonging to the specified pack.

### Combine Filters

Filters can be combined:

```bash
GET /api/v1/workflows?enabled=true&tags=incident&search=response
```

---

## Best Practices

### Workflow Naming

- Use dot notation: `pack_name.workflow_name`
- Keep names descriptive but concise
- Use snake_case for workflow names

### Versioning

- Follow semantic versioning (MAJOR.MINOR.PATCH)
- Increment MAJOR for breaking changes
- Increment MINOR for new features
- Increment PATCH for bug fixes

### Task Organization

- Keep tasks focused on single responsibilities
- Use descriptive task names
- Define clear success/failure paths
- Set appropriate timeouts and retries

### Error Handling

- Always define `on_failure` handlers for critical tasks
- Use dedicated error notification tasks
- Log errors for debugging

### Performance

- Minimize task dependencies
- Use parallel execution where possible
- Set reasonable timeouts
- Consider task execution costs

---

## Common Use Cases

### 1. Incident Response

```json
{
  "tasks": [
    {
      "name": "create_incident",
      "action": "pagerduty.create_incident"
    },
    {
      "name": "notify_team",
      "action": "slack.post_message"
    },
    {
      "name": "create_ticket",
      "action": "jira.create_issue"
    }
  ]
}
```

### 2. Approval Workflow

```json
{
  "tasks": [
    {
      "name": "request_approval",
      "action": "core.inquiry",
      "input": {
        "approvers": ["manager@company.com"],
        "timeout": 3600
      },
      "on_success": "execute_action",
      "on_failure": "notify_rejection"
    },
    {
      "name": "execute_action",
      "action": "aws.deploy"
    },
    {
      "name": "notify_rejection",
      "action": "email.send"
    }
  ]
}
```

### 3. Data Pipeline

```json
{
  "tasks": [
    {
      "name": "extract",
      "action": "database.query"
    },
    {
      "name": "transform",
      "action": "core.transform",
      "input": {
        "data": "{{ extract.output }}"
      }
    },
    {
      "name": "load",
      "action": "s3.upload",
      "input": {
        "data": "{{ transform.output }}"
      }
    }
  ]
}
```

---

## Related Documentation

- [Actions API](./api-actions.md)
- [Executions API](./api-executions.md)
- [Packs API](./api-packs.md)
- [Workflow Execution Engine](./workflows.md)