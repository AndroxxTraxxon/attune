//! Workflow Registrar
//!
//! This module handles registering workflows as workflow definitions in the database.
//! Workflows are stored in the `workflow_definition` table with their full YAML definition
//! as JSON. Optionally, actions can be created that reference workflow definitions.

use attune_common::error::{Error, Result};
use attune_common::repositories::workflow::{
    CreateWorkflowDefinitionInput, UpdateWorkflowDefinitionInput,
};
use attune_common::repositories::{
    Create, Delete, FindByRef, PackRepository, Update, WorkflowDefinitionRepository,
};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use super::loader::LoadedWorkflow;
use super::parser::WorkflowDefinition as WorkflowYaml;

/// Options for workflow registration
#[derive(Debug, Clone)]
pub struct RegistrationOptions {
    /// Whether to update existing workflows
    pub update_existing: bool,
    /// Whether to skip workflows with validation errors
    pub skip_invalid: bool,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            update_existing: true,
            skip_invalid: true,
        }
    }
}

/// Result of workflow registration
#[derive(Debug, Clone)]
pub struct RegistrationResult {
    /// Workflow reference name
    pub ref_name: String,
    /// Whether the workflow was created (false = updated)
    pub created: bool,
    /// Workflow definition ID
    pub workflow_def_id: i64,
    /// Any warnings during registration
    pub warnings: Vec<String>,
}

/// Workflow registrar for registering workflows in the database
pub struct WorkflowRegistrar {
    pool: PgPool,
    options: RegistrationOptions,
}

impl WorkflowRegistrar {
    /// Create a new workflow registrar
    pub fn new(pool: PgPool, options: RegistrationOptions) -> Self {
        Self { pool, options }
    }

    /// Register a single workflow
    pub async fn register_workflow(&self, loaded: &LoadedWorkflow) -> Result<RegistrationResult> {
        debug!("Registering workflow: {}", loaded.file.ref_name);

        // Check for validation errors
        if loaded.validation_error.is_some() {
            if self.options.skip_invalid {
                return Err(Error::validation(format!(
                    "Workflow has validation errors: {}",
                    loaded.validation_error.as_ref().unwrap()
                )));
            }
        }

        // Verify pack exists
        let pack = PackRepository::find_by_ref(&self.pool, &loaded.file.pack)
            .await?
            .ok_or_else(|| Error::not_found("pack", "ref", &loaded.file.pack))?;

        // Check if workflow already exists
        let existing_workflow =
            WorkflowDefinitionRepository::find_by_ref(&self.pool, &loaded.file.ref_name).await?;

        let mut warnings = Vec::new();

        // Add validation warning if present
        if let Some(ref err) = loaded.validation_error {
            warnings.push(err.clone());
        }

        let (workflow_def_id, created) = if let Some(existing) = existing_workflow {
            if !self.options.update_existing {
                return Err(Error::already_exists(
                    "workflow",
                    "ref",
                    &loaded.file.ref_name,
                ));
            }

            info!("Updating existing workflow: {}", loaded.file.ref_name);
            let workflow_def_id = self
                .update_workflow(&existing.id, &loaded.workflow, &pack.r#ref)
                .await?;
            (workflow_def_id, false)
        } else {
            info!("Creating new workflow: {}", loaded.file.ref_name);
            let workflow_def_id = self
                .create_workflow(&loaded.workflow, &loaded.file.pack, pack.id, &pack.r#ref)
                .await?;
            (workflow_def_id, true)
        };

        Ok(RegistrationResult {
            ref_name: loaded.file.ref_name.clone(),
            created,
            workflow_def_id,
            warnings,
        })
    }

    /// Register multiple workflows
    pub async fn register_workflows(
        &self,
        workflows: &HashMap<String, LoadedWorkflow>,
    ) -> Result<Vec<RegistrationResult>> {
        let mut results = Vec::new();
        let mut errors = Vec::new();

        for (ref_name, loaded) in workflows {
            match self.register_workflow(loaded).await {
                Ok(result) => {
                    info!("Registered workflow: {}", ref_name);
                    results.push(result);
                }
                Err(e) => {
                    warn!("Failed to register workflow '{}': {}", ref_name, e);
                    errors.push(format!("{}: {}", ref_name, e));
                }
            }
        }

        if !errors.is_empty() && results.is_empty() {
            return Err(Error::validation(format!(
                "Failed to register any workflows: {}",
                errors.join("; ")
            )));
        }

        Ok(results)
    }

    /// Unregister a workflow by reference
    pub async fn unregister_workflow(&self, ref_name: &str) -> Result<()> {
        debug!("Unregistering workflow: {}", ref_name);

        let workflow = WorkflowDefinitionRepository::find_by_ref(&self.pool, ref_name)
            .await?
            .ok_or_else(|| Error::not_found("workflow", "ref", ref_name))?;

        // Delete workflow definition (cascades to workflow_execution and related executions)
        WorkflowDefinitionRepository::delete(&self.pool, workflow.id).await?;

        info!("Unregistered workflow: {}", ref_name);
        Ok(())
    }

    /// Create a new workflow definition
    async fn create_workflow(
        &self,
        workflow: &WorkflowYaml,
        _pack_name: &str,
        pack_id: i64,
        pack_ref: &str,
    ) -> Result<i64> {
        // Convert the parsed workflow back to JSON for storage
        let definition = serde_json::to_value(workflow)
            .map_err(|e| Error::validation(format!("Failed to serialize workflow: {}", e)))?;

        let input = CreateWorkflowDefinitionInput {
            r#ref: workflow.r#ref.clone(),
            pack: pack_id,
            pack_ref: pack_ref.to_string(),
            label: workflow.label.clone(),
            description: workflow.description.clone(),
            version: workflow.version.clone(),
            param_schema: workflow.parameters.clone(),
            out_schema: workflow.output.clone(),
            definition: definition,
            tags: workflow.tags.clone(),
            enabled: true,
        };

        let created = WorkflowDefinitionRepository::create(&self.pool, input).await?;

        Ok(created.id)
    }

    /// Update an existing workflow definition
    async fn update_workflow(
        &self,
        workflow_id: &i64,
        workflow: &WorkflowYaml,
        _pack_ref: &str,
    ) -> Result<i64> {
        // Convert the parsed workflow back to JSON for storage
        let definition = serde_json::to_value(workflow)
            .map_err(|e| Error::validation(format!("Failed to serialize workflow: {}", e)))?;

        let input = UpdateWorkflowDefinitionInput {
            label: Some(workflow.label.clone()),
            description: workflow.description.clone(),
            version: Some(workflow.version.clone()),
            param_schema: workflow.parameters.clone(),
            out_schema: workflow.output.clone(),
            definition: Some(definition),
            tags: Some(workflow.tags.clone()),
            enabled: Some(true),
        };

        let updated = WorkflowDefinitionRepository::update(&self.pool, *workflow_id, input).await?;

        Ok(updated.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_options_default() {
        let options = RegistrationOptions::default();
        assert_eq!(options.update_existing, true);
        assert_eq!(options.skip_invalid, true);
    }

    #[test]
    fn test_registration_result_creation() {
        let result = RegistrationResult {
            ref_name: "test.workflow".to_string(),
            created: true,
            workflow_def_id: 123,
            warnings: vec![],
        };

        assert_eq!(result.ref_name, "test.workflow");
        assert_eq!(result.created, true);
        assert_eq!(result.workflow_def_id, 123);
        assert_eq!(result.warnings.len(), 0);
    }
}
