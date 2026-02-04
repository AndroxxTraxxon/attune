# Rule Parameter Mapping Examples

This document provides practical, copy-paste ready examples of rule parameter mapping.

---

## Example 1: Static Parameters Only

**Use Case:** Simple echo with fixed message (included in seed data as `core.rule.timer_10s_echo`)

**Rule:**
```json
{
  "ref": "core.rule.timer_10s_echo",
  "pack_ref": "core",
  "trigger_ref": "core.intervaltimer",
  "action_ref": "core.echo",
  "action_params": {
    "message": "hello, world"
  },
  "enabled": true
}
```

**How it works:**
- The rule references the generic `core.intervaltimer` trigger type
- A sensor (`core.timer_10s_sensor`) is configured with `{"unit": "seconds", "interval": 10}` to fire this trigger every 10 seconds
- When the sensor fires the trigger, the rule evaluates and executes the `core.echo` action with the message "hello, world"

**Result:** Every 10 seconds, the timer sensor fires and the echo action receives the message "hello, world" to print to stdout.

**When to use:** Fixed notifications, health checks, scheduled tasks with constant parameters.

**Note:** This example is included in the core pack seed data (`scripts/seed_core_pack.sql`) and serves as a basic demonstration of rule functionality. The seed script creates both the sensor instance and the rule.

---

## Example 2: Dynamic from Trigger Payload

**Use Case:** Alert with error details from event

**Event Payload:**
```json
{
  "service": "payment-api",
  "error": "Database connection timeout",
  "severity": "critical",
  "timestamp": "2026-01-17T15:30:00Z"
}
```

**Rule:**
```json
{
  "ref": "alerts.error_notification",
  "pack_ref": "alerts",
  "trigger_ref": "core.error_event",
  "action_ref": "slack.post_message",
  "action_params": {
    "channel": "#incidents",
    "message": "🚨 Error in {{ trigger.payload.service }}: {{ trigger.payload.error }}",
    "severity": "{{ trigger.payload.severity }}",
    "timestamp": "{{ trigger.payload.timestamp }}"
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "channel": "#incidents",
  "message": "🚨 Error in payment-api: Database connection timeout",
  "severity": "critical",
  "timestamp": "2026-01-17T15:30:00Z"
}
```

**When to use:** Alerts, notifications, any scenario where event data drives the action.

---

## Example 3: Dynamic from Pack Config

**Use Case:** API integration with credentials from config

**Pack Config:**
```json
{
  "ref": "slack",
  "config": {
    "api_token": "xoxb-1234567890-abcdefghijk",
    "default_channel": "#general",
    "bot_name": "Attune Bot"
  }
}
```

**Rule:**
```json
{
  "ref": "slack.auto_notify",
  "pack_ref": "slack",
  "trigger_ref": "core.notification_event",
  "action_ref": "slack.post_message",
  "action_params": {
    "token": "{{ pack.config.api_token }}",
    "channel": "{{ pack.config.default_channel }}",
    "username": "{{ pack.config.bot_name }}",
    "message": "Notification triggered"
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "token": "xoxb-1234567890-abcdefghijk",
  "channel": "#general",
  "username": "Attune Bot",
  "message": "Notification triggered"
}
```

**When to use:** API integrations, any action requiring credentials or configuration.

---

## Example 4: Mixed Static and Dynamic

**Use Case:** GitHub issue creation with mixed parameters

**Event Payload:**
```json
{
  "error_message": "Memory leak detected",
  "severity": "high",
  "service": "worker-pool",
  "stack_trace": "Error at line 42..."
}
```

**Pack Config:**
```json
{
  "ref": "github",
  "config": {
    "token": "ghp_xxxxxxxxxxxx",
    "repo_owner": "myorg",
    "repo_name": "myrepo"
  }
}
```

**Rule:**
```json
{
  "ref": "github.create_issue_on_error",
  "pack_ref": "github",
  "trigger_ref": "core.error_event",
  "action_ref": "github.create_issue",
  "action_params": {
    "token": "{{ pack.config.token }}",
    "repo": "{{ pack.config.repo_owner }}/{{ pack.config.repo_name }}",
    "title": "[{{ trigger.payload.severity }}] {{ trigger.payload.service }}: {{ trigger.payload.error_message }}",
    "body": "Error Details:\n\nService: {{ trigger.payload.service }}\nSeverity: {{ trigger.payload.severity }}\n\nStack Trace:\n{{ trigger.payload.stack_trace }}",
    "labels": ["bug", "automated"],
    "assignees": ["oncall"]
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "token": "ghp_xxxxxxxxxxxx",
  "repo": "myorg/myrepo",
  "title": "[high] worker-pool: Memory leak detected",
  "body": "Error Details:\n\nService: worker-pool\nSeverity: high\n\nStack Trace:\nError at line 42...",
  "labels": ["bug", "automated"],
  "assignees": ["oncall"]
}
```

**When to use:** Complex integrations requiring both configuration and event data.

---

## Example 5: Nested Object Access

**Use Case:** Extract deeply nested values

**Event Payload:**
```json
{
  "user": {
    "id": 12345,
    "profile": {
      "name": "Alice Smith",
      "email": "alice@example.com",
      "department": "Engineering"
    }
  },
  "action": "login",
  "metadata": {
    "ip": "192.168.1.100",
    "user_agent": "Mozilla/5.0..."
  }
}
```

**Rule:**
```json
{
  "ref": "audit.log_user_action",
  "pack_ref": "audit",
  "trigger_ref": "core.user_event",
  "action_ref": "audit.log",
  "action_params": {
    "user_id": "{{ trigger.payload.user.id }}",
    "user_name": "{{ trigger.payload.user.profile.name }}",
    "user_email": "{{ trigger.payload.user.profile.email }}",
    "department": "{{ trigger.payload.user.profile.department }}",
    "action": "{{ trigger.payload.action }}",
    "ip_address": "{{ trigger.payload.metadata.ip }}"
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "user_id": 12345,
  "user_name": "Alice Smith",
  "user_email": "alice@example.com",
  "department": "Engineering",
  "action": "login",
  "ip_address": "192.168.1.100"
}
```

**When to use:** Complex event structures, deeply nested data.

---

## Example 6: Array Access

**Use Case:** Extract specific array elements

**Event Payload:**
```json
{
  "errors": [
    "Connection timeout",
    "Retry failed",
    "Circuit breaker open"
  ],
  "tags": ["production", "critical", "api-gateway"]
}
```

**Rule:**
```json
{
  "ref": "alerts.first_error",
  "pack_ref": "alerts",
  "trigger_ref": "core.error_event",
  "action_ref": "slack.post_message",
  "action_params": {
    "channel": "#alerts",
    "message": "Primary error: {{ trigger.payload.errors.0 }}",
    "secondary_error": "{{ trigger.payload.errors.1 }}",
    "environment": "{{ trigger.payload.tags.0 }}",
    "severity": "{{ trigger.payload.tags.1 }}"
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "channel": "#alerts",
  "message": "Primary error: Connection timeout",
  "secondary_error": "Retry failed",
  "environment": "production",
  "severity": "critical"
}
```

**When to use:** Event payloads with arrays where you need specific elements.

---

## Example 7: System Variables

**Use Case:** Include system-provided metadata

**Rule:**
```json
{
  "ref": "monitoring.heartbeat",
  "pack_ref": "monitoring",
  "trigger_ref": "core.timer_5m",
  "action_ref": "http.post",
  "action_params": {
    "url": "{{ pack.config.monitoring_url }}",
    "body": {
      "source": "attune",
      "timestamp": "{{ system.timestamp }}",
      "rule_id": "{{ system.rule.id }}",
      "rule_ref": "{{ system.rule.ref }}",
      "event_id": "{{ system.event.id }}",
      "status": "healthy"
    }
  },
  "enabled": true
}
```

**Resolved Parameters (example):**
```json
{
  "url": "https://monitoring.example.com/heartbeat",
  "body": {
    "source": "attune",
    "timestamp": "2026-01-17T15:30:00Z",
    "rule_id": 42,
    "rule_ref": "monitoring.heartbeat",
    "event_id": 123,
    "status": "healthy"
  }
}
```

**When to use:** Audit trails, logging, debugging, tracking execution context.

---

## Example 8: PagerDuty Integration

**Use Case:** Trigger PagerDuty incident from metrics

**Event Payload:**
```json
{
  "metric_name": "cpu_usage",
  "current_value": 95.3,
  "threshold": 80,
  "host": "web-server-01",
  "duration_seconds": 300
}
```

**Pack Config:**
```json
{
  "ref": "pagerduty",
  "config": {
    "routing_key": "R123ABC456DEF789",
    "default_severity": "error"
  }
}
```

**Rule:**
```json
{
  "ref": "pagerduty.critical_metric",
  "pack_ref": "pagerduty",
  "trigger_ref": "metrics.threshold_exceeded",
  "action_ref": "pagerduty.trigger_incident",
  "action_params": {
    "routing_key": "{{ pack.config.routing_key }}",
    "event_action": "trigger",
    "payload": {
      "summary": "{{ trigger.payload.metric_name }} exceeded threshold on {{ trigger.payload.host }}",
      "severity": "critical",
      "source": "{{ trigger.payload.host }}",
      "custom_details": {
        "metric": "{{ trigger.payload.metric_name }}",
        "current_value": "{{ trigger.payload.current_value }}",
        "threshold": "{{ trigger.payload.threshold }}",
        "duration": "{{ trigger.payload.duration_seconds }}s"
      }
    },
    "dedup_key": "{{ trigger.payload.host }}_{{ trigger.payload.metric_name }}"
  },
  "enabled": true
}
```

**Resolved Parameters:**
```json
{
  "routing_key": "R123ABC456DEF789",
  "event_action": "trigger",
  "payload": {
    "summary": "cpu_usage exceeded threshold on web-server-01",
    "severity": "critical",
    "source": "web-server-01",
    "custom_details": {
      "metric": "cpu_usage",
      "current_value": 95.3,
      "threshold": 80,
      "duration": "300s"
    }
  },
  "dedup_key": "web-server-01_cpu_usage"
}
```

**When to use:** Incident management, alerting, on-call notifications.

---

## Example 9: Webhook to Multiple Services

**Use Case:** Fan-out webhook data to multiple channels

**Event Payload:**
```json
{
  "event_type": "deployment",
  "service": "api-gateway",
  "version": "v2.3.1",
  "environment": "production",
  "deployed_by": "alice@example.com",
  "timestamp": "2026-01-17T15:30:00Z"
}
```

**Pack Config:**
```json
{
  "ref": "notifications",
  "config": {
    "slack_channel": "#deployments",
    "slack_token": "xoxb-...",
    "teams_webhook": "https://outlook.office.com/webhook/...",
    "email_recipients": ["team@example.com"]
  }
}
```

**Rule (Slack):**
```json
{
  "ref": "notifications.deployment_slack",
  "pack_ref": "notifications",
  "trigger_ref": "webhooks.deployment",
  "action_ref": "slack.post_message",
  "action_params": {
    "token": "{{ pack.config.slack_token }}",
    "channel": "{{ pack.config.slack_channel }}",
    "message": "✅ Deployment Complete",
    "attachments": [
      {
        "color": "good",
        "fields": [
          {
            "title": "Service",
            "value": "{{ trigger.payload.service }}",
            "short": true
          },
          {
            "title": "Version",
            "value": "{{ trigger.payload.version }}",
            "short": true
          },
          {
            "title": "Environment",
            "value": "{{ trigger.payload.environment }}",
            "short": true
          },
          {
            "title": "Deployed By",
            "value": "{{ trigger.payload.deployed_by }}",
            "short": true
          }
        ],
        "footer": "Attune Automation",
        "ts": "{{ trigger.payload.timestamp }}"
      }
    ]
  },
  "enabled": true
}
```

**When to use:** Multi-channel notifications, deployment tracking, audit trails.

---

## Example 10: Conditional Channels (Future with Filters)

**Use Case:** Route to different channels based on severity

**Event Payload:**
```json
{
  "severity": "critical",
  "message": "Database unreachable"
}
```

**Rule (Future Enhancement with Filters):**
```json
{
  "ref": "alerts.smart_routing",
  "pack_ref": "alerts",
  "trigger_ref": "core.alert_event",
  "action_ref": "slack.post_message",
  "action_params": {
    "channel": "{{ trigger.payload.severity | default: 'info' | map: {'critical': '#incidents', 'high': '#alerts', 'medium': '#monitoring', 'low': '#logs'} }}",
    "message": "{{ trigger.payload.message }}",
    "color": "{{ trigger.payload.severity | map: {'critical': 'danger', 'high': 'warning', 'medium': 'good', 'low': '#cccccc'} }}"
  },
  "enabled": true
}
```

**Note:** This uses advanced filter syntax not yet implemented (Phase 2).

---

## Testing Your Rules

### 1. Create a Test Event

```bash
curl -X POST http://localhost:8080/api/v1/events \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_ref": "core.error_event",
    "payload": {
      "service": "test-service",
      "error": "Test error message",
      "severity": "info"
    }
  }'
```

### 2. Check Enforcement

```bash
curl -X GET http://localhost:8080/api/v1/enforcements?limit=1 \
  -H "Authorization: Bearer $TOKEN"
```

Look at the `config` field to verify parameters were resolved correctly.

### 3. Check Execution

```bash
curl -X GET http://localhost:8080/api/v1/executions?limit=1 \
  -H "Authorization: Bearer $TOKEN"
```

The `config` field should contain the same resolved parameters.

---

## Common Patterns

### API Authentication
```json
{
  "action_params": {
    "url": "{{ pack.config.api_url }}",
    "headers": {
      "Authorization": "Bearer {{ pack.config.api_token }}",
      "Content-Type": "application/json"
    }
  }
}
```

### Error Context
```json
{
  "action_params": {
    "summary": "Error: {{ trigger.payload.message }}",
    "details": {
      "service": "{{ trigger.payload.service }}",
      "host": "{{ trigger.payload.host }}",
      "timestamp": "{{ trigger.payload.timestamp }}",
      "stack_trace": "{{ trigger.payload.stack_trace }}"
    }
  }
}
```

### User Information
```json
{
  "action_params": {
    "user": {
      "id": "{{ trigger.payload.user.id }}",
      "name": "{{ trigger.payload.user.name }}",
      "email": "{{ trigger.payload.user.email }}"
    },
    "action": "{{ trigger.payload.action_type }}"
  }
}
```

---

## Tips and Best Practices

1. **Use pack config for secrets** - Never hardcode API keys in rules
2. **Provide context** - Include relevant fields from event payload
3. **Keep templates simple** - Deeply nested access can be fragile
4. **Test with sample events** - Verify your templates work before production
5. **Use descriptive field names** - Make it clear what each parameter is for
6. **Document your rules** - Use clear labels and descriptions

---

## Related Documentation

- [Rule Parameter Mapping Guide](../rule-parameter-mapping.md) - Complete reference
- [Rule Management API](../api-rules.md) - API documentation
- [Pack Management](../api-packs.md) - Pack configuration

---

## Need Help?

If your templates aren't resolving correctly:

1. Check the event payload structure in the database
2. Verify the pack config exists and has the expected fields
3. Review sensor service logs for template resolution warnings
4. Test with a simple template first, then add complexity
5. Ensure field names match exactly (case-sensitive)