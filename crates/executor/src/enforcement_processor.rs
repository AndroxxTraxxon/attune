//! Enforcement Processor - Handles enforcement creation and processing
//!
//! This module is responsible for:
//! - Listening for EnforcementCreated messages
//! - Evaluating rule conditions and context
//! - Determining whether to create executions
//! - Applying execution policies (via PolicyEnforcer + QueueManager)
//! - Waiting for queue slot if concurrency limited
//! - Creating execution records
//! - Publishing ExecutionRequested messages

use anyhow::{bail, Result};
use attune_common::{
    models::{Enforcement, EnforcementStatus, Event, Rule},
    mq::{
        Consumer, EnforcementCreatedPayload, ExecutionRequestedPayload, MessageEnvelope, Publisher,
    },
    repositories::{
        event::{EnforcementRepository, EventRepository, UpdateEnforcementInput},
        execution::{CreateExecutionInput, ExecutionRepository},
        rule::RuleRepository,
        Create, FindById, Update,
    },
};

use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::policy_enforcer::PolicyEnforcer;
use crate::queue_manager::ExecutionQueueManager;

/// Enforcement processor that handles enforcement messages
pub struct EnforcementProcessor {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
    policy_enforcer: Arc<PolicyEnforcer>,
    queue_manager: Arc<ExecutionQueueManager>,
}

impl EnforcementProcessor {
    /// Create a new enforcement processor
    pub fn new(
        pool: PgPool,
        publisher: Arc<Publisher>,
        consumer: Arc<Consumer>,
        policy_enforcer: Arc<PolicyEnforcer>,
        queue_manager: Arc<ExecutionQueueManager>,
    ) -> Self {
        Self {
            pool,
            publisher,
            consumer,
            policy_enforcer,
            queue_manager,
        }
    }

    /// Start processing enforcement messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting enforcement processor");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();
        let policy_enforcer = self.policy_enforcer.clone();
        let queue_manager = self.queue_manager.clone();

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(
                move |envelope: MessageEnvelope<EnforcementCreatedPayload>| {
                    let pool = pool.clone();
                    let publisher = publisher.clone();
                    let policy_enforcer = policy_enforcer.clone();
                    let queue_manager = queue_manager.clone();

                    async move {
                        if let Err(e) = Self::process_enforcement_created(
                            &pool,
                            &publisher,
                            &policy_enforcer,
                            &queue_manager,
                            &envelope,
                        )
                        .await
                        {
                            error!("Error processing enforcement: {}", e);
                            // Return error to trigger nack with requeue
                            return Err(format!("Failed to process enforcement: {}", e).into());
                        }
                        Ok(())
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Process an enforcement created message
    async fn process_enforcement_created(
        pool: &PgPool,
        publisher: &Publisher,
        policy_enforcer: &PolicyEnforcer,
        queue_manager: &ExecutionQueueManager,
        envelope: &MessageEnvelope<EnforcementCreatedPayload>,
    ) -> Result<()> {
        debug!("Processing enforcement message: {:?}", envelope);

        let enforcement_id = envelope.payload.enforcement_id;
        info!("Processing enforcement: {}", enforcement_id);

        // Fetch enforcement from database
        let enforcement = EnforcementRepository::find_by_id(pool, enforcement_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Enforcement not found: {}", enforcement_id))?;

        // Fetch associated rule
        let rule = RuleRepository::find_by_id(
            pool,
            enforcement.rule.ok_or_else(|| {
                anyhow::anyhow!("Enforcement {} has no associated rule", enforcement_id)
            })?,
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("Rule not found for enforcement: {}", enforcement_id))?;

        // Fetch associated event if present
        let event = if let Some(event_id) = enforcement.event {
            EventRepository::find_by_id(pool, event_id).await?
        } else {
            None
        };

        // Evaluate whether to create execution
        if Self::should_create_execution(&enforcement, &rule, event.as_ref())? {
            Self::create_execution(
                pool,
                publisher,
                policy_enforcer,
                queue_manager,
                &enforcement,
                &rule,
            )
            .await?;

            // Update enforcement status to Processed after successful execution creation
            EnforcementRepository::update(
                pool,
                enforcement_id,
                UpdateEnforcementInput {
                    status: Some(EnforcementStatus::Processed),
                    payload: None,
                    resolved_at: Some(chrono::Utc::now()),
                },
            )
            .await?;

            debug!("Updated enforcement {} status to Processed", enforcement_id);
        } else {
            info!(
                "Skipping execution creation for enforcement: {}",
                enforcement_id
            );

            // Update enforcement status to Disabled since it was not actionable
            EnforcementRepository::update(
                pool,
                enforcement_id,
                UpdateEnforcementInput {
                    status: Some(EnforcementStatus::Disabled),
                    payload: None,
                    resolved_at: Some(chrono::Utc::now()),
                },
            )
            .await?;

            debug!(
                "Updated enforcement {} status to Disabled (skipped)",
                enforcement_id
            );
        }

        Ok(())
    }

    /// Determine if an execution should be created for this enforcement
    fn should_create_execution(
        enforcement: &Enforcement,
        rule: &Rule,
        _event: Option<&Event>,
    ) -> Result<bool> {
        // Check if rule is enabled
        if !rule.enabled {
            warn!("Rule {} is disabled, skipping execution", rule.id);
            return Ok(false);
        }

        // Check if the rule's action still exists (may have been deleted with its pack)
        if rule.action.is_none() {
            warn!(
                "Rule {} references a deleted action (action_ref: {}), skipping execution",
                rule.id, rule.action_ref
            );
            return Ok(false);
        }

        // Check if the rule's trigger still exists
        if rule.trigger.is_none() {
            warn!(
                "Rule {} references a deleted trigger (trigger_ref: {}), skipping execution",
                rule.id, rule.trigger_ref
            );
            return Ok(false);
        }

        // TODO: Evaluate rule conditions against event payload
        // For now, we'll create executions for all valid enforcements

        debug!(
            "Enforcement {} passed validation, will create execution",
            enforcement.id
        );

        Ok(true)
    }

    /// Create an execution record for the enforcement
    async fn create_execution(
        pool: &PgPool,
        publisher: &Publisher,
        policy_enforcer: &PolicyEnforcer,
        _queue_manager: &ExecutionQueueManager,
        enforcement: &Enforcement,
        rule: &Rule,
    ) -> Result<()> {
        // Extract action ID — should_create_execution already verified it's Some,
        // but guard defensively here as well.
        let action_id = match rule.action {
            Some(id) => id,
            None => {
                error!(
                    "Rule {} has no action ID (deleted?), cannot create execution for enforcement {}",
                    rule.id, enforcement.id
                );
                bail!(
                    "Rule {} references a deleted action (action_ref: {})",
                    rule.id,
                    rule.action_ref
                );
            }
        };

        info!(
            "Creating execution for enforcement: {}, rule: {}, action: {}",
            enforcement.id, rule.id, action_id
        );

        let pack_id = rule.pack;
        let action_ref = &rule.action_ref;

        // Enforce policies and wait for queue slot if needed
        info!(
            "Enforcing policies for action {} (enforcement: {})",
            action_id, enforcement.id
        );

        // Use enforcement ID for queue tracking (execution doesn't exist yet)
        if let Err(e) = policy_enforcer
            .enforce_and_wait(action_id, Some(pack_id), enforcement.id)
            .await
        {
            error!(
                "Policy enforcement failed for enforcement {}: {}",
                enforcement.id, e
            );
            return Err(e);
        }

        info!(
            "Policy check passed and queue slot obtained for enforcement: {}",
            enforcement.id
        );

        // Now create execution in database (we have a queue slot)
        let execution_input = CreateExecutionInput {
            action: Some(action_id),
            action_ref: action_ref.clone(),
            config: enforcement.config.clone(),
            env_vars: None, // No custom env vars for rule-triggered executions
            parent: None,   // TODO: Handle workflow parent-child relationships
            enforcement: Some(enforcement.id),
            executor: None, // Will be assigned during scheduling
            worker: None,
            status: attune_common::models::enums::ExecutionStatus::Requested,
            result: None,
            workflow_task: None, // Non-workflow execution
        };

        let execution = ExecutionRepository::create(pool, execution_input).await?;

        info!(
            "Created execution: {} for enforcement: {}",
            execution.id, enforcement.id
        );

        // Publish ExecutionRequested message
        let payload = ExecutionRequestedPayload {
            execution_id: execution.id,
            action_id: Some(action_id),
            action_ref: action_ref.clone(),
            parent_id: None,
            enforcement_id: Some(enforcement.id),
            config: enforcement.config.clone(),
        };

        let envelope =
            MessageEnvelope::new(attune_common::mq::MessageType::ExecutionRequested, payload)
                .with_source("executor");

        // Publish to execution requests queue with routing key
        let routing_key = "execution.requested";
        let exchange = "attune.executions";

        publisher
            .publish_envelope_with_routing(&envelope, exchange, routing_key)
            .await?;

        info!(
            "Published execution.requested message for execution: {} (enforcement: {}, action: {})",
            execution.id, enforcement.id, action_id
        );

        // NOTE: Queue slot will be released when worker publishes execution.completed
        // and CompletionListener calls queue_manager.notify_completion(action_id)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_create_execution_disabled_rule() {
        use serde_json::json;

        let enforcement = Enforcement {
            id: 1,
            rule: Some(1),
            rule_ref: "test.rule".to_string(),
            trigger_ref: "test.trigger".to_string(),
            event: Some(1),
            config: None,
            status: attune_common::models::enums::EnforcementStatus::Processed,
            payload: json!({}),
            condition: attune_common::models::enums::EnforcementCondition::Any,
            conditions: json!({}),
            created: chrono::Utc::now(),
            resolved_at: Some(chrono::Utc::now()),
        };

        let mut rule = Rule {
            id: 1,
            r#ref: "test.rule".to_string(),
            pack: 1,
            pack_ref: "test".to_string(),
            label: "Test Rule".to_string(),
            description: "Test rule description".to_string(),
            trigger_ref: "test.trigger".to_string(),
            trigger: Some(1),
            action_ref: "test.action".to_string(),
            action: Some(1),
            enabled: false, // Disabled
            conditions: json!({}),
            action_params: json!({}),
            trigger_params: json!({}),
            is_adhoc: false,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

        let result = EnforcementProcessor::should_create_execution(&enforcement, &rule, None);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should not create execution

        // Test with enabled rule
        rule.enabled = true;
        let result = EnforcementProcessor::should_create_execution(&enforcement, &rule, None);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should create execution
    }
}
