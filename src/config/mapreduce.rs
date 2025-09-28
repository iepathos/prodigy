//! MapReduce workflow configuration parsing
//!
//! Handles parsing of MapReduce workflow YAML files.

use crate::cook::execution::variable_capture::CaptureConfig;
use crate::cook::execution::{MapPhase, MapReduceConfig, ReducePhase, SetupPhase};
use crate::cook::workflow::{WorkflowErrorPolicy, WorkflowStep};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MapReduce workflow configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceWorkflowConfig {
    /// Workflow name
    pub name: String,

    /// Workflow mode (should be "mapreduce")
    #[serde(default = "default_mode")]
    pub mode: String,

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
    #[serde(default = "default_merge_timeout")]
    pub timeout: u64,
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
                #[serde(default = "default_merge_timeout")]
                timeout: u64,
            },
        }

        let value = MergeValue::deserialize(deserializer)?;

        match value {
            MergeValue::Commands(commands) => Ok(MergeWorkflow {
                commands,
                timeout: default_merge_timeout(),
            }),
            MergeValue::Config { commands, timeout } => Ok(MergeWorkflow { commands, timeout }),
        }
    }
}

fn default_merge_timeout() -> u64 {
    600 // 10 minutes default for merge operations
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
    #[serde(
        default = "default_setup_timeout",
        deserialize_with = "deserialize_timeout_required"
    )]
    pub timeout: u64,

    /// Variables to capture from setup commands
    /// Key is variable name, value is the capture configuration
    #[serde(
        default,
        skip_serializing_if = "HashMap::is_empty",
        deserialize_with = "deserialize_capture_outputs"
    )]
    pub capture_outputs: HashMap<String, CaptureConfig>,
}

fn default_setup_timeout() -> u64 {
    300 // 5 minutes default
}

/// Custom deserializer for required timeout values
fn deserialize_timeout_required<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_timeout(deserializer)?
        .ok_or_else(|| serde::de::Error::custom("timeout is required"))
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
            Ok(Some(SetupPhaseConfig {
                commands,
                timeout: default_setup_timeout(),
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

    /// Maximum parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,

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
}

fn default_max_parallel() -> usize {
    10
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

/// Custom deserializer for timeout values
fn deserialize_timeout<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TimeoutValue {
        Number(u64),
        String(String),
    }

    let value = Option::<TimeoutValue>::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(TimeoutValue::Number(n)) => Ok(Some(n)),
        Some(TimeoutValue::String(s)) => {
            // Parse strings like "600s", "10m", etc.
            if let Some(num_str) = s.strip_suffix('s') {
                num_str
                    .parse::<u64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            } else if let Some(num_str) = s.strip_suffix('m') {
                num_str
                    .parse::<u64>()
                    .map(|m| Some(m * 60))
                    .map_err(serde::de::Error::custom)
            } else {
                // Try parsing as plain number
                s.parse::<u64>().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl MapReduceWorkflowConfig {
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
    pub fn to_setup_phase(&self) -> Option<SetupPhase> {
        self.setup.as_ref().map(|s| SetupPhase {
            commands: s.commands.clone(),
            timeout: s.timeout,
            capture_outputs: s.capture_outputs.clone(),
        })
    }

    /// Convert to execution-ready MapPhase
    pub fn to_map_phase(&self) -> MapPhase {
        MapPhase {
            config: MapReduceConfig {
                input: self.map.input.clone(),
                json_path: self.map.json_path.clone(),
                max_parallel: self.map.max_parallel,
                agent_timeout_secs: None,
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
        }
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
        assert_eq!(config.map.max_parallel, 10);
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
        assert_eq!(merge.timeout, 600); // Default timeout

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
        assert_eq!(merge.timeout, 900); // Custom timeout

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
        assert_eq!(merge.timeout, 600); // Should use default of 600
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
