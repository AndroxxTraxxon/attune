//! Data models for Attune services
//!
//! This module contains the data models that map to the database schema.
//! Models are organized by functional area and use SQLx for database operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

// Re-export common types
pub use action::*;
pub use artifact::Artifact;
pub use artifact_version::ArtifactVersion;
pub use entity_history::*;
pub use enums::*;
pub use event::*;
pub use execution::*;
pub use identity::*;
pub use inquiry::*;
pub use key::*;
pub use notification::*;
pub use pack::*;
pub use pack_test::*;
pub use rule::*;
pub use runtime::*;
pub use trigger::*;
pub use workflow::*;

/// Common ID type used throughout the system
pub type Id = i64;

/// JSON dictionary type
pub type JsonDict = JsonValue;

/// JSON schema type
pub type JsonSchema = JsonValue;

/// Enumeration types
pub mod enums {
    use serde::{Deserialize, Serialize};
    use sqlx::Type;
    use std::fmt;
    use std::str::FromStr;
    use utoipa::ToSchema;

    /// How parameters should be delivered to an action
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum ParameterDelivery {
        /// Pass parameters via stdin (secure, recommended for most cases)
        Stdin,
        /// Pass parameters via temporary file (secure, best for large payloads)
        File,
    }

    impl Default for ParameterDelivery {
        fn default() -> Self {
            Self::Stdin
        }
    }

    impl fmt::Display for ParameterDelivery {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Stdin => write!(f, "stdin"),
                Self::File => write!(f, "file"),
            }
        }
    }

    impl FromStr for ParameterDelivery {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "stdin" => Ok(Self::Stdin),
                "file" => Ok(Self::File),
                _ => Err(format!("Invalid parameter delivery method: {}", s)),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for ParameterDelivery {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            <String as sqlx::Type<sqlx::Postgres>>::type_info()
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ParameterDelivery {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            s.parse().map_err(|e: String| e.into())
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ParameterDelivery {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
            Ok(<String as sqlx::Encode<sqlx::Postgres>>::encode(
                self.to_string(),
                buf,
            )?)
        }
    }

    /// Format for parameter serialization
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum ParameterFormat {
        /// KEY='VALUE' format (one per line)
        Dotenv,
        /// JSON object
        Json,
        /// YAML format
        Yaml,
    }

    impl Default for ParameterFormat {
        fn default() -> Self {
            Self::Json
        }
    }

    impl fmt::Display for ParameterFormat {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Json => write!(f, "json"),
                Self::Dotenv => write!(f, "dotenv"),
                Self::Yaml => write!(f, "yaml"),
            }
        }
    }

    impl FromStr for ParameterFormat {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "json" => Ok(Self::Json),
                "dotenv" => Ok(Self::Dotenv),
                "yaml" => Ok(Self::Yaml),
                _ => Err(format!("Invalid parameter format: {}", s)),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for ParameterFormat {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            <String as sqlx::Type<sqlx::Postgres>>::type_info()
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ParameterFormat {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            s.parse().map_err(|e: String| e.into())
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ParameterFormat {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
            Ok(<String as sqlx::Encode<sqlx::Postgres>>::encode(
                self.to_string(),
                buf,
            )?)
        }
    }

    /// Format for action output parsing
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum OutputFormat {
        /// Plain text (no parsing)
        Text,
        /// Parse as JSON
        Json,
        /// Parse as YAML
        Yaml,
        /// Parse as JSON Lines (each line is a separate JSON object/value)
        Jsonl,
    }

    impl Default for OutputFormat {
        fn default() -> Self {
            Self::Text
        }
    }

    impl fmt::Display for OutputFormat {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Text => write!(f, "text"),
                Self::Json => write!(f, "json"),
                Self::Yaml => write!(f, "yaml"),
                Self::Jsonl => write!(f, "jsonl"),
            }
        }
    }

    impl FromStr for OutputFormat {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "text" => Ok(Self::Text),
                "json" => Ok(Self::Json),
                "yaml" => Ok(Self::Yaml),
                "jsonl" => Ok(Self::Jsonl),
                _ => Err(format!("Invalid output format: {}", s)),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for OutputFormat {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            <String as sqlx::Type<sqlx::Postgres>>::type_info()
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for OutputFormat {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            s.parse().map_err(|e: String| e.into())
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for OutputFormat {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
            Ok(<String as sqlx::Encode<sqlx::Postgres>>::encode(
                self.to_string(),
                buf,
            )?)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_type_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerType {
        Local,
        Remote,
        Container,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerStatus {
        Active,
        Inactive,
        Busy,
        Error,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_role_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerRole {
        Action,
        Sensor,
        Hybrid,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "enforcement_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum EnforcementStatus {
        Created,
        Processed,
        Disabled,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "enforcement_condition_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum EnforcementCondition {
        Any,
        All,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "execution_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum ExecutionStatus {
        Requested,
        Scheduling,
        Scheduled,
        Running,
        Completed,
        Failed,
        Canceling,
        Cancelled,
        Timeout,
        Abandoned,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "inquiry_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum InquiryStatus {
        Pending,
        Responded,
        Timeout,
        Cancelled,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "policy_method_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum PolicyMethod {
        Cancel,
        Enqueue,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "owner_type_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum OwnerType {
        System,
        Identity,
        Pack,
        Action,
        Sensor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "notification_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum NotificationState {
        Created,
        Queued,
        Processing,
        Error,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "artifact_type_enum", rename_all = "snake_case")]
    #[serde(rename_all = "snake_case")]
    pub enum ArtifactType {
        FileBinary,
        #[serde(rename = "file_datatable")]
        #[sqlx(rename = "file_datatable")]
        FileDataTable,
        FileImage,
        FileText,
        Other,
        Progress,
        Url,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "artifact_retention_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum RetentionPolicyType {
        Versions,
        Days,
        Hours,
        Minutes,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "workflow_task_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkflowTaskStatus {
        Pending,
        Running,
        Completed,
        Failed,
        Skipped,
        Cancelled,
    }
}

/// Pack model
pub mod pack {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Pack {
        pub id: Id,
        pub r#ref: String,
        pub label: String,
        pub description: Option<String>,
        pub version: String,
        pub conf_schema: JsonSchema,
        pub config: JsonDict,
        pub meta: JsonDict,
        pub tags: Vec<String>,
        pub runtime_deps: Vec<String>,
        pub dependencies: Vec<String>,
        pub is_standard: bool,
        pub installers: JsonDict,
        // Installation metadata (nullable for non-installed packs)
        pub source_type: Option<String>,
        pub source_url: Option<String>,
        pub source_ref: Option<String>,
        pub checksum: Option<String>,
        pub checksum_verified: Option<bool>,
        pub installed_at: Option<DateTime<Utc>>,
        pub installed_by: Option<Id>,
        pub installation_method: Option<String>,
        pub storage_path: Option<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Runtime model
pub mod runtime {
    use super::*;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use tracing::{debug, warn};

    /// Configuration for how a runtime executes actions.
    ///
    /// Stored as JSONB in the `runtime.execution_config` column.
    /// Uses template variables that are resolved at execution time:
    /// - `{pack_dir}` — absolute path to the pack directory
    /// - `{env_dir}` — resolved environment directory
    ///   When an external `env_dir` is provided (e.g., from `runtime_envs_dir`
    ///   config), that path is used directly. Otherwise falls back to
    ///   `pack_dir/dir_name` for backward compatibility.
    /// - `{interpreter}` — resolved interpreter path
    /// - `{action_file}` — absolute path to the action script file
    /// - `{manifest_path}` — absolute path to the dependency manifest file
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct RuntimeExecutionConfig {
        /// Interpreter configuration (how to invoke the action script)
        #[serde(default)]
        pub interpreter: InterpreterConfig,

        /// Optional isolated environment configuration (venv, node_modules, etc.)
        #[serde(default)]
        pub environment: Option<EnvironmentConfig>,

        /// Optional dependency management configuration
        #[serde(default)]
        pub dependencies: Option<DependencyConfig>,

        /// Optional environment variables to set during action execution.
        ///
        /// Values support the same template variables as other fields:
        /// `{pack_dir}`, `{env_dir}`, `{interpreter}`, `{manifest_path}`.
        ///
        /// Example: `{"NODE_PATH": "{env_dir}/node_modules"}` ensures Node.js
        /// can find packages installed in the isolated runtime environment.
        #[serde(default)]
        pub env_vars: HashMap<String, String>,
    }

    /// Describes the interpreter binary and how it invokes action scripts.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct InterpreterConfig {
        /// Path or name of the interpreter binary (e.g., "python3", "/bin/bash").
        #[serde(default = "default_interpreter_binary")]
        pub binary: String,

        /// Additional arguments inserted before the action file path
        /// (e.g., `["-u"]` for unbuffered Python output).
        #[serde(default)]
        pub args: Vec<String>,

        /// File extension this runtime handles (e.g., ".py", ".sh").
        /// Used to match actions to runtimes when runtime_name is not explicit.
        #[serde(default)]
        pub file_extension: Option<String>,
    }

    fn default_interpreter_binary() -> String {
        String::new()
    }

    impl Default for InterpreterConfig {
        fn default() -> Self {
            Self {
                binary: default_interpreter_binary(),
                args: Vec::new(),
                file_extension: None,
            }
        }
    }

    /// Describes how to create and manage an isolated runtime environment
    /// (e.g., Python virtualenv, Node.js node_modules).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EnvironmentConfig {
        /// Type of environment: "virtualenv", "node_modules", "none".
        pub env_type: String,

        /// Fallback directory name relative to the pack directory (e.g., ".venv").
        /// Only used when no external `env_dir` is provided (legacy/bare-metal).
        /// In production, the env_dir is computed externally as
        /// `{runtime_envs_dir}/{pack_ref}/{runtime_name}`.
        #[serde(default = "super::runtime::default_env_dir_name")]
        pub dir_name: String,

        /// Command(s) to create the environment.
        /// Template variables: `{env_dir}`, `{pack_dir}`.
        /// Example: `["python3", "-m", "venv", "{env_dir}"]`
        #[serde(default)]
        pub create_command: Vec<String>,

        /// Path to the interpreter inside the environment.
        /// When the environment exists, this overrides `interpreter.binary`.
        /// Template variables: `{env_dir}`.
        /// Example: `"{env_dir}/bin/python3"`
        pub interpreter_path: Option<String>,
    }

    /// Describes how to detect and install dependencies for a pack.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DependencyConfig {
        /// Name of the manifest file to look for in the pack directory
        /// (e.g., "requirements.txt", "package.json").
        pub manifest_file: String,

        /// Command to install dependencies.
        /// Template variables: `{interpreter}`, `{env_dir}`, `{manifest_path}`, `{pack_dir}`.
        /// Example: `["{interpreter}", "-m", "pip", "install", "-r", "{manifest_path}"]`
        #[serde(default)]
        pub install_command: Vec<String>,
    }

    fn default_env_dir_name() -> String {
        ".venv".to_string()
    }

    impl RuntimeExecutionConfig {
        /// Resolve template variables in a single string.
        pub fn resolve_template(template: &str, vars: &HashMap<&str, String>) -> String {
            let mut result = template.to_string();
            for (key, value) in vars {
                result = result.replace(&format!("{{{}}}", key), value);
            }
            result
        }

        /// Resolve the interpreter binary path using a pack-relative env_dir
        /// (legacy fallback — prefers [`resolve_interpreter_with_env`]).
        pub fn resolve_interpreter(&self, pack_dir: &Path) -> PathBuf {
            let fallback_env_dir = self
                .environment
                .as_ref()
                .map(|cfg| pack_dir.join(&cfg.dir_name));
            self.resolve_interpreter_with_env(pack_dir, fallback_env_dir.as_deref())
        }

        /// Resolve the interpreter binary path for a given pack directory and
        /// an explicit environment directory.
        ///
        /// If `env_dir` is provided and exists on disk, returns the
        /// environment's interpreter. Otherwise returns the system interpreter.
        pub fn resolve_interpreter_with_env(
            &self,
            pack_dir: &Path,
            env_dir: Option<&Path>,
        ) -> PathBuf {
            if let Some(ref env_cfg) = self.environment {
                if let Some(ref interp_path_template) = env_cfg.interpreter_path {
                    if let Some(env_dir) = env_dir {
                        if env_dir.exists() {
                            let mut vars = HashMap::new();
                            vars.insert("env_dir", env_dir.to_string_lossy().to_string());
                            vars.insert("pack_dir", pack_dir.to_string_lossy().to_string());
                            let resolved = Self::resolve_template(interp_path_template, &vars);
                            let resolved_path = PathBuf::from(&resolved);
                            // Path::exists() follows symlinks — returns true only
                            // if the final target is reachable. A valid symlink to
                            // an existing executable passes this check just fine.
                            if resolved_path.exists() {
                                debug!(
                                    "Using environment interpreter: {} (template: '{}', env_dir: {})",
                                    resolved_path.display(),
                                    interp_path_template,
                                    env_dir.display(),
                                );
                                return resolved_path;
                            }
                            // exists() returned false — check whether the path is
                            // a broken symlink (symlink_metadata succeeds for the
                            // link itself even when its target is missing).
                            let is_broken_symlink = std::fs::symlink_metadata(&resolved_path)
                                .map(|m| m.file_type().is_symlink())
                                .unwrap_or(false);
                            if is_broken_symlink {
                                // Read the dangling target for the diagnostic
                                let target = std::fs::read_link(&resolved_path)
                                    .map(|t| t.display().to_string())
                                    .unwrap_or_else(|_| "<unreadable>".to_string());
                                warn!(
                                    "Environment interpreter at '{}' is a broken symlink \
                                     (target '{}' does not exist). This typically happens \
                                     when the venv was created by a different container \
                                     where python3 lives at a different path. \
                                     Recreate the venv with `--copies` or delete '{}' \
                                     and restart the worker. \
                                     Falling back to system interpreter '{}'",
                                    resolved_path.display(),
                                    target,
                                    env_dir.display(),
                                    self.interpreter.binary,
                                );
                            } else {
                                warn!(
                                    "Environment interpreter not found at resolved path '{}' \
                                     (template: '{}', env_dir: {}). \
                                     Falling back to system interpreter '{}'",
                                    resolved_path.display(),
                                    interp_path_template,
                                    env_dir.display(),
                                    self.interpreter.binary,
                                );
                            }
                        } else {
                            warn!(
                                "Environment directory does not exist: {}. \
                                 Expected interpreter template '{}' cannot be resolved. \
                                 Falling back to system interpreter '{}'",
                                env_dir.display(),
                                interp_path_template,
                                self.interpreter.binary,
                            );
                        }
                    } else {
                        debug!(
                            "No env_dir provided; skipping environment interpreter resolution. \
                             Using system interpreter '{}'",
                            self.interpreter.binary,
                        );
                    }
                } else {
                    debug!(
                        "No interpreter_path configured in environment config. \
                         Using system interpreter '{}'",
                        self.interpreter.binary,
                    );
                }
            } else {
                debug!(
                    "No environment config present. Using system interpreter '{}'",
                    self.interpreter.binary,
                );
            }
            PathBuf::from(&self.interpreter.binary)
        }

        /// Resolve the working directory for action execution.
        /// Returns the pack directory.
        pub fn resolve_working_dir(&self, pack_dir: &Path) -> PathBuf {
            pack_dir.to_path_buf()
        }

        /// Resolve the environment directory for a pack (legacy pack-relative
        /// fallback — callers should prefer computing `env_dir` externally
        /// from `runtime_envs_dir`).
        pub fn resolve_env_dir(&self, pack_dir: &Path) -> Option<PathBuf> {
            self.environment
                .as_ref()
                .map(|env_cfg| pack_dir.join(&env_cfg.dir_name))
        }

        /// Check whether the pack directory has a dependency manifest file.
        pub fn has_dependencies(&self, pack_dir: &Path) -> bool {
            if let Some(ref dep_cfg) = self.dependencies {
                pack_dir.join(&dep_cfg.manifest_file).exists()
            } else {
                false
            }
        }

        /// Build template variables using a pack-relative env_dir
        /// (legacy fallback — prefers [`build_template_vars_with_env`]).
        pub fn build_template_vars(&self, pack_dir: &Path) -> HashMap<&'static str, String> {
            let fallback_env_dir = self
                .environment
                .as_ref()
                .map(|cfg| pack_dir.join(&cfg.dir_name));
            self.build_template_vars_with_env(pack_dir, fallback_env_dir.as_deref())
        }

        /// Build template variables for a given pack directory and an explicit
        /// environment directory.
        ///
        /// The `env_dir` should be the external runtime environment path
        /// (e.g., `/opt/attune/runtime_envs/{pack_ref}/{runtime_name}`).
        /// If `None`, falls back to the pack-relative `dir_name`.
        pub fn build_template_vars_with_env(
            &self,
            pack_dir: &Path,
            env_dir: Option<&Path>,
        ) -> HashMap<&'static str, String> {
            let mut vars = HashMap::new();
            vars.insert("pack_dir", pack_dir.to_string_lossy().to_string());

            if let Some(env_dir) = env_dir {
                vars.insert("env_dir", env_dir.to_string_lossy().to_string());
            } else if let Some(ref env_cfg) = self.environment {
                let fallback = pack_dir.join(&env_cfg.dir_name);
                vars.insert("env_dir", fallback.to_string_lossy().to_string());
            }

            let interpreter = self.resolve_interpreter_with_env(pack_dir, env_dir);
            vars.insert("interpreter", interpreter.to_string_lossy().to_string());

            if let Some(ref dep_cfg) = self.dependencies {
                let manifest_path = pack_dir.join(&dep_cfg.manifest_file);
                vars.insert("manifest_path", manifest_path.to_string_lossy().to_string());
            }

            vars
        }

        /// Resolve a command template (Vec<String>) with the given variables.
        pub fn resolve_command(
            cmd_template: &[String],
            vars: &HashMap<&str, String>,
        ) -> Vec<String> {
            cmd_template
                .iter()
                .map(|part| Self::resolve_template(part, vars))
                .collect()
        }

        /// Check if this runtime can execute a file based on its extension.
        pub fn matches_file_extension(&self, file_path: &Path) -> bool {
            if let Some(ref ext) = self.interpreter.file_extension {
                let expected = ext.trim_start_matches('.');
                file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(expected))
                    .unwrap_or(false)
            } else {
                false
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Runtime {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub description: Option<String>,
        pub name: String,
        pub distributions: JsonDict,
        pub installation: Option<JsonDict>,
        pub installers: JsonDict,
        pub execution_config: JsonDict,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    impl Runtime {
        /// Parse the `execution_config` JSONB into a typed `RuntimeExecutionConfig`.
        pub fn parsed_execution_config(&self) -> RuntimeExecutionConfig {
            serde_json::from_value(self.execution_config.clone()).unwrap_or_default()
        }
    }

    /// A specific version of a runtime (e.g., Python 3.12.1, Node.js 20.11.0).
    ///
    /// Each version stores its own complete `execution_config` so the worker can
    /// use a version-specific interpreter binary, environment commands, etc.
    /// Actions and sensors declare an optional version constraint (semver range)
    /// which is matched against available `RuntimeVersion` rows at execution time.
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct RuntimeVersion {
        pub id: Id,
        /// Parent runtime ID (FK → runtime.id)
        pub runtime: Id,
        /// Parent runtime ref for display/filtering (e.g., "core.python")
        pub runtime_ref: String,
        /// Semantic version string (e.g., "3.12.1", "20.11.0")
        pub version: String,
        /// Major version component (nullable for non-numeric schemes)
        pub version_major: Option<i32>,
        /// Minor version component
        pub version_minor: Option<i32>,
        /// Patch version component
        pub version_patch: Option<i32>,
        /// Complete execution configuration for this version
        /// (same structure as `runtime.execution_config`)
        pub execution_config: JsonDict,
        /// Version-specific distribution/verification metadata
        pub distributions: JsonDict,
        /// Whether this is the default version for the parent runtime
        pub is_default: bool,
        /// Whether this version is verified as available on the system
        pub available: bool,
        /// When this version was last verified
        pub verified_at: Option<DateTime<Utc>>,
        /// Arbitrary version-specific metadata
        pub meta: JsonDict,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    impl RuntimeVersion {
        /// Parse the `execution_config` JSONB into a typed `RuntimeExecutionConfig`.
        pub fn parsed_execution_config(&self) -> RuntimeExecutionConfig {
            serde_json::from_value(self.execution_config.clone()).unwrap_or_default()
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Worker {
        pub id: Id,
        pub name: String,
        pub worker_type: WorkerType,
        pub worker_role: WorkerRole,
        pub runtime: Option<Id>,
        pub host: Option<String>,
        pub port: Option<i32>,
        pub status: Option<WorkerStatus>,
        pub capabilities: Option<JsonDict>,
        pub meta: Option<JsonDict>,
        pub last_heartbeat: Option<DateTime<Utc>>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Trigger model
pub mod trigger {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Trigger {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: String,
        pub description: Option<String>,
        pub enabled: bool,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub webhook_enabled: bool,
        pub webhook_key: Option<String>,
        pub webhook_config: Option<JsonDict>,
        pub is_adhoc: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Sensor {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: String,
        pub description: String,
        pub entrypoint: String,
        pub runtime: Id,
        pub runtime_ref: String,
        /// Optional semver version constraint for the runtime
        /// (e.g., ">=3.12", ">=3.12,<4.0", "~18.0"). NULL means any version.
        pub runtime_version_constraint: Option<String>,
        pub trigger: Id,
        pub trigger_ref: String,
        pub enabled: bool,
        pub param_schema: Option<JsonSchema>,
        pub config: Option<JsonValue>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Action model
pub mod action {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Action {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: String,
        pub entrypoint: String,
        pub runtime: Option<Id>,
        /// Optional semver version constraint for the runtime
        /// (e.g., ">=3.12", ">=3.12,<4.0", "~18.0"). NULL means any version.
        pub runtime_version_constraint: Option<String>,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub workflow_def: Option<Id>,
        pub is_adhoc: bool,
        #[sqlx(default)]
        pub parameter_delivery: ParameterDelivery,
        #[sqlx(default)]
        pub parameter_format: ParameterFormat,
        #[sqlx(default)]
        pub output_format: OutputFormat,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Policy {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub action: Option<Id>,
        pub action_ref: Option<String>,
        pub parameters: Vec<String>,
        pub method: PolicyMethod,
        pub threshold: i32,
        pub name: String,
        pub description: Option<String>,
        pub tags: Vec<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Rule model
pub mod rule {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Rule {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: String,
        pub action: Option<Id>,
        pub action_ref: String,
        pub trigger: Option<Id>,
        pub trigger_ref: String,
        pub conditions: JsonValue,
        pub action_params: JsonValue,
        pub trigger_params: JsonValue,
        pub enabled: bool,
        pub is_adhoc: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    /// Webhook event log for auditing and analytics
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WebhookEventLog {
        pub id: Id,
        pub trigger_id: Id,
        pub trigger_ref: String,
        pub webhook_key: String,
        pub event_id: Option<Id>,
        pub source_ip: Option<String>,
        pub user_agent: Option<String>,
        pub payload_size_bytes: Option<i32>,
        pub headers: Option<JsonValue>,
        pub status_code: i32,
        pub error_message: Option<String>,
        pub processing_time_ms: Option<i32>,
        pub hmac_verified: Option<bool>,
        pub rate_limited: bool,
        pub ip_allowed: Option<bool>,
        pub created: DateTime<Utc>,
    }
}

pub mod event {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Event {
        pub id: Id,
        pub trigger: Option<Id>,
        pub trigger_ref: String,
        pub config: Option<JsonDict>,
        pub payload: Option<JsonDict>,
        pub source: Option<Id>,
        pub source_ref: Option<String>,
        pub created: DateTime<Utc>,
        pub rule: Option<Id>,
        pub rule_ref: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Enforcement {
        pub id: Id,
        pub rule: Option<Id>,
        pub rule_ref: String,
        pub trigger_ref: String,
        pub config: Option<JsonDict>,
        pub event: Option<Id>,
        pub status: EnforcementStatus,
        pub payload: JsonDict,
        pub condition: EnforcementCondition,
        pub conditions: JsonValue,
        pub created: DateTime<Utc>,
        pub resolved_at: Option<DateTime<Utc>>,
    }
}

/// Execution model
pub mod execution {
    use super::*;

    /// Workflow-specific task metadata
    /// Stored as JSONB in the execution table's workflow_task column
    ///
    /// This metadata is only populated for workflow task executions.
    /// It provides a direct link to the workflow_execution record for efficient queries.
    ///
    /// Note: The `workflow_execution` field here is separate from `Execution.parent`.
    /// - `parent`: Generic execution hierarchy (used for all execution types)
    /// - `workflow_execution`: Specific link to workflow orchestration state
    ///
    /// See docs/execution-hierarchy.md for detailed explanation.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[cfg_attr(test, derive(Eq))]
    pub struct WorkflowTaskMetadata {
        /// ID of the workflow_execution record (orchestration state)
        pub workflow_execution: Id,

        /// Task name within the workflow
        pub task_name: String,

        /// Index for with-items iteration (0-based)
        pub task_index: Option<i32>,

        /// Batch number for batched with-items processing
        pub task_batch: Option<i32>,

        /// Current retry attempt count
        pub retry_count: i32,

        /// Maximum retries allowed
        pub max_retries: i32,

        /// Scheduled time for next retry
        pub next_retry_at: Option<DateTime<Utc>>,

        /// Timeout in seconds
        pub timeout_seconds: Option<i32>,

        /// Whether task timed out
        pub timed_out: bool,

        /// Task execution duration in milliseconds
        pub duration_ms: Option<i64>,

        /// When task started executing
        pub started_at: Option<DateTime<Utc>>,

        /// When task completed
        pub completed_at: Option<DateTime<Utc>>,
    }

    /// Represents an action execution with support for hierarchical relationships
    ///
    /// Executions support two types of parent-child relationships:
    ///
    /// 1. **Generic hierarchy** (`parent` field):
    ///    - Used for all execution types (workflows, actions, nested workflows)
    ///    - Enables generic tree traversal queries
    ///    - Example: action spawning child actions
    ///
    /// 2. **Workflow-specific** (`workflow_task` metadata):
    ///    - Only populated for workflow task executions
    ///    - Provides direct link to workflow orchestration state
    ///    - Example: task within a workflow execution
    ///
    /// For workflow tasks, both fields are populated and serve different purposes.
    /// See docs/execution-hierarchy.md for detailed explanation.
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Execution {
        pub id: Id,
        pub action: Option<Id>,
        pub action_ref: String,
        pub config: Option<JsonDict>,

        /// Environment variables for this execution (string -> string mapping)
        /// These are set as environment variables in the action's process.
        /// Separate from parameters which are passed via stdin/file.
        pub env_vars: Option<JsonDict>,

        /// Parent execution ID (generic hierarchy for all execution types)
        ///
        /// Used for:
        /// - Workflow tasks: parent is the workflow's execution
        /// - Child actions: parent is the spawning action
        /// - Nested workflows: parent is the outer workflow
        pub parent: Option<Id>,

        pub enforcement: Option<Id>,
        pub executor: Option<Id>,
        pub status: ExecutionStatus,
        pub result: Option<JsonDict>,

        /// When the execution actually started running (worker picked it up).
        /// Set when status transitions to `Running`. Used to compute accurate
        /// duration that excludes queue/scheduling wait time.
        pub started_at: Option<DateTime<Utc>>,

        /// Workflow task metadata (only populated for workflow task executions)
        ///
        /// Provides direct access to workflow orchestration state without JOINs.
        /// The `workflow_execution` field within this metadata is separate from
        /// the `parent` field above, as they serve different query patterns.
        #[sqlx(json, default)]
        pub workflow_task: Option<WorkflowTaskMetadata>,

        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    impl Execution {
        /// Check if this execution is a workflow task
        ///
        /// Returns `true` if this execution represents a task within a workflow,
        /// as opposed to a standalone action execution or the workflow itself.
        pub fn is_workflow_task(&self) -> bool {
            self.workflow_task.is_some()
        }

        /// Get the workflow execution ID if this is a workflow task
        ///
        /// Returns the ID of the workflow_execution record that contains
        /// the orchestration state (task graph, variables, etc.) for this task.
        pub fn workflow_execution_id(&self) -> Option<Id> {
            self.workflow_task.as_ref().map(|wt| wt.workflow_execution)
        }

        /// Check if this execution has child executions
        ///
        /// Note: This only checks if the parent field is populated.
        /// To actually query for children, use ExecutionRepository::find_by_parent().
        pub fn is_parent(&self) -> bool {
            // This would need a query to check, so we provide a helper for the inverse
            self.parent.is_some()
        }

        /// Get the task name if this is a workflow task
        pub fn task_name(&self) -> Option<&str> {
            self.workflow_task.as_ref().map(|wt| wt.task_name.as_str())
        }
    }
}

/// Inquiry model
pub mod inquiry {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Inquiry {
        pub id: Id,
        pub execution: Id,
        pub prompt: String,
        pub response_schema: Option<JsonSchema>,
        pub assigned_to: Option<Id>,
        pub status: InquiryStatus,
        pub response: Option<JsonDict>,
        pub timeout_at: Option<DateTime<Utc>>,
        pub responded_at: Option<DateTime<Utc>>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Identity and permissions
pub mod identity {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Identity {
        pub id: Id,
        pub login: String,
        pub display_name: Option<String>,
        pub password_hash: Option<String>,
        pub attributes: JsonDict,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct PermissionSet {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: Option<String>,
        pub description: Option<String>,
        pub grants: JsonValue,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct PermissionAssignment {
        pub id: Id,
        pub identity: Id,
        pub permset: Id,
        pub created: DateTime<Utc>,
    }
}

/// Key/Value storage
pub mod key {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Key {
        pub id: Id,
        pub r#ref: String,
        pub owner_type: OwnerType,
        pub owner: Option<String>,
        pub owner_identity: Option<Id>,
        pub owner_pack: Option<Id>,
        pub owner_pack_ref: Option<String>,
        pub owner_action: Option<Id>,
        pub owner_action_ref: Option<String>,
        pub owner_sensor: Option<Id>,
        pub owner_sensor_ref: Option<String>,
        pub name: String,
        pub encrypted: bool,
        pub encryption_key_hash: Option<String>,
        pub value: String,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Notification model
pub mod notification {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Notification {
        pub id: Id,
        pub channel: String,
        pub entity_type: String,
        pub entity: String,
        pub activity: String,
        pub state: NotificationState,
        pub content: Option<JsonDict>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Artifact model
pub mod artifact {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Artifact {
        pub id: Id,
        pub r#ref: String,
        pub scope: OwnerType,
        pub owner: String,
        pub r#type: ArtifactType,
        pub retention_policy: RetentionPolicyType,
        pub retention_limit: i32,
        /// Human-readable name (e.g. "Build Log", "Test Results")
        pub name: Option<String>,
        /// Optional longer description
        pub description: Option<String>,
        /// MIME content type (e.g. "application/json", "text/plain")
        pub content_type: Option<String>,
        /// Size of the latest version's content in bytes
        pub size_bytes: Option<i64>,
        /// Execution that produced this artifact (no FK — execution is a hypertable)
        pub execution: Option<Id>,
        /// Structured JSONB data for progress artifacts or metadata
        pub data: Option<serde_json::Value>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    /// Select columns for Artifact queries (excludes DB-only columns if any arise).
    /// Must be kept in sync with the Artifact struct field order.
    pub const SELECT_COLUMNS: &str =
        "id, ref, scope, owner, type, retention_policy, retention_limit, \
         name, description, content_type, size_bytes, execution, data, \
         created, updated";
}

/// Artifact version model — immutable content snapshots
pub mod artifact_version {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct ArtifactVersion {
        pub id: Id,
        /// Parent artifact
        pub artifact: Id,
        /// Version number (1-based, monotonically increasing per artifact)
        pub version: i32,
        /// MIME content type for this version
        pub content_type: Option<String>,
        /// Size of content in bytes
        pub size_bytes: Option<i64>,
        /// Binary content (file data) — not included in default queries for performance
        #[serde(skip_serializing)]
        pub content: Option<Vec<u8>>,
        /// Structured JSON content
        pub content_json: Option<serde_json::Value>,
        /// Free-form metadata about this version
        pub meta: Option<serde_json::Value>,
        /// Who created this version
        pub created_by: Option<String>,
        pub created: DateTime<Utc>,
    }

    /// Select columns WITHOUT the potentially large `content` BYTEA column.
    /// Use `SELECT_COLUMNS_WITH_CONTENT` when you need the binary payload.
    pub const SELECT_COLUMNS: &str = "id, artifact, version, content_type, size_bytes, \
         NULL::bytea AS content, content_json, meta, created_by, created";

    /// Select columns INCLUDING the binary `content` column.
    pub const SELECT_COLUMNS_WITH_CONTENT: &str =
        "id, artifact, version, content_type, size_bytes, \
         content, content_json, meta, created_by, created";
}

/// Workflow orchestration models
pub mod workflow {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WorkflowDefinition {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: Option<String>,
        pub version: String,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub definition: JsonDict,
        pub tags: Vec<String>,
        pub enabled: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WorkflowExecution {
        pub id: Id,
        pub execution: Id,
        pub workflow_def: Id,
        pub current_tasks: Vec<String>,
        pub completed_tasks: Vec<String>,
        pub failed_tasks: Vec<String>,
        pub skipped_tasks: Vec<String>,
        pub variables: JsonDict,
        pub task_graph: JsonDict,
        pub status: ExecutionStatus,
        pub error_message: Option<String>,
        pub paused: bool,
        pub pause_reason: Option<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Pack testing models
pub mod pack_test {
    use super::*;
    use utoipa::ToSchema;

    /// Pack test execution record
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestExecution {
        pub id: Id,
        pub pack_id: Id,
        pub pack_version: String,
        pub execution_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
        pub result: JsonValue,
        pub created: DateTime<Utc>,
    }

    /// Pack test result structure (not from DB, used for test execution)
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestResult {
        pub pack_ref: String,
        pub pack_version: String,
        pub execution_time: DateTime<Utc>,
        pub status: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
        pub test_suites: Vec<TestSuiteResult>,
    }

    /// Test suite result (collection of test cases)
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct TestSuiteResult {
        pub name: String,
        pub runner_type: String,
        pub total: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub duration_ms: i64,
        pub test_cases: Vec<TestCaseResult>,
    }

    /// Individual test case result
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct TestCaseResult {
        pub name: String,
        pub status: TestStatus,
        pub duration_ms: i64,
        pub error_message: Option<String>,
        pub stdout: Option<String>,
        pub stderr: Option<String>,
    }

    /// Test status enum
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum TestStatus {
        Passed,
        Failed,
        Skipped,
        Error,
    }

    /// Pack test summary view
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestSummary {
        pub pack_id: Id,
        pub pack_ref: String,
        pub pack_label: String,
        pub test_execution_id: Id,
        pub pack_version: String,
        pub test_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
    }

    /// Pack latest test view
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackLatestTest {
        pub pack_id: Id,
        pub pack_ref: String,
        pub pack_label: String,
        pub test_execution_id: Id,
        pub pack_version: String,
        pub test_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
    }

    /// Pack test statistics
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestStats {
        pub total_executions: i64,
        pub successful_executions: i64,
        pub failed_executions: i64,
        pub avg_pass_rate: Option<f64>,
        pub avg_duration_ms: Option<i64>,
        pub last_test_time: Option<DateTime<Utc>>,
        pub last_test_passed: Option<bool>,
    }
}

/// Entity history tracking models (TimescaleDB hypertables)
///
/// These models represent rows in the `<entity>_history` append-only hypertables
/// that track field-level changes to operational tables via PostgreSQL triggers.
pub mod entity_history {
    use super::*;

    /// A single history record capturing a field-level change to an entity.
    ///
    /// History records are append-only and populated by PostgreSQL triggers —
    /// they are never created or modified by application code.
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct EntityHistoryRecord {
        /// When the change occurred (hypertable partitioning dimension)
        pub time: DateTime<Utc>,

        /// The operation that produced this record: `INSERT`, `UPDATE`, or `DELETE`
        pub operation: String,

        /// The primary key of the changed row in the source table
        pub entity_id: Id,

        /// Denormalized human-readable identifier (e.g., `action_ref`, `worker.name`, `rule_ref`, `trigger_ref`)
        pub entity_ref: Option<String>,

        /// Names of fields that changed in this operation (empty for INSERT/DELETE)
        pub changed_fields: Vec<String>,

        /// Previous values of the changed fields (NULL for INSERT)
        pub old_values: Option<JsonValue>,

        /// New values of the changed fields (NULL for DELETE)
        pub new_values: Option<JsonValue>,
    }

    /// Supported entity types that have history tracking.
    ///
    /// Each variant maps to a `<name>_history` hypertable in the database.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum HistoryEntityType {
        Execution,
        Worker,
    }

    impl HistoryEntityType {
        /// Returns the history table name for this entity type.
        pub fn table_name(&self) -> &'static str {
            match self {
                Self::Execution => "execution_history",
                Self::Worker => "worker_history",
            }
        }
    }

    impl std::fmt::Display for HistoryEntityType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Execution => write!(f, "execution"),
                Self::Worker => write!(f, "worker"),
            }
        }
    }

    impl std::str::FromStr for HistoryEntityType {
        type Err = String;

        fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "execution" => Ok(Self::Execution),
                "worker" => Ok(Self::Worker),
                other => Err(format!(
                    "unknown history entity type '{}'; expected one of: execution, worker",
                    other
                )),
            }
        }
    }
}
