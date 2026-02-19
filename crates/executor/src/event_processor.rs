//! Event Processor - Handles EventCreated messages and creates enforcements
//!
//! This component listens for EventCreated messages from the message queue,
//! finds matching rules for the event's trigger, evaluates conditions, and
//! creates enforcement records for rules that match.

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use attune_common::{
    models::{EnforcementCondition, EnforcementStatus, Event, Rule},
    mq::{
        Consumer, EnforcementCreatedPayload, EventCreatedPayload, MessageEnvelope, MessageType,
        Publisher,
    },
    repositories::{
        event::{CreateEnforcementInput, EnforcementRepository, EventRepository},
        rule::RuleRepository,
        Create, FindById, List,
    },
    template_resolver::{resolve_templates, TemplateContext},
};

/// Event processor that handles event-to-rule matching
pub struct EventProcessor {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
}

impl EventProcessor {
    /// Create a new event processor
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, consumer: Arc<Consumer>) -> Self {
        Self {
            pool,
            publisher,
            consumer,
        }
    }

    /// Start processing EventCreated messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting event processor");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(move |envelope: MessageEnvelope<EventCreatedPayload>| {
                let pool = pool.clone();
                let publisher = publisher.clone();

                async move {
                    if let Err(e) = Self::process_event_created(&pool, &publisher, &envelope).await
                    {
                        error!("Error processing event: {}", e);
                        // Return error to trigger nack with requeue
                        return Err(format!("Failed to process event: {}", e).into());
                    }
                    Ok(())
                }
            })
            .await?;

        Ok(())
    }

    /// Process an EventCreated message
    async fn process_event_created(
        pool: &PgPool,
        publisher: &Publisher,
        envelope: &MessageEnvelope<EventCreatedPayload>,
    ) -> Result<()> {
        let payload = &envelope.payload;

        info!(
            "Processing EventCreated for event {} (trigger: {})",
            payload.event_id, payload.trigger_ref
        );

        // Fetch the event from database
        let event = EventRepository::find_by_id(pool, payload.event_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Event {} not found", payload.event_id))?;

        // Find matching rules for this trigger
        let matching_rules = Self::find_matching_rules(pool, &event).await?;

        if matching_rules.is_empty() {
            debug!(
                "No matching rules found for event {} (trigger: {})",
                event.id, event.trigger_ref
            );
            return Ok(());
        }

        info!(
            "Found {} matching rule(s) for event {}",
            matching_rules.len(),
            event.id
        );

        // Create enforcements for each matching rule
        for rule in matching_rules {
            if let Err(e) = Self::create_enforcement(pool, publisher, &rule, &event).await {
                error!(
                    "Failed to create enforcement for rule {} and event {}: {}",
                    rule.r#ref, event.id, e
                );
                // Continue with other rules even if one fails
            }
        }

        Ok(())
    }

    /// Find all enabled rules that match the event's trigger
    async fn find_matching_rules(pool: &PgPool, event: &Event) -> Result<Vec<Rule>> {
        // Check if event is associated with a specific rule
        if let Some(rule_id) = event.rule {
            // Event is for a specific rule - only match that rule
            info!(
                "Event {} is associated with specific rule ID: {}",
                event.id, rule_id
            );
            match RuleRepository::find_by_id(pool, rule_id).await? {
                Some(rule) => {
                    if rule.enabled {
                        Ok(vec![rule])
                    } else {
                        debug!("Rule {} is disabled, skipping", rule.r#ref);
                        Ok(vec![])
                    }
                }
                None => {
                    warn!(
                        "Event {} references non-existent rule {}",
                        event.id, rule_id
                    );
                    Ok(vec![])
                }
            }
        } else {
            // No specific rule - match all enabled rules for trigger
            let all_rules = RuleRepository::list(pool).await?;
            let matching_rules: Vec<Rule> = all_rules
                .into_iter()
                .filter(|r| r.enabled && r.trigger_ref == event.trigger_ref)
                .collect();

            Ok(matching_rules)
        }
    }

    /// Create an enforcement for a rule and event
    async fn create_enforcement(
        pool: &PgPool,
        publisher: &Publisher,
        rule: &Rule,
        event: &Event,
    ) -> Result<()> {
        // Evaluate rule conditions
        let conditions_pass = Self::evaluate_conditions(rule, event)?;

        if !conditions_pass {
            debug!(
                "Rule {} conditions did not match event {}",
                rule.r#ref, event.id
            );
            return Ok(());
        }

        info!(
            "Rule {} matched event {} - creating enforcement",
            rule.r#ref, event.id
        );

        // Prepare payload for enforcement
        let payload = event
            .payload
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        // Convert payload to dict if it's an object
        let payload_dict = payload
            .as_object()
            .cloned()
            .unwrap_or_else(|| serde_json::Map::new());

        // Resolve action parameters using the template resolver
        let resolved_params = Self::resolve_action_params(rule, event, &payload)?;

        let create_input = CreateEnforcementInput {
            rule: Some(rule.id),
            rule_ref: rule.r#ref.clone(),
            trigger_ref: rule.trigger_ref.clone(),
            config: Some(serde_json::Value::Object(resolved_params)),
            event: Some(event.id),
            status: EnforcementStatus::Created,
            payload: serde_json::Value::Object(payload_dict),
            condition: EnforcementCondition::All,
            conditions: rule.conditions.clone(),
        };

        let enforcement = EnforcementRepository::create(pool, create_input).await?;

        info!(
            "Enforcement {} created for rule {} (event: {})",
            enforcement.id, rule.r#ref, event.id
        );

        // Publish EnforcementCreated message
        let enforcement_payload = EnforcementCreatedPayload {
            enforcement_id: enforcement.id,
            rule_id: Some(rule.id),
            rule_ref: rule.r#ref.clone(),
            event_id: Some(event.id),
            trigger_ref: event.trigger_ref.clone(),
            payload: payload.clone(),
        };

        let envelope = MessageEnvelope::new(MessageType::EnforcementCreated, enforcement_payload)
            .with_source("event-processor");

        publisher.publish_envelope(&envelope).await?;

        debug!(
            "Published EnforcementCreated message for enforcement {}",
            enforcement.id
        );

        Ok(())
    }

    /// Evaluate rule conditions against event payload
    fn evaluate_conditions(rule: &Rule, event: &Event) -> Result<bool> {
        // If no payload, conditions cannot be evaluated (default to match)
        let payload = match &event.payload {
            Some(p) => p,
            None => {
                debug!("Event {} has no payload, matching by default", event.id);
                return Ok(true);
            }
        };

        // If rule has no conditions, it always matches
        if rule.conditions.is_null() || rule.conditions.as_array().map_or(true, |a| a.is_empty()) {
            debug!("Rule {} has no conditions, matching by default", rule.r#ref);
            return Ok(true);
        }

        // Parse conditions array
        let conditions = match rule.conditions.as_array() {
            Some(conds) => conds,
            None => {
                warn!("Rule {} conditions are not an array", rule.r#ref);
                return Ok(false);
            }
        };

        // Evaluate each condition (simplified - full evaluation logic would go here)
        let mut results = Vec::new();
        for condition in conditions {
            let result = Self::evaluate_single_condition(condition, payload)?;
            results.push(result);
        }

        // Apply logical operator (default to "all" = AND)
        let matches = results.iter().all(|&r| r);

        debug!(
            "Rule {} condition evaluation result: {} ({} condition(s))",
            rule.r#ref,
            matches,
            results.len()
        );

        Ok(matches)
    }

    /// Evaluate a single condition (simplified implementation)
    fn evaluate_single_condition(
        condition: &serde_json::Value,
        payload: &serde_json::Value,
    ) -> Result<bool> {
        // Expected condition format:
        // {
        //   "field": "payload.field_name",
        //   "operator": "equals",
        //   "value": "expected_value"
        // }

        let field = condition["field"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Condition missing 'field'"))?;

        let operator = condition["operator"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Condition missing 'operator'"))?;

        let expected_value = &condition["value"];

        // Extract field value from payload using dot notation
        let field_value = Self::extract_field_value(payload, field)?;

        // Apply operator
        let result = match operator {
            "equals" => field_value == expected_value,
            "not_equals" => field_value != expected_value,
            "contains" => {
                if let (Some(haystack), Some(needle)) =
                    (field_value.as_str(), expected_value.as_str())
                {
                    haystack.contains(needle)
                } else {
                    false
                }
            }
            _ => {
                warn!("Unknown operator '{}', defaulting to false", operator);
                false
            }
        };

        debug!(
            "Condition evaluation: field='{}', operator='{}', result={}",
            field, operator, result
        );

        Ok(result)
    }

    /// Extract field value from payload using dot notation
    fn extract_field_value<'a>(
        payload: &'a serde_json::Value,
        field: &str,
    ) -> Result<&'a serde_json::Value> {
        let mut current = payload;

        for part in field.split('.') {
            current = current
                .get(part)
                .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in payload", field))?;
        }

        Ok(current)
    }

    /// Resolve action parameters by applying template variable substitution.
    ///
    /// Replaces `{{ event.payload.* }}`, `{{ event.id }}`, `{{ event.trigger }}`,
    /// `{{ event.created }}`, `{{ pack.config.* }}`, and `{{ system.* }}` references
    /// in the rule's `action_params` with values from the event and context.
    fn resolve_action_params(
        rule: &Rule,
        event: &Event,
        event_payload: &serde_json::Value,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let action_params = &rule.action_params;

        // If there are no action params, return empty
        if action_params.is_null() || action_params.as_object().map_or(true, |o| o.is_empty()) {
            return Ok(serde_json::Map::new());
        }

        // Build template context from the event
        let context = TemplateContext::new(
            event_payload.clone(),
            // TODO: Load pack config from database for pack.config.* resolution
            serde_json::json!({}),
            serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "rule": {
                    "id": rule.id,
                    "ref": rule.r#ref,
                },
            }),
        )
        .with_event_id(event.id)
        .with_event_trigger(&event.trigger_ref)
        .with_event_created(&event.created.to_rfc3339());

        let resolved = resolve_templates(action_params, &context)?;

        if let Some(obj) = resolved.as_object() {
            Ok(obj.clone())
        } else {
            Ok(serde_json::Map::new())
        }
    }
}
