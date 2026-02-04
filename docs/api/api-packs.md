# Pack Management API

This document provides comprehensive documentation for the Pack Management API endpoints in Attune.

## Overview

Packs are containers that bundle related automation components (actions, triggers, rules, and sensors) together. They provide:

- **Organization**: Group related automation components by domain or service
- **Versioning**: Track and manage versions of automation components
- **Configuration**: Define pack-level configuration schemas and defaults
- **Dependencies**: Declare runtime and pack dependencies
- **Metadata**: Store tags, descriptions, and other metadata

## Pack Data Model

### Pack Structure

```json
{
  "id": 1,
  "ref": "aws.ec2",
  "label": "AWS EC2 Pack",
  "description": "Actions and triggers for AWS EC2 management",
  "version": "1.0.0",
  "conf_schema": {
    "type": "object",
    "properties": {
      "region": {"type": "string"},
      "access_key_id": {"type": "string"},
      "secret_access_key": {"type": "string"}
    },
    "required": ["region"]
  },
  "config": {
    "region": "us-east-1"
  },
  "meta": {
    "author": "Attune Team",
    "license": "MIT"
  },
  "tags": ["aws", "ec2", "cloud"],
  "runtime_deps": [1, 2],
  "is_standard": true,
  "created": "2024-01-15T10:30:00Z",
  "updated": "2024-01-15T10:30:00Z"
}
```

### Field Descriptions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | integer | auto | Unique pack identifier |
| `ref` | string | yes | Unique reference (e.g., "aws.ec2", "core.http") |
| `label` | string | yes | Human-readable pack name |
| `description` | string | yes | Pack description and purpose |
| `version` | string | no | Semantic version (e.g., "1.0.0") |
| `conf_schema` | object | no | JSON Schema for pack configuration |
| `config` | object | no | Default pack configuration values |
| `meta` | object | no | Additional metadata (author, license, etc.) |
| `tags` | array | no | Tags for categorization and search |
| `runtime_deps` | array | no | Runtime IDs this pack depends on |
| `is_standard` | boolean | no | Whether this is a standard/built-in pack |
| `created` | timestamp | auto | Creation timestamp |
| `updated` | timestamp | auto | Last update timestamp |

## API Endpoints

### 1. List Packs

Retrieve a paginated list of all packs.

**Endpoint:** `GET /api/v1/packs`

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | integer | 1 | Page number (1-based) |
| `per_page` | integer | 50 | Items per page (max: 100) |

**Example Request:**

```bash
curl -X GET "http://localhost:3000/api/v1/packs?page=1&per_page=20" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "data": [
    {
      "id": 1,
      "ref": "core.http",
      "label": "HTTP Core Pack",
      "description": "HTTP actions and triggers",
      "version": "1.0.0",
      "tags": ["http", "core"],
      "is_standard": true,
      "created": "2024-01-15T10:30:00Z",
      "updated": "2024-01-15T10:30:00Z"
    },
    {
      "id": 2,
      "ref": "aws.ec2",
      "label": "AWS EC2 Pack",
      "description": "AWS EC2 management",
      "version": "1.0.0",
      "tags": ["aws", "ec2", "cloud"],
      "is_standard": false,
      "created": "2024-01-16T14:20:00Z",
      "updated": "2024-01-16T14:20:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 2,
    "total_pages": 1
  }
}
```

### 2. Get Pack by Reference

Retrieve a specific pack by its reference identifier.

**Endpoint:** `GET /api/v1/packs/:ref`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference (e.g., "aws.ec2") |

**Example Request:**

```bash
curl -X GET "http://localhost:3000/api/v1/packs/aws.ec2" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "data": {
    "id": 2,
    "ref": "aws.ec2",
    "label": "AWS EC2 Pack",
    "description": "Actions and triggers for AWS EC2 management",
    "version": "1.0.0",
    "conf_schema": {
      "type": "object",
      "properties": {
        "region": {"type": "string"},
        "access_key_id": {"type": "string"}
      }
    },
    "config": {
      "region": "us-east-1"
    },
    "meta": {
      "author": "Attune Team"
    },
    "tags": ["aws", "ec2", "cloud"],
    "runtime_deps": [1],
    "is_standard": false,
    "created": "2024-01-16T14:20:00Z",
    "updated": "2024-01-16T14:20:00Z"
  }
}
```

### 3. Create Pack

Create a new pack.

**Endpoint:** `POST /api/v1/packs`

**Request Body:**

```json
{
  "ref": "slack.notifications",
  "label": "Slack Notifications Pack",
  "description": "Actions for sending Slack notifications",
  "version": "1.0.0",
  "conf_schema": {
    "type": "object",
    "properties": {
      "webhook_url": {"type": "string"},
      "default_channel": {"type": "string"}
    },
    "required": ["webhook_url"]
  },
  "config": {
    "default_channel": "#general"
  },
  "meta": {
    "author": "Your Team",
    "license": "MIT"
  },
  "tags": ["slack", "notifications", "messaging"],
  "runtime_deps": [1],
  "is_standard": false
}
```

**Example Request:**

```bash
curl -X POST "http://localhost:3000/api/v1/packs" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "slack.notifications",
    "label": "Slack Notifications Pack",
    "description": "Actions for sending Slack notifications",
    "version": "1.0.0",
    "tags": ["slack", "notifications"],
    "is_standard": false
  }'
```

**Example Response:**

```json
{
  "data": {
    "id": 3,
    "ref": "slack.notifications",
    "label": "Slack Notifications Pack",
    "description": "Actions for sending Slack notifications",
    "version": "1.0.0",
    "conf_schema": null,
    "config": null,
    "meta": null,
    "tags": ["slack", "notifications"],
    "runtime_deps": [],
    "is_standard": false,
    "created": "2024-01-17T09:15:00Z",
    "updated": "2024-01-17T09:15:00Z"
  },
  "message": "Pack created successfully"
}
```

**Status Codes:**

- `201 Created`: Pack created successfully
- `400 Bad Request`: Invalid request data or validation error
- `409 Conflict`: Pack with the same ref already exists

### 4. Update Pack

Update an existing pack.

**Endpoint:** `PUT /api/v1/packs/:ref`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference to update |

**Request Body:** (all fields optional)

```json
{
  "label": "Updated Pack Label",
  "description": "Updated description",
  "version": "1.1.0",
  "conf_schema": {...},
  "config": {...},
  "meta": {...},
  "tags": ["updated", "tags"],
  "runtime_deps": [1, 2],
  "is_standard": false
}
```

**Example Request:**

```bash
curl -X PUT "http://localhost:3000/api/v1/packs/slack.notifications" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "version": "1.1.0",
    "description": "Enhanced Slack notifications with rich formatting"
  }'
```

**Example Response:**

```json
{
  "data": {
    "id": 3,
    "ref": "slack.notifications",
    "label": "Slack Notifications Pack",
    "description": "Enhanced Slack notifications with rich formatting",
    "version": "1.1.0",
    "conf_schema": null,
    "config": null,
    "meta": null,
    "tags": ["slack", "notifications"],
    "runtime_deps": [],
    "is_standard": false,
    "created": "2024-01-17T09:15:00Z",
    "updated": "2024-01-17T09:30:00Z"
  },
  "message": "Pack updated successfully"
}
```

**Status Codes:**

- `200 OK`: Pack updated successfully
- `400 Bad Request`: Invalid request data or validation error
- `404 Not Found`: Pack not found

### 5. Delete Pack

Delete a pack and all its associated components.

**Endpoint:** `DELETE /api/v1/packs/:ref`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference to delete |

**Example Request:**

```bash
curl -X DELETE "http://localhost:3000/api/v1/packs/slack.notifications" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "success": true,
  "message": "Pack 'slack.notifications' deleted successfully"
}
```

**Status Codes:**

- `200 OK`: Pack deleted successfully
- `404 Not Found`: Pack not found

**Warning:** Deleting a pack will cascade delete all actions, triggers, rules, and sensors that belong to it. This operation cannot be undone.

### 6. List Pack Actions

Retrieve all actions that belong to a specific pack.

**Endpoint:** `GET /api/v1/packs/:ref/actions`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference |

**Example Request:**

```bash
curl -X GET "http://localhost:3000/api/v1/packs/aws.ec2/actions" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "data": [
    {
      "id": 1,
      "ref": "aws.ec2.start_instance",
      "pack_ref": "aws.ec2",
      "label": "Start EC2 Instance",
      "description": "Start an EC2 instance",
      "runtime": 1,
      "created": "2024-01-16T14:30:00Z",
      "updated": "2024-01-16T14:30:00Z"
    },
    {
      "id": 2,
      "ref": "aws.ec2.stop_instance",
      "pack_ref": "aws.ec2",
      "label": "Stop EC2 Instance",
      "description": "Stop an EC2 instance",
      "runtime": 1,
      "created": "2024-01-16T14:35:00Z",
      "updated": "2024-01-16T14:35:00Z"
    }
  ]
}
```

**Status Codes:**

- `200 OK`: Actions retrieved successfully (empty array if none)
- `404 Not Found`: Pack not found

### 7. List Pack Triggers

Retrieve all triggers that belong to a specific pack.

**Endpoint:** `GET /api/v1/packs/:ref/triggers`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference |

**Example Request:**

```bash
curl -X GET "http://localhost:3000/api/v1/packs/aws.ec2/triggers" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "data": [
    {
      "id": 1,
      "ref": "aws.ec2.instance_state_change",
      "pack_ref": "aws.ec2",
      "label": "EC2 Instance State Change",
      "description": "Triggered when EC2 instance state changes",
      "enabled": true,
      "created": "2024-01-16T14:40:00Z",
      "updated": "2024-01-16T14:40:00Z"
    }
  ]
}
```

**Status Codes:**

- `200 OK`: Triggers retrieved successfully (empty array if none)
- `404 Not Found`: Pack not found

### 8. List Pack Rules

Retrieve all rules that belong to a specific pack.

**Endpoint:** `GET /api/v1/packs/:ref/rules`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Pack reference |

**Example Request:**

```bash
curl -X GET "http://localhost:3000/api/v1/packs/aws.ec2/rules" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Example Response:**

```json
{
  "data": [
    {
      "id": 1,
      "ref": "aws.ec2.auto_stop_idle",
      "pack_ref": "aws.ec2",
      "label": "Auto-stop Idle Instances",
      "description": "Automatically stop idle EC2 instances",
      "action_ref": "aws.ec2.stop_instance",
      "trigger_ref": "aws.ec2.instance_state_change",
      "enabled": true,
      "created": "2024-01-16T15:00:00Z",
      "updated": "2024-01-16T15:00:00Z"
    }
  ]
}
```

**Status Codes:**

- `200 OK`: Rules retrieved successfully (empty array if none)
- `404 Not Found`: Pack not found

## Pack Lifecycle

### 1. Pack Creation Workflow

```
1. Define pack metadata (ref, label, description)
2. Create configuration schema (optional)
3. Set default configuration values (optional)
4. Add metadata and tags
5. Specify runtime dependencies
6. POST to /api/v1/packs
7. Create pack actions, triggers, and rules
```

### 2. Pack Update Workflow

```
1. Retrieve current pack details (GET /api/v1/packs/:ref)
2. Modify desired fields
3. Update pack (PUT /api/v1/packs/:ref)
4. Update version number if making breaking changes
5. Update dependent components if needed
```

### 3. Pack Deletion Workflow

```
1. List all pack components:
   - GET /api/v1/packs/:ref/actions
   - GET /api/v1/packs/:ref/triggers
   - GET /api/v1/packs/:ref/rules
2. Verify no critical dependencies
3. Backup pack configuration if needed
4. DELETE /api/v1/packs/:ref
5. All components cascade deleted automatically
```

## Configuration Schema

Packs can define a JSON Schema for their configuration. This schema validates pack configuration values and provides documentation for users.

### Example Configuration Schema

```json
{
  "type": "object",
  "properties": {
    "api_key": {
      "type": "string",
      "description": "API key for authentication"
    },
    "region": {
      "type": "string",
      "enum": ["us-east-1", "us-west-2", "eu-west-1"],
      "default": "us-east-1",
      "description": "AWS region"
    },
    "timeout": {
      "type": "integer",
      "minimum": 1000,
      "maximum": 30000,
      "default": 5000,
      "description": "Request timeout in milliseconds"
    },
    "retry_count": {
      "type": "integer",
      "minimum": 0,
      "maximum": 5,
      "default": 3,
      "description": "Number of retry attempts"
    }
  },
  "required": ["api_key", "region"]
}
```

### Using Configuration in Actions

Actions within a pack can access pack configuration:

```python
# In an action's Python script
pack_config = context.pack.config
api_key = pack_config.get("api_key")
region = pack_config.get("region", "us-east-1")
```

## Best Practices

### Pack Design

1. **Single Responsibility**: Each pack should focus on a specific domain or service
2. **Versioning**: Use semantic versioning (MAJOR.MINOR.PATCH)
3. **Documentation**: Provide clear descriptions for pack and configuration
4. **Dependencies**: Minimize runtime dependencies when possible
5. **Configuration**: Use sensible defaults in configuration schemas

### Naming Conventions

- **Pack Ref**: Use dot notation (e.g., "aws.ec2", "slack.notifications")
- **Labels**: Use title case (e.g., "AWS EC2 Pack")
- **Tags**: Use lowercase, kebab-case if needed (e.g., "aws", "cloud-provider")

### Security

1. **Secrets**: Never store secrets in pack configuration
2. **Schema**: Define strict configuration schemas
3. **Validation**: Validate all configuration values
4. **Access Control**: Limit pack modification to authorized users

### Organization

1. **Standard Packs**: Mark built-in/core packs as `is_standard: true`
2. **Tagging**: Use consistent tags for discoverability
3. **Metadata**: Include author, license, and documentation URLs
4. **Dependencies**: Document runtime requirements clearly

## Error Handling

### Common Error Responses

**404 Not Found:**
```json
{
  "error": "Pack 'nonexistent.pack' not found"
}
```

**409 Conflict:**
```json
{
  "error": "Pack with ref 'aws.ec2' already exists"
}
```

**400 Bad Request:**
```json
{
  "error": "Validation failed",
  "details": {
    "ref": ["Must be between 1 and 255 characters"],
    "label": ["Field is required"]
  }
}
```

## Integration Examples

### Creating a Complete Pack

```bash
#!/bin/bash

# 1. Create the pack
PACK_ID=$(curl -X POST "http://localhost:3000/api/v1/packs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "monitoring.healthcheck",
    "label": "Health Check Monitoring",
    "description": "Monitor endpoint health",
    "version": "1.0.0",
    "tags": ["monitoring", "health"],
    "is_standard": false
  }' | jq -r '.data.id')

# 2. Create a trigger
curl -X POST "http://localhost:3000/api/v1/triggers" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "monitoring.healthcheck.endpoint_down",
    "pack_ref": "monitoring.healthcheck",
    "label": "Endpoint Down",
    "description": "Triggered when endpoint is unreachable"
  }'

# 3. Create an action
curl -X POST "http://localhost:3000/api/v1/actions" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "monitoring.healthcheck.send_alert",
    "pack_ref": "monitoring.healthcheck",
    "label": "Send Alert",
    "description": "Send notification about endpoint status",
    "entrypoint": "actions/send_alert.py",
    "runtime": 1
  }'

# 4. Create a rule
curl -X POST "http://localhost:3000/api/v1/rules" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "monitoring.healthcheck.alert_on_down",
    "pack_ref": "monitoring.healthcheck",
    "label": "Alert on Down",
    "description": "Send alert when endpoint goes down",
    "action_ref": "monitoring.healthcheck.send_alert",
    "trigger_ref": "monitoring.healthcheck.endpoint_down",
    "enabled": true
  }'

echo "Pack created successfully!"
```

### Listing Pack Components

```bash
#!/bin/bash

PACK_REF="aws.ec2"

echo "=== Pack Details ==="
curl -s "http://localhost:3000/api/v1/packs/$PACK_REF" \
  -H "Authorization: Bearer $TOKEN" | jq '.data'

echo -e "\n=== Actions ==="
curl -s "http://localhost:3000/api/v1/packs/$PACK_REF/actions" \
  -H "Authorization: Bearer $TOKEN" | jq '.data[] | {ref, label}'

echo -e "\n=== Triggers ==="
curl -s "http://localhost:3000/api/v1/packs/$PACK_REF/triggers" \
  -H "Authorization: Bearer $TOKEN" | jq '.data[] | {ref, label}'

echo -e "\n=== Rules ==="
curl -s "http://localhost:3000/api/v1/packs/$PACK_REF/rules" \
  -H "Authorization: Bearer $TOKEN" | jq '.data[] | {ref, label, enabled}'
```

## Related Documentation

- [Action Management API](api-actions.md)
- [Trigger & Sensor Management API](api-triggers-sensors.md)
- [Rule Management API](api-rules.md)
- [Authentication Guide](authentication.md)

## Summary

The Pack Management API provides:

- ✅ Full CRUD operations for packs
- ✅ Pack component listing (actions, triggers, rules)
- ✅ Configuration schema support
- ✅ Version management
- ✅ Dependency tracking
- ✅ Comprehensive validation and error handling

Packs are the organizational foundation of Attune, enabling modular and reusable automation components.