//! Shared work queue definition parsing and validation helpers.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    models::{
        WorkQueueBatchMode, WorkQueueConfig, WorkQueueTunableSource, WorkQueueTunableValue,
        WorkQueueUpdateStrategy,
    },
    schema::RefValidator,
    Error, Result,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkQueueDefinition {
    #[serde(rename = "ref")]
    pub r#ref: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub dispatch_action: String,
    #[serde(default)]
    pub default_priority: i32,
    #[serde(default)]
    pub allow_pending_update: bool,
    #[serde(default)]
    pub update_strategy: WorkQueueUpdateStrategy,
    #[serde(default)]
    pub batch_mode: WorkQueueBatchMode,
    #[serde(default = "default_config")]
    pub config: JsonValue,
}

fn default_true() -> bool {
    true
}

fn default_config() -> JsonValue {
    serde_json::json!({})
}

pub fn parse_work_queue_definition_yaml(content: &str) -> Result<WorkQueueDefinition> {
    let definition: WorkQueueDefinition = serde_yaml_ng::from_str(content).map_err(|e| {
        Error::validation(format!("Failed to parse work queue YAML definition: {}", e))
    })?;
    validate_work_queue_definition(&definition)?;
    Ok(definition)
}

pub fn validate_work_queue_definition(definition: &WorkQueueDefinition) -> Result<WorkQueueConfig> {
    RefValidator::validate_work_queue_ref(&definition.r#ref)?;
    RefValidator::validate_component_ref(&definition.dispatch_action)?;

    if definition.label.trim().is_empty() {
        return Err(Error::validation("Work queue label cannot be empty"));
    }

    if definition
        .description
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(Error::validation(
            "Work queue description cannot be an empty string",
        ));
    }

    validate_work_queue_config(&definition.config)
}

pub fn validate_work_queue_config(config: &JsonValue) -> Result<WorkQueueConfig> {
    let config: WorkQueueConfig = serde_json::from_value(config.clone())
        .map_err(|e| Error::validation(format!("Invalid work queue config structure: {}", e)))?;

    if let Some(mapping) = &config.input_mapping {
        if mapping
            .items_path
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(Error::validation(
                "config.input_mapping.items_path cannot be empty",
            ));
        }

        if mapping
            .single_item_path
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(Error::validation(
                "config.input_mapping.single_item_path cannot be empty",
            ));
        }
    }

    if let Some(priority) = &config.priority {
        if let Some(default) = &priority.default {
            validate_tunable_value("config.priority.default", default)?;
        }
    }

    if let Some(dispatch) = &config.dispatch {
        if let Some(concurrency) = &dispatch.concurrency {
            validate_tunable_value("config.dispatch.concurrency", concurrency)?;
        }
        if let Some(batch_size) = &dispatch.batch_size {
            validate_tunable_value("config.dispatch.batch_size", batch_size)?;
        }
    }

    if let Some(ack_contract) = &config.ack_contract {
        if ack_contract.version < 1 {
            return Err(Error::validation(
                "config.ack_contract.version must be >= 1",
            ));
        }
    }

    Ok(config)
}

fn validate_tunable_value(path: &str, value: &WorkQueueTunableValue) -> Result<()> {
    match value.source {
        WorkQueueTunableSource::Literal => {
            if value.value.is_none() {
                return Err(Error::validation(format!(
                    "{path} requires a literal value"
                )));
            }
            if value.path.is_some() {
                return Err(Error::validation(format!(
                    "{path} cannot set 'path' when source=literal"
                )));
            }
            if value.key_ref.is_some() {
                return Err(Error::validation(format!(
                    "{path} cannot set 'key_ref' when source=literal"
                )));
            }
        }
        WorkQueueTunableSource::PackConfig => {
            if value.value.is_some() {
                return Err(Error::validation(format!(
                    "{path} cannot set 'value' when source=pack_config"
                )));
            }
            if value.key_ref.is_some() {
                return Err(Error::validation(format!(
                    "{path} cannot set 'key_ref' when source=pack_config"
                )));
            }
            if value
                .path
                .as_ref()
                .is_none_or(|path| path.trim().is_empty())
            {
                return Err(Error::validation(format!(
                    "{path} requires a non-empty 'path' when source=pack_config"
                )));
            }
        }
        WorkQueueTunableSource::Keystore => {
            if value.value.is_some() {
                return Err(Error::validation(format!(
                    "{path} cannot set 'value' when source=keystore"
                )));
            }
            if value
                .key_ref
                .as_ref()
                .is_none_or(|key_ref| key_ref.trim().is_empty())
            {
                return Err(Error::validation(format!(
                    "{path} requires a non-empty 'key_ref' when source=keystore"
                )));
            }
            if value
                .path
                .as_ref()
                .is_some_and(|path| path.trim().is_empty())
            {
                return Err(Error::validation(format!(
                    "{path}.path cannot be an empty string"
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_work_queue_definition_yaml, validate_work_queue_config};
    use crate::models::{WorkQueueBatchMode, WorkQueueUpdateStrategy};

    #[test]
    fn parses_valid_work_queue_definition() {
        let definition = parse_work_queue_definition_yaml(
            r#"
ref: core.inbox
label: Core Inbox
dispatch_action: core.process_item
allow_pending_update: true
update_strategy: merge_patch
batch_mode: batch
config:
  dispatch:
    concurrency:
      source: literal
      value: 5
"#,
        )
        .expect("queue definition should parse");

        assert_eq!(definition.r#ref, "core.inbox");
        assert_eq!(definition.dispatch_action, "core.process_item");
        assert_eq!(definition.batch_mode, WorkQueueBatchMode::Batch);
        assert_eq!(
            definition.update_strategy,
            WorkQueueUpdateStrategy::MergePatch
        );
    }

    #[test]
    fn rejects_invalid_tunable_config() {
        let error = validate_work_queue_config(&json!({
            "dispatch": {
                "concurrency": {
                    "source": "pack_config"
                }
            }
        }))
        .expect_err("config should be rejected");

        assert!(error.to_string().contains("config.dispatch.concurrency"));
    }
}
