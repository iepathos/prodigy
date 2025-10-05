//! MapReduce workflow configuration parsing
//!
//! Handles parsing of MapReduce workflow YAML files.

use crate::cook::environment::{EnvProfile, SecretValue};
use crate::cook::execution::variable_capture::CaptureConfig;
use crate::cook::execution::{MapPhase, MapReduceConfig, ReducePhase, SetupPhase};
use crate::cook::workflow::{WorkflowErrorPolicy, WorkflowStep};
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// MapReduce workflow configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceWorkflowConfig {
    /// Workflow name
    pub name: String,

    /// Workflow mode (should be "mapreduce")
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Global environment variables for all commands
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Secret environment variables (masked in logs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, SecretValue>>,

    /// Environment files to load (.env format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_files: Option<Vec<PathBuf>>,

    /// Environment profiles for different contexts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<HashMap<String, EnvProfile>>,

    /// Optional setup phase with separate configuration or simple list of steps
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_setup_phase_option"
    )]
    pub setup: Option<SetupPhaseConfig>,

    /// Map phase configuration
    pub map: MapPhaseYaml,

    /// Optional reduce phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce: Option<ReducePhaseYaml>,

    /// Workflow-level error handling policy
    #[serde(default, skip_serializing_if = "is_default_error_policy")]
    pub error_policy: WorkflowErrorPolicy,

    /// Action to take when an item fails (convenience field, maps to error_policy.on_item_failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_item_failure: Option<String>,

    /// Continue processing after failures (convenience field, maps to error_policy.continue_on_failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_on_failure: Option<bool>,

    /// Maximum number of failures before stopping (convenience field, maps to error_policy.max_failures)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_failures: Option<usize>,

    /// Failure rate threshold (convenience field, maps to error_policy.failure_threshold)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_threshold: Option<f64>,

    /// Error collection strategy (convenience field, maps to error_policy.error_collection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_collection: Option<String>,

    /// Optional custom merge workflow for worktree integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeWorkflow>,
}

/// Custom merge workflow configuration
#[derive(Debug, Clone, Serialize)]
pub struct MergeWorkflow {
    /// Commands to execute for merge process
    pub commands: Vec<WorkflowStep>,

    /// Timeout for the entire merge phase (in seconds)
    /// If not specified, no timeout is applied
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

impl<'de> Deserialize<'de> for MergeWorkflow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum MergeValue {
            // Direct list of commands (simplified format)
            Commands(Vec<WorkflowStep>),
            // Full config with commands and timeout
            Config {
                commands: Vec<WorkflowStep>,
                #[serde(default)]
                timeout: Option<u64>,
            },
        }

        let value = MergeValue::deserialize(deserializer)?;

        match value {
            MergeValue::Commands(commands) => Ok(MergeWorkflow {
                commands,
                timeout: None, // No timeout by default
            }),
            MergeValue::Config { commands, timeout } => Ok(MergeWorkflow { commands, timeout }),
        }
    }
}

fn is_default_error_policy(policy: &WorkflowErrorPolicy) -> bool {
    // Check if the policy equals the default
    matches!(policy, WorkflowErrorPolicy { .. } if false) // Never skip for now, can optimize later
}

fn default_mode() -> String {
    "mapreduce".to_string()
}

/// Setup phase configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhaseConfig {
    /// Commands to execute during setup
    pub commands: Vec<WorkflowStep>,

    /// Timeout for the entire setup phase (in seconds)
    /// If not specified, no timeout is applied
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_u64_or_string"
    )]
    pub timeout: Option<String>,

    /// Variables to capture from setup commands
    /// Key is variable name, value is the capture configuration
    #[serde(
        default,
        skip_serializing_if = "HashMap::is_empty",
        deserialize_with = "deserialize_capture_outputs"
    )]
    pub capture_outputs: HashMap<String, CaptureConfig>,
}

/// Custom deserializer for capture_outputs that supports both legacy and new format
fn deserialize_capture_outputs<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, CaptureConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CaptureValue {
        // Legacy format: just a command index
        LegacyIndex(usize),
        // New format: full CaptureConfig
        Config(CaptureConfig),
    }

    let raw_map: HashMap<String, CaptureValue> = HashMap::deserialize(deserializer)?;
    let mut result = HashMap::new();

    for (key, value) in raw_map {
        let config = match value {
            CaptureValue::LegacyIndex(idx) => CaptureConfig::Simple(idx),
            CaptureValue::Config(cfg) => cfg,
        };
        result.insert(key, config);
    }

    Ok(result)
}

/// Custom deserializer for setup phase that supports both simple list and full config
fn deserialize_setup_phase_option<'de, D>(
    deserializer: D,
) -> Result<Option<SetupPhaseConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SetupValue {
        Commands(Vec<WorkflowStep>),
        Config(SetupPhaseConfig),
    }

    let value = Option::<SetupValue>::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(SetupValue::Commands(commands)) => {
            // Convert simple list of commands to full setup config
            // No timeout by default - user must specify if they want one
            Ok(Some(SetupPhaseConfig {
                commands,
                timeout: None,
                capture_outputs: HashMap::new(),
            }))
        }
        Some(SetupValue::Config(config)) => Ok(Some(config)),
    }
}

/// Map phase configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhaseYaml {
    /// Input source: either a file path or command to execute
    pub input: String,

    /// JSON path expression
    #[serde(default)]
    pub json_path: String,

    /// Agent template commands
    pub agent_template: AgentTemplate,

    /// Maximum parallel agents (can be a number or environment variable reference)
    #[serde(
        default = "default_max_parallel_string",
        deserialize_with = "deserialize_usize_or_string"
    )]
    pub max_parallel: String,

    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// Optional sort field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,

    /// Maximum number of items to process (limit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,

    /// Number of items to skip (offset)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,

    /// Field for deduplication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct: Option<String>,

    /// Agent timeout in seconds (can be a number or environment variable reference)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_u64_or_string"
    )]
    pub agent_timeout_secs: Option<String>,

    /// Timeout configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_config: Option<crate::cook::execution::mapreduce::timeout::TimeoutConfig>,
}

fn default_max_parallel_string() -> String {
    "10".to_string()
}

/// Agent template configuration
#[derive(Debug, Clone, Serialize)]
pub struct AgentTemplate {
    /// Commands to execute for each work item
    pub commands: Vec<WorkflowStep>,
}

impl<'de> Deserialize<'de> for AgentTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AgentTemplateValue {
            // New simplified format: direct array of steps
            Commands(Vec<WorkflowStep>),
            // Old nested format with 'commands' key
            Nested { commands: Vec<WorkflowStep> },
        }

        let value = AgentTemplateValue::deserialize(deserializer)?;

        match value {
            AgentTemplateValue::Commands(commands) => {
                // Using the new simplified format - this is preferred
                Ok(AgentTemplate { commands })
            }
            AgentTemplateValue::Nested { commands } => {
                // Using deprecated nested format
                tracing::warn!("Using deprecated nested 'commands' syntax in agent_template. Consider using the simplified array format directly under 'agent_template'.");
                Ok(AgentTemplate { commands })
            }
        }
    }
}

/// Reduce phase configuration from YAML
#[derive(Debug, Clone, Serialize)]
pub struct ReducePhaseYaml {
    /// Commands to execute in reduce phase
    pub commands: Vec<WorkflowStep>,
}

impl<'de> Deserialize<'de> for ReducePhaseYaml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ReduceValue {
            // New simplified format: direct array of steps
            Commands(Vec<WorkflowStep>),
            // Old nested format with 'commands' key
            Nested { commands: Vec<WorkflowStep> },
        }

        let value = ReduceValue::deserialize(deserializer)?;

        match value {
            ReduceValue::Commands(commands) => {
                // Using the new simplified format - this is preferred
                Ok(ReducePhaseYaml { commands })
            }
            ReduceValue::Nested { commands } => {
                // Using deprecated nested format
                tracing::warn!("Using deprecated nested 'commands' syntax in reduce. Consider using the simplified array format directly under 'reduce'.");
                Ok(ReducePhaseYaml { commands })
            }
        }
    }
}

/// Custom deserializer for usize values that can also be environment variable references
fn deserialize_usize_or_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum UsizeOrString {
        Number(usize),
        String(String),
    }

    let value = UsizeOrString::deserialize(deserializer)?;

    match value {
        UsizeOrString::Number(n) => Ok(n.to_string()),
        UsizeOrString::String(s) => Ok(s),
    }
}

/// Custom deserializer for optional u64 values that can also be environment variable references
fn deserialize_optional_u64_or_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum U64OrString {
        Number(u64),
        String(String),
    }

    let value = Option::<U64OrString>::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(U64OrString::Number(n)) => Ok(Some(n.to_string())),
        Some(U64OrString::String(s)) => Ok(Some(s)),
    }
}

impl MapReduceWorkflowConfig {
    /// Resolve an environment variable reference or parse a numeric string
    fn resolve_env_or_parse<T>(&self, value: &str) -> Result<T, anyhow::Error>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        // Check if it's an environment variable reference (both ${VAR} and $VAR syntaxes)
        if value.starts_with('$') {
            // Extract variable name from ${VAR_NAME} or $VAR_NAME
            let var_name = if let Some(stripped) = value.strip_prefix("${") {
                stripped.strip_suffix('}').unwrap_or(stripped)
            } else if let Some(stripped) = value.strip_prefix('$') {
                stripped
            } else {
                value
            };

            // Try to resolve from workflow env first
            if let Some(ref env) = self.env {
                if let Some(env_value) = env.get(var_name) {
                    return env_value.parse::<T>().map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to parse environment variable '{}' value '{}': {}",
                            var_name,
                            env_value,
                            e
                        )
                    });
                }
            }

            // Fall back to system environment
            if let Ok(env_value) = std::env::var(var_name) {
                return env_value.parse::<T>().map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to parse environment variable '{}' value '{}': {}",
                        var_name,
                        env_value,
                        e
                    )
                });
            }

            return Err(anyhow::anyhow!(
                "Environment variable '{}' not found in workflow env or system environment",
                var_name
            ));
        }

        // Parse as a plain number
        value
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("Failed to parse numeric value '{}': {}", value, e))
    }

    /// Get the merged error policy, combining convenience fields with the error_policy field
    pub fn get_error_policy(&self) -> WorkflowErrorPolicy {
        use crate::cook::workflow::{ErrorCollectionStrategy, ItemFailureAction};

        let mut policy = self.error_policy.clone();

        // Apply convenience field overrides
        if let Some(ref action_str) = self.on_item_failure {
            policy.on_item_failure = match action_str.as_str() {
                "dlq" => ItemFailureAction::Dlq,
                "retry" => ItemFailureAction::Retry,
                "skip" => ItemFailureAction::Skip,
                "stop" => ItemFailureAction::Stop,
                custom => ItemFailureAction::Custom(custom.to_string()),
            };
        }

        if let Some(continue_on_failure) = self.continue_on_failure {
            policy.continue_on_failure = continue_on_failure;
        }

        if let Some(max_failures) = self.max_failures {
            policy.max_failures = Some(max_failures);
        }

        if let Some(failure_threshold) = self.failure_threshold {
            policy.failure_threshold = Some(failure_threshold);
        }

        if let Some(ref collection_str) = self.error_collection {
            policy.error_collection = match collection_str.as_str() {
                "aggregate" => ErrorCollectionStrategy::Aggregate,
                "immediate" => ErrorCollectionStrategy::Immediate,
                _ if collection_str.starts_with("batched:") => {
                    if let Some(size_str) = collection_str.strip_prefix("batched:") {
                        if let Ok(size) = size_str.parse::<usize>() {
                            ErrorCollectionStrategy::Batched { size }
                        } else {
                            ErrorCollectionStrategy::Aggregate
                        }
                    } else {
                        ErrorCollectionStrategy::Aggregate
                    }
                }
                _ => ErrorCollectionStrategy::Aggregate,
            };
        }

        policy
    }

    /// Convert to execution-ready SetupPhase
    pub fn to_setup_phase(&self) -> Result<Option<SetupPhase>, anyhow::Error> {
        if let Some(ref s) = self.setup {
            // Resolve timeout if present
            let timeout = if let Some(ref timeout_str) = s.timeout {
                Some(
                    self.resolve_env_or_parse::<u64>(timeout_str)
                        .context("Failed to resolve setup timeout")?,
                )
            } else {
                None
            };

            Ok(Some(SetupPhase {
                commands: s.commands.clone(),
                timeout,
                capture_outputs: s.capture_outputs.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Convert to execution-ready MapPhase
    /// Returns an error if environment variable resolution or numeric parsing fails
    pub fn to_map_phase(&self) -> Result<MapPhase, anyhow::Error> {
        // Resolve max_parallel from string (may be a number or env var reference)
        let max_parallel = self
            .resolve_env_or_parse::<usize>(&self.map.max_parallel)
            .context("Failed to resolve max_parallel")?;

        // Resolve agent_timeout_secs if present
        let agent_timeout_secs = if let Some(ref timeout_str) = self.map.agent_timeout_secs {
            Some(
                self.resolve_env_or_parse::<u64>(timeout_str)
                    .context("Failed to resolve agent_timeout_secs")?,
            )
        } else {
            None
        };

        Ok(MapPhase {
            config: MapReduceConfig {
                input: self.map.input.clone(),
                json_path: self.map.json_path.clone(),
                max_parallel,
                agent_timeout_secs,
                continue_on_failure: false,
                batch_size: None,
                enable_checkpoints: true,
                max_items: self.map.max_items,
                offset: self.map.offset,
            },
            json_path: Some(self.map.json_path.clone()).filter(|s| !s.is_empty()),
            agent_template: self.map.agent_template.commands.clone(),
            filter: self.map.filter.clone(),
            sort_by: self.map.sort_by.clone(),
            max_items: self.map.max_items,
            distinct: self.map.distinct.clone(),
            timeout_config: self.map.timeout_config.clone(),
        })
    }

    /// Convert to execution-ready ReducePhase
    pub fn to_reduce_phase(&self) -> Option<ReducePhase> {
        self.reduce.as_ref().map(|r| ReducePhase {
            commands: r.commands.clone(),
            timeout_secs: None,
        })
    }

    /// Check if this is a MapReduce workflow
    pub fn is_mapreduce(&self) -> bool {
        self.mode.to_lowercase() == "mapreduce"
    }
}

/// Parse a MapReduce workflow from YAML content
pub fn parse_mapreduce_workflow(
    content: &str,
) -> Result<MapReduceWorkflowConfig, serde_yaml::Error> {
    serde_yaml::from_str(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_mapreduce_workflow() {
        let yaml = r#"
name: parallel-debt-elimination
mode: mapreduce

map:
  input: items.json
  json_path: "$.debt_items[*]"
  
  agent_template:
    commands:
      - claude: "/fix-issue ${item.description}"
      - shell: "cargo test"
  
  max_parallel: 10

reduce:
  commands:
    - claude: "/summarize-fixes ${map.results}"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.name, "parallel-debt-elimination");
        assert_eq!(config.mode, "mapreduce");
        assert_eq!(config.map.max_parallel, "10");
        assert_eq!(config.map.agent_template.commands.len(), 2);
    }

    #[test]
    fn test_simplified_agent_template_syntax() {
        // Test new simplified format (preferred)
        let yaml = r#"
name: test-simplified
mode: mapreduce

map:
  input: items.json
  json_path: "$.items[*]"

  # New simplified syntax - direct array of commands
  agent_template:
    - claude: "/process '${item}'"
    - shell: "validate ${item}"

  max_parallel: 5
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.name, "test-simplified");
        assert_eq!(config.map.agent_template.commands.len(), 2);

        // Verify commands are correctly parsed
        let first_step = &config.map.agent_template.commands[0];
        assert!(first_step.claude.is_some());
        assert!(first_step.claude.as_ref().unwrap().contains("/process"));
    }

    #[test]
    fn test_nested_agent_template_syntax() {
        // Test old nested format (still supported for backward compatibility)
        let yaml = r#"
name: test-nested
mode: mapreduce

map:
  input: items.json
  json_path: "$.items[*]"

  # Old nested syntax with 'commands' key
  agent_template:
    commands:
      - claude: "/process '${item}'"
      - shell: "validate ${item}"

  max_parallel: 5
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.name, "test-nested");
        assert_eq!(config.map.agent_template.commands.len(), 2);
    }

    #[test]
    fn test_simplified_reduce_syntax() {
        // Test new simplified format for reduce phase
        let yaml = r#"
name: test-reduce-simplified
mode: mapreduce

map:
  input: items.json
  agent_template:
    - shell: "echo processing"

# New simplified reduce syntax
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "generate-report"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.reduce.is_some());
        assert_eq!(config.reduce.as_ref().unwrap().commands.len(), 2);
    }

    #[test]
    fn test_nested_reduce_syntax() {
        // Test old nested format for reduce phase
        let yaml = r#"
name: test-reduce-nested
mode: mapreduce

map:
  input: items.json
  agent_template:
    - shell: "echo processing"

# Old nested reduce syntax
reduce:
  commands:
    - claude: "/summarize ${map.results}"
    - shell: "generate-report"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.reduce.is_some());
        assert_eq!(config.reduce.as_ref().unwrap().commands.len(), 2);
    }

    #[test]
    fn test_mixed_simplified_and_nested_syntax() {
        // Test workflow with mixed syntax (not recommended but should work)
        let yaml = r#"
name: test-mixed
mode: mapreduce

# Setup uses simple list format (already supported)
setup:
  - shell: "prepare-data"
  - claude: "/analyze-requirements"

map:
  input: items.json
  # Using new simplified syntax for agent_template
  agent_template:
    - claude: "/process ${item}"
    - shell: "test ${item}"

# Using old nested syntax for reduce
reduce:
  commands:
    - claude: "/summarize ${map.results}"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.setup.is_some());
        assert_eq!(config.setup.as_ref().unwrap().commands.len(), 2);
        assert_eq!(config.map.agent_template.commands.len(), 2);
        assert!(config.reduce.is_some());
        assert_eq!(config.reduce.as_ref().unwrap().commands.len(), 1);
    }

    #[test]
    fn test_mapreduce_with_env_variables() {
        let yaml = r#"
name: test-env-vars
mode: mapreduce

env:
  PROJECT_NAME: "TestProject"
  CONFIG_PATH: ".test/config.json"
  OUTPUT_DIR: ".test/output"

map:
  input: items.json
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo Processing $PROJECT_NAME"
    - claude: "/process --config $CONFIG_PATH"

reduce:
  - shell: "echo Saving to $OUTPUT_DIR"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.env.is_some());
        let env = config.env.unwrap();
        assert_eq!(env.get("PROJECT_NAME"), Some(&"TestProject".to_string()));
        assert_eq!(
            env.get("CONFIG_PATH"),
            Some(&".test/config.json".to_string())
        );
        assert_eq!(env.get("OUTPUT_DIR"), Some(&".test/output".to_string()));
    }

    #[test]
    fn test_mapreduce_backward_compatibility_without_env() {
        let yaml = r#"
name: test-no-env
mode: mapreduce

map:
  input: items.json
  agent_template:
    - shell: "echo test"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.env.is_none());
        assert!(config.secrets.is_none());
        assert!(config.env_files.is_none());
        assert!(config.profiles.is_none());
    }
}

#[cfg(test)]
mod merge_workflow_tests {
    use super::*;

    #[test]
    fn test_deserialize_simplified_syntax() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  - shell: "git fetch origin"
  - claude: "/merge-master ${merge.source_branch}"
  - shell: "cargo test"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.commands.len(), 3);
        assert_eq!(merge.timeout, None); // No timeout by default

        // Check first command
        assert!(merge.commands[0].shell.is_some());
        assert_eq!(
            merge.commands[0].shell.as_ref().unwrap(),
            "git fetch origin"
        );

        // Check second command
        assert!(merge.commands[1].claude.is_some());
        assert!(merge.commands[1]
            .claude
            .as_ref()
            .unwrap()
            .contains("${merge.source_branch}"));
    }

    #[test]
    fn test_deserialize_full_syntax() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  commands:
    - shell: "git fetch origin"
    - claude: "/merge-master"
    - shell: "git push"
  timeout: 900
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.commands.len(), 3);
        assert_eq!(merge.timeout, Some(900)); // Custom timeout

        // Verify commands
        assert!(merge.commands[0].shell.is_some());
        assert!(merge.commands[1].claude.is_some());
        assert!(merge.commands[2].shell.is_some());
    }

    #[test]
    fn test_default_timeout() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  commands:
    - shell: "git merge"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.timeout, None); // No timeout by default
    }

    #[test]
    fn test_empty_merge_workflow() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge: []
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.commands.len(), 0);
    }

    #[test]
    fn test_no_merge_workflow() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_none());
    }

    #[test]
    fn test_merge_with_variable_interpolation() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  - shell: "echo Merging ${merge.worktree}"
  - shell: "git checkout ${merge.target_branch}"
  - shell: "git merge ${merge.source_branch}"
  - claude: "/log-merge ${merge.session_id}"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.commands.len(), 4);

        // Verify all variable placeholders are present
        assert!(merge.commands[0]
            .shell
            .as_ref()
            .unwrap()
            .contains("${merge.worktree}"));
        assert!(merge.commands[1]
            .shell
            .as_ref()
            .unwrap()
            .contains("${merge.target_branch}"));
        assert!(merge.commands[2]
            .shell
            .as_ref()
            .unwrap()
            .contains("${merge.source_branch}"));
        assert!(merge.commands[3]
            .claude
            .as_ref()
            .unwrap()
            .contains("${merge.session_id}"));
    }

    #[test]
    fn test_invalid_merge_syntax_handled_gracefully() {
        // Test that invalid YAML is caught by the parser
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  invalid_key: "should not parse"
"#;

        // This should fail to parse because invalid_key is not a valid format
        let result = parse_mapreduce_workflow(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_workflow_with_on_failure() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: items.json
  agent_template:
    - shell: "echo test"

merge:
  - shell: "cargo test"
    on_failure:
      claude: "/fix-test-failures"
  - claude: "/merge-worktree"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert!(config.merge.is_some());

        let merge = config.merge.unwrap();
        assert_eq!(merge.commands.len(), 2);

        // Check that on_failure is preserved
        assert!(merge.commands[0].on_failure.is_some());
    }
}
