# Pack Workflow API Documentation

This document describes the pack workflow integration endpoints that enable automatic loading, validation, and synchronization of workflow definitions from pack directories.

## Overview

The Pack Workflow API provides endpoints to:
- Automatically sync workflows from filesystem when packs are created/updated
- Manually trigger workflow synchronization for a pack
- Validate workflow definitions without registering them
- Integrate workflow lifecycle with pack management

## Endpoints

### Sync Pack Workflows

Synchronizes workflow definitions from the filesystem to the database for a specific pack.

**Endpoint:** `POST /api/v1/packs/{ref}/workflows/sync`

**Authentication:** Required (Bearer token)

**Path Parameters:**
- `ref` (string, required): Pack reference identifier

**Response:** `200 OK`

```json
{
  "data": {
    "pack_ref": "my_pack",
    "loaded_count": 5,
    "registered_count": 5,
    "workflows": [
      {
        "ref_name": "my_pack.deploy_app",
        "created": true,
        "workflow_def_id": 123,
        "warnings": []
      }
    ],
    "errors": []
  },
  "message": "Pack workflows synced successfully"
}
```

**Response Fields:**
- `pack_ref`: The pack reference that was synced
- `loaded_count`: Number of workflow files found and loaded from filesystem
- `registered_count`: Number of workflows successfully registered/updated in database
- `workflows`: Array of individual workflow sync results
  - `ref_name`: Full workflow reference (pack.workflow_name)
  - `created`: Whether workflow was created (true) or updated (false)
  - `workflow_def_id`: Database ID of the workflow definition
  - `warnings`: Any validation warnings (workflow still registered)
- `errors`: Array of error messages (workflows that failed to sync)

**Error Responses:**
- `404 Not Found`: Pack does not exist
- `401 Unauthorized`: Missing or invalid authentication token
- `500 Internal Server Error`: Failed to sync workflows

**Example:**

```bash
curl -X POST http://localhost:8080/api/v1/packs/core/workflows/sync \
  -H "Authorization: Bearer $TOKEN"
```

---

### Validate Pack Workflows

Validates workflow definitions from the filesystem without registering them in the database. Useful for checking workflow syntax and structure before deployment.

**Endpoint:** `POST /api/v1/packs/{ref}/workflows/validate`

**Authentication:** Required (Bearer token)

**Path Parameters:**
- `ref` (string, required): Pack reference identifier

**Response:** `200 OK`

```json
{
  "data": {
    "pack_ref": "my_pack",
    "validated_count": 5,
    "error_count": 1,
    "errors": {
      "my_pack.broken_workflow": [
        "Missing required field: version",
        "Task 'step1' references undefined action 'invalid.action'"
      ]
    }
  },
  "message": "Pack workflows validated"
}
```

**Response Fields:**
- `pack_ref`: The pack reference that was validated
- `validated_count`: Total number of workflow files validated
- `error_count`: Number of workflows with validation errors
- `errors`: Map of workflow references to their validation error messages

**Error Responses:**
- `404 Not Found`: Pack does not exist
- `401 Unauthorized`: Missing or invalid authentication token
- `500 Internal Server Error`: Failed to validate workflows

**Example:**

```bash
curl -X POST http://localhost:8080/api/v1/packs/core/workflows/validate \
  -H "Authorization: Bearer $TOKEN"
```

---

## Automatic Workflow Synchronization

Workflows are automatically synchronized in the following scenarios:

### Pack Creation

When a pack is created via `POST /api/v1/packs`, the system automatically attempts to load and register workflows from the pack's `workflows/` directory.

**Example:**

```bash
curl -X POST http://localhost:8080/api/v1/packs \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "my_pack",
    "label": "My Pack",
    "version": "1.0.0",
    "description": "A custom pack"
  }'
```

If workflows exist in `/opt/attune/packs/my_pack/workflows/`, they will be automatically loaded and registered.

### Pack Update

When a pack is updated via `PUT /api/v1/packs/{ref}`, the system automatically resyncs workflows to capture any changes.

**Example:**

```bash
curl -X PUT http://localhost:8080/api/v1/packs/my_pack \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "version": "1.1.0",
    "label": "My Pack (Updated)"
  }'
```

**Note:** Auto-sync uses `skip_validation_errors: true`, so pack operations won't fail if workflows have validation errors. Use the validate endpoint to check for errors.

---

## Workflow Directory Structure

Workflows must be placed in the `workflows/` subdirectory of the pack directory:

```
/opt/attune/packs/
  └── my_pack/
      ├── actions/
      ├── sensors/
      └── workflows/
          ├── deploy_app.yaml
          ├── rollback.yaml
          └── health_check.yml
```

**Workflow File Requirements:**
- Must be in YAML format (`.yaml` or `.yml` extension)
- Filename (without extension) becomes the workflow name
- Full workflow reference is `pack_ref.workflow_name`
- Must conform to the workflow schema (see Workflow API documentation)

---

## Configuration

The pack workflows feature uses the following configuration:

**Config File (`config.yaml`):**

```yaml
packs_base_dir: "/opt/attune/packs"  # Base directory for pack directories
```

**Environment Variable:**

```bash
export ATTUNE__PACKS_BASE_DIR="/opt/attune/packs"
```

Default: `/opt/attune/packs`

---

## Workflow Lifecycle

1. **Development:** Create workflow YAML files in pack's `workflows/` directory
2. **Validation:** Use validate endpoint to check for errors
3. **Deployment:** Create/update pack via API (auto-syncs workflows)
4. **Manual Sync:** Use sync endpoint to reload workflows after filesystem changes
5. **Execution:** Workflows become available for execution via workflow API

---

## Best Practices

### 1. Validate Before Deploy

Always validate workflows before deploying:

```bash
# Validate workflows
curl -X POST http://localhost:8080/api/v1/packs/my_pack/workflows/validate \
  -H "Authorization: Bearer $TOKEN"

# If validation passes, sync
curl -X POST http://localhost:8080/api/v1/packs/my_pack/workflows/sync \
  -H "Authorization: Bearer $TOKEN"
```

### 2. Version Control

Keep workflow YAML files in version control alongside pack code:

```
my_pack/
  ├── actions/
  │   └── deploy.py
  ├── workflows/
  │   └── deploy_app.yaml
  └── pack.yaml
```

### 3. Naming Conventions

Use descriptive workflow filenames that indicate their purpose:
- `deploy_production.yaml`
- `backup_database.yaml`
- `incident_response.yaml`

### 4. Handle Sync Errors

Check the `errors` field in sync responses:

```json
{
  "data": {
    "errors": [
      "workflow 'my_pack.invalid' failed validation: Missing required field"
    ]
  }
}
```

### 5. Incremental Updates

When updating workflows:
1. Modify YAML files on filesystem
2. Call sync endpoint to reload
3. Previous workflow versions are updated (not duplicated)

---

## Error Handling

### Common Errors

**Pack Not Found (404):**
```json
{
  "error": "Pack 'nonexistent_pack' not found"
}
```

**Validation Errors:**
Workflows with validation errors are reported but don't prevent sync:
```json
{
  "data": {
    "workflows": [...],
    "errors": [
      "Validation failed for my_pack.broken: Missing tasks field"
    ]
  }
}
```

**Filesystem Access:**
If pack directory doesn't exist on filesystem:
```json
{
  "data": {
    "loaded_count": 0,
    "registered_count": 0,
    "errors": ["Failed to load workflows: Directory not found"]
  }
}
```

---

## Integration Examples

### CI/CD Pipeline

```bash
#!/bin/bash
# deploy-pack.sh

PACK_NAME="my_pack"
PACK_VERSION="1.0.0"
API_URL="http://localhost:8080"

# 1. Validate workflows locally
echo "Validating workflows..."
response=$(curl -s -X POST "$API_URL/api/v1/packs/$PACK_NAME/workflows/validate" \
  -H "Authorization: Bearer $TOKEN")

error_count=$(echo $response | jq -r '.data.error_count')
if [ "$error_count" -gt 0 ]; then
  echo "Validation errors found:"
  echo $response | jq '.data.errors'
  exit 1
fi

# 2. Create or update pack
echo "Deploying pack..."
curl -X POST "$API_URL/api/v1/packs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"ref\": \"$PACK_NAME\",
    \"label\": \"My Pack\",
    \"version\": \"$PACK_VERSION\"
  }"

# 3. Verify sync
echo "Syncing workflows..."
curl -X POST "$API_URL/api/v1/packs/$PACK_NAME/workflows/sync" \
  -H "Authorization: Bearer $TOKEN"

echo "Deployment complete!"
```

### Development Workflow

```python
import requests

API_URL = "http://localhost:8080"
TOKEN = "your_access_token"

def sync_workflows(pack_ref: str):
    """Sync workflows after local changes."""
    response = requests.post(
        f"{API_URL}/api/v1/packs/{pack_ref}/workflows/sync",
        headers={"Authorization": f"Bearer {TOKEN}"}
    )
    
    data = response.json()["data"]
    print(f"Synced {data['registered_count']} workflows")
    
    if data['errors']:
        print("Errors:", data['errors'])
    
    return data

# Usage
sync_workflows("my_pack")
```

---

## Related Documentation

- [Workflow API](api-workflows.md) - Workflow definition management
- [Pack API](api-packs.md) - Pack management endpoints
- [Workflow Orchestration](workflow-orchestration.md) - Workflow execution and concepts

---

## Changelog

- **v0.1.0** (2024-01): Initial implementation of pack workflow integration
  - Auto-sync on pack create/update
  - Manual sync endpoint
  - Validation endpoint