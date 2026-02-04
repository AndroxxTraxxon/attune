# Inquiry Handling - Human-in-the-Loop Workflows

## Overview

Inquiry handling enables **human-in-the-loop workflows** in Attune, allowing action executions to pause and wait for human input, approval, or decisions before continuing. This is essential for workflows that require manual intervention, approval gates, or interactive decision-making.

## Architecture

### Components

1. **Action** - Returns a result containing an inquiry request
2. **Worker** - Executes action and returns result with `__inquiry` marker
3. **Executor (Completion Listener)** - Detects inquiry request and creates inquiry
4. **Inquiry Record** - Database record tracking the inquiry state
5. **API** - Endpoints for users to view and respond to inquiries
6. **Executor (Inquiry Handler)** - Listens for responses and resumes executions
7. **Notifier** - Sends real-time notifications about inquiry events

### Message Flow

```
Action Execution → Worker completes → ExecutionCompleted message →
Completion Listener detects __inquiry → Creates Inquiry record →
Publishes InquiryCreated message → Notifier alerts users →
User responds via API → API publishes InquiryResponded message →
Inquiry Handler receives message → Updates execution with response →
Execution continues/completes
```

## Inquiry Request Format

### Action Result with Inquiry

Actions can request human input by including an `__inquiry` key in their result:

```json
{
  "__inquiry": {
    "prompt": "Approve deployment to production?",
    "response_schema": {
      "type": "object",
      "properties": {
        "approved": {"type": "boolean"},
        "comments": {"type": "string"}
      },
      "required": ["approved"]
    },
    "assigned_to": 123,
    "timeout_seconds": 3600
  },
  "deployment_plan": {
    "target": "production",
    "version": "v2.5.0"
  }
}
```

### Inquiry Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `prompt` | string | Yes | Question/prompt text displayed to user |
| `response_schema` | JSON Schema | No | Schema defining expected response format |
| `assigned_to` | integer | No | Identity ID of user assigned to respond |
| `timeout_seconds` | integer | No | Seconds from creation until inquiry times out |

## Creating Inquiries

### From Python Actions

```python
def run(deployment_plan):
    # Validate deployment plan
    validate_plan(deployment_plan)
    
    # Request human approval
    return {
        "__inquiry": {
            "prompt": f"Approve deployment of {deployment_plan['version']} to production?",
            "response_schema": {
                "type": "object",
                "properties": {
                    "approved": {"type": "boolean"},
                    "reason": {"type": "string"}
                },
                "required": ["approved"]
            },
            "timeout_seconds": 7200  # 2 hours
        },
        "plan": deployment_plan
    }
```

### From JavaScript Actions

```javascript
async function run(config) {
    // Prepare deployment
    const plan = await prepareDeploy(config);
    
    // Request approval with assigned user
    return {
        __inquiry: {
            prompt: `Deploy ${plan.serviceName} to ${plan.environment}?`,
            response_schema: {
                type: "object",
                properties: {
                    approved: { type: "boolean" },
                    comments: { type: "string" }
                }
            },
            assigned_to: config.approver_id,
            timeout_seconds: 3600
        },
        deployment: plan
    };
}
```

## Inquiry Lifecycle

### Status Flow

```
pending → responded (user provides response)
pending → timeout (timeout_at expires)
pending → cancelled (manual cancellation)
```

### Database Schema

```sql
CREATE TABLE attune.inquiry (
    id BIGSERIAL PRIMARY KEY,
    execution BIGINT NOT NULL REFERENCES attune.execution(id),
    prompt TEXT NOT NULL,
    response_schema JSONB,
    assigned_to BIGINT REFERENCES attune.identity(id),
    status attune.inquiry_status_enum NOT NULL DEFAULT 'pending',
    response JSONB,
    timeout_at TIMESTAMPTZ,
    responded_at TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## API Endpoints

### List Inquiries

**GET** `/api/v1/inquiries`

Query parameters:
- `status` - Filter by status (pending, responded, timeout, cancelled)
- `execution` - Filter by execution ID
- `assigned_to` - Filter by assigned user ID
- `page`, `per_page` - Pagination

Response:
```json
{
  "data": [
    {
      "id": 1,
      "execution": 123,
      "prompt": "Approve deployment?",
      "assigned_to": 5,
      "status": "pending",
      "has_response": false,
      "timeout_at": "2024-01-15T12:00:00Z",
      "created": "2024-01-15T10:00:00Z"
    }
  ],
  "total": 1,
  "page": 1,
  "per_page": 50,
  "pages": 1
}
```

### Get Inquiry Details

**GET** `/api/v1/inquiries/:id`

Response:
```json
{
  "data": {
    "id": 1,
    "execution": 123,
    "prompt": "Approve deployment to production?",
    "response_schema": {
      "type": "object",
      "properties": {
        "approved": {"type": "boolean"}
      }
    },
    "assigned_to": 5,
    "status": "pending",
    "response": null,
    "timeout_at": "2024-01-15T12:00:00Z",
    "responded_at": null,
    "created": "2024-01-15T10:00:00Z",
    "updated": "2024-01-15T10:00:00Z"
  }
}
```

### Respond to Inquiry

**POST** `/api/v1/inquiries/:id/respond`

Request body:
```json
{
  "response": {
    "approved": true,
    "comments": "LGTM - all tests passed"
  }
}
```

Response:
```json
{
  "data": {
    "id": 1,
    "execution": 123,
    "status": "responded",
    "response": {
      "approved": true,
      "comments": "LGTM - all tests passed"
    },
    "responded_at": "2024-01-15T10:30:00Z"
  },
  "message": "Response submitted successfully"
}
```

### Cancel Inquiry

**POST** `/api/v1/inquiries/:id/cancel`

Cancels a pending inquiry (admin/system use).

## Message Queue Events

### InquiryCreated

Published when an inquiry is created.

Routing key: `inquiry.created`

Payload:
```json
{
  "inquiry_id": 1,
  "execution_id": 123,
  "prompt": "Approve deployment?",
  "response_schema": {...},
  "assigned_to": 5,
  "timeout_at": "2024-01-15T12:00:00Z"
}
```

### InquiryResponded

Published when a user responds to an inquiry.

Routing key: `inquiry.responded`

Payload:
```json
{
  "inquiry_id": 1,
  "execution_id": 123,
  "response": {
    "approved": true
  },
  "responded_by": 5,
  "responded_at": "2024-01-15T10:30:00Z"
}
```

## Executor Service Integration

### Completion Listener

The completion listener detects inquiry requests in execution results:

```rust
// Check if execution result contains an inquiry request
if let Some(result) = &exec.result {
    if InquiryHandler::has_inquiry_request(result) {
        // Create inquiry and publish InquiryCreated message
        InquiryHandler::create_inquiry_from_result(
            pool,
            publisher,
            execution_id,
            result,
        ).await?;
    }
}
```

### Inquiry Handler

The inquiry handler processes inquiry responses:

```rust
// Listen for InquiryResponded messages
consumer.consume_with_handler(|envelope: MessageEnvelope<InquiryRespondedPayload>| {
    async move {
        // Update execution with inquiry response
        Self::resume_execution_with_response(
            pool,
            publisher,
            execution,
            inquiry,
            response,
        ).await?;
    }
}).await?;
```

### Timeout Checker

A background task periodically checks for expired inquiries:

```rust
// Run every 60 seconds
InquiryHandler::timeout_check_loop(pool, 60).await;
```

This updates pending inquiries to `timeout` status when `timeout_at` is exceeded.

## Access Control

### Assignment Enforcement

If an inquiry has `assigned_to` set, only that user can respond:

```rust
if let Some(assigned_to) = inquiry.assigned_to {
    if assigned_to != user_id {
        return Err(ApiError::Forbidden("Not authorized to respond"));
    }
}
```

### RBAC Integration (Future)

Future versions will integrate with RBAC for:
- Permission to respond to inquiries
- Permission to cancel inquiries
- Visibility filtering based on roles

## Timeout Handling

### Automatic Timeout

Inquiries with `timeout_at` set are automatically marked as timed out:

```sql
UPDATE attune.inquiry
SET status = 'timeout', updated = NOW()
WHERE status = 'pending'
  AND timeout_at IS NOT NULL
  AND timeout_at < NOW();
```

### Timeout Behavior

When an inquiry times out:
1. Status changes to `timeout`
2. Execution remains in current state
3. Optional: Publish timeout event
4. Optional: Resume execution with timeout indicator

## Real-Time Notifications

### WebSocket Integration

The Notifier service sends real-time notifications for inquiry events:

```javascript
// Subscribe to inquiry notifications
ws.send(JSON.stringify({
    type: "subscribe",
    filters: {
        entity_type: "inquiry",
        user_id: 5
    }
}));

// Receive notification
{
    "id": 123,
    "entity_type": "inquiry",
    "entity": "1",
    "activity": "created",
    "content": {
        "prompt": "Approve deployment?",
        "assigned_to": 5
    }
}
```

### Notification Triggers

- **inquiry.created** - New inquiry created
- **inquiry.responded** - Inquiry received response
- **inquiry.timeout** - Inquiry timed out
- **inquiry.cancelled** - Inquiry was cancelled

## Use Cases

### Deployment Approval

```python
def deploy_to_production(config):
    # Prepare deployment
    plan = prepare_deployment(config)
    
    # Request approval
    return {
        "__inquiry": {
            "prompt": f"Approve deployment of {config['service']} v{config['version']}?",
            "response_schema": {
                "type": "object",
                "properties": {
                    "approved": {"type": "boolean"},
                    "rollback_plan": {"type": "string"}
                }
            },
            "assigned_to": get_on_call_engineer(),
            "timeout_seconds": 1800  # 30 minutes
        },
        "deployment_plan": plan
    }
```

### Data Validation

```python
def validate_data_import(data):
    # Check for anomalies
    anomalies = detect_anomalies(data)
    
    if anomalies:
        return {
            "__inquiry": {
                "prompt": f"Found {len(anomalies)} anomalies. Continue import?",
                "response_schema": {
                    "type": "object",
                    "properties": {
                        "continue": {"type": "boolean"},
                        "exclude_records": {"type": "array", "items": {"type": "integer"}}
                    }
                },
                "timeout_seconds": 3600
            },
            "anomalies": anomalies
        }
    
    # No anomalies, proceed normally
    return import_data(data)
```

### Configuration Review

```python
def update_firewall_rules(rules):
    # Analyze impact
    impact = analyze_impact(rules)
    
    if impact["severity"] == "high":
        return {
            "__inquiry": {
                "prompt": "High-impact firewall changes detected. Approve?",
                "response_schema": {
                    "type": "object",
                    "properties": {
                        "approved": {"type": "boolean"},
                        "review_notes": {"type": "string"}
                    }
                },
                "assigned_to": get_security_team_lead(),
                "timeout_seconds": 7200
            },
            "impact_analysis": impact,
            "proposed_rules": rules
        }
    
    # Low impact, apply immediately
    return apply_rules(rules)
```

## Best Practices

### 1. Clear Prompts

Write clear, actionable prompts:

✅ Good: "Approve deployment of api-service v2.1.0 to production?"
❌ Bad: "Continue?"

### 2. Reasonable Timeouts

Set appropriate timeout values:

- **Critical decisions**: 30-60 minutes
- **Routine approvals**: 2-4 hours
- **Non-urgent reviews**: 24-48 hours

### 3. Response Schemas

Define clear response schemas to validate user input:

```json
{
  "type": "object",
  "properties": {
    "approved": {
      "type": "boolean",
      "description": "Whether to approve the action"
    },
    "comments": {
      "type": "string",
      "description": "Optional comments explaining the decision"
    }
  },
  "required": ["approved"]
}
```

### 4. Assignment

Assign inquiries to specific users for accountability:

```python
{
    "__inquiry": {
        "prompt": "...",
        "assigned_to": get_on_call_user_id()
    }
}
```

### 5. Context Information

Include relevant context in the action result:

```python
return {
    "__inquiry": {
        "prompt": "Approve deployment?"
    },
    "deployment_details": {
        "service": "api",
        "version": "v2.1.0",
        "changes": ["Added new endpoint", "Fixed bug #123"],
        "tests_passed": True,
        "ci_build_url": "https://ci.example.com/builds/456"
    }
}
```

## Troubleshooting

### Inquiry Not Created

**Problem**: Action completes but no inquiry is created.

**Check**:
1. Action result contains `__inquiry` key
2. Completion listener is running
3. Check executor logs for errors
4. Verify inquiry table exists

### Execution Not Resuming

**Problem**: User responds but execution doesn't continue.

**Check**:
1. InquiryResponded message was published (check API logs)
2. Inquiry handler is running and consuming messages
3. Check executor logs for errors processing response
4. Verify execution exists and is in correct state

### Timeout Not Working

**Problem**: Inquiries not timing out automatically.

**Check**:
1. Timeout checker loop is running
2. `timeout_at` is set correctly in inquiry record
3. Check system time/timezone configuration
4. Review executor logs for timeout check errors

### Response Rejected

**Problem**: API rejects inquiry response.

**Check**:
1. Inquiry is still in `pending` status
2. Inquiry hasn't timed out
3. User is authorized (if `assigned_to` is set)
4. Response matches `response_schema` (when validation is enabled)

## Performance Considerations

### Database Indexes

Ensure these indexes exist for efficient inquiry queries:

```sql
CREATE INDEX idx_inquiry_status ON attune.inquiry(status);
CREATE INDEX idx_inquiry_assigned_status ON attune.inquiry(assigned_to, status);
CREATE INDEX idx_inquiry_timeout_at ON attune.inquiry(timeout_at) WHERE timeout_at IS NOT NULL;
```

### Message Queue

- Use separate consumer for inquiry responses
- Set appropriate prefetch count (10-20)
- Enable message acknowledgment

### Timeout Checking

- Run timeout checker every 60-120 seconds
- Use batched updates for efficiency
- Monitor for long-running timeout queries

## Security

### Input Validation

Always validate inquiry responses:

```rust
// TODO: Validate response against response_schema
if let Some(schema) = &inquiry.response_schema {
    validate_json_schema(&request.response, schema)?;
}
```

### Authorization

Verify user permissions:

```rust
// Check assignment
if let Some(assigned_to) = inquiry.assigned_to {
    if assigned_to != user.id {
        return Err(ApiError::Forbidden("Not authorized"));
    }
}

// Future: Check RBAC permissions
if !user.has_permission("inquiry:respond") {
    return Err(ApiError::Forbidden("Missing permission"));
}
```

### Audit Trail

All inquiry responses are logged:

- Who responded
- When they responded
- What they responded with
- Original inquiry context

## Future Enhancements

### Planned Features

1. **Multi-step Approvals** - Chain multiple inquiries for approval workflows
2. **Conditional Resumption** - Resume execution differently based on response
3. **Inquiry Templates** - Reusable inquiry definitions
4. **Bulk Operations** - Approve/reject multiple inquiries at once
5. **Escalation** - Auto-reassign if no response within timeframe
6. **Reminder Notifications** - Alert users of pending inquiries
7. **Response Validation** - Validate responses against JSON schema
8. **Inquiry History** - View history of all inquiries for an execution chain

### Integration Opportunities

- **Slack/Teams** - Respond to inquiries via chat
- **Email** - Send inquiry notifications and accept email responses
- **Mobile Apps** - Native mobile inquiry interface
- **External Systems** - Webhook integration for external approval systems

## Related Documentation

- [Workflow Orchestration](workflow-orchestration.md)
- [Message Queue Architecture](message-queue.md)
- [Notifier Service](notifier-service.md)
- [API Documentation](api-overview.md)
- [Executor Service](executor-service.md)