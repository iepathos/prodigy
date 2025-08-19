//! MapReduce workflow configuration parsing
//!
//! Handles parsing of MapReduce workflow YAML files.

use crate::cook::execution::{MapPhase, MapReduceConfig, ReducePhase};
use crate::cook::workflow::WorkflowStep;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// MapReduce workflow configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceWorkflowConfig {
    /// Workflow name
    pub name: String,

    /// Workflow mode (should be "mapreduce")
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Optional setup phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<Vec<WorkflowStep>>,

    /// Map phase configuration
    pub map: MapPhaseYaml,

    /// Optional reduce phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce: Option<ReducePhaseYaml>,
}

fn default_mode() -> String {
    "mapreduce".to_string()
}

/// Map phase configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhaseYaml {
    /// Input file path
    pub input: PathBuf,

    /// JSON path expression
    #[serde(default)]
    pub json_path: String,

    /// Agent template commands
    pub agent_template: AgentTemplate,

    /// Maximum parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,

    /// Timeout per agent (can be string like "600s" or number)
    #[serde(default, deserialize_with = "deserialize_timeout")]
    pub timeout_per_agent: Option<u64>,

    /// Retry attempts on failure
    #[serde(default)]
    pub retry_on_failure: u32,

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
}

fn default_max_parallel() -> usize {
    10
}

/// Agent template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplate {
    /// Commands to execute for each work item
    pub commands: Vec<WorkflowStep>,
}

/// Reduce phase configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseYaml {
    /// Commands to execute in reduce phase
    pub commands: Vec<WorkflowStep>,
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
    /// Convert to execution-ready MapPhase
    pub fn to_map_phase(&self) -> MapPhase {
        MapPhase {
            config: MapReduceConfig {
                input: self.map.input.clone(),
                json_path: self.map.json_path.clone(),
                max_parallel: self.map.max_parallel,
                timeout_per_agent: self.map.timeout_per_agent.unwrap_or(600),
                retry_on_failure: self.map.retry_on_failure,
                max_items: self.map.max_items,
                offset: self.map.offset,
            },
            agent_template: self.map.agent_template.commands.clone(),
            filter: self.map.filter.clone(),
            sort_by: self.map.sort_by.clone(),
        }
    }

    /// Convert to execution-ready ReducePhase
    pub fn to_reduce_phase(&self) -> Option<ReducePhase> {
        self.reduce.as_ref().map(|r| ReducePhase {
            commands: r.commands.clone(),
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
  timeout_per_agent: 600
  retry_on_failure: 2

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
    fn test_parse_timeout_formats() {
        let yaml = r#"
name: test
mode: mapreduce
map:
  input: test.json
  timeout_per_agent: "300s"
  agent_template:
    commands:
      - shell: "echo test"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.map.timeout_per_agent, Some(300));

        let yaml = r#"
name: test
mode: mapreduce
map:
  input: test.json
  timeout_per_agent: "5m"
  agent_template:
    commands:
      - shell: "echo test"
"#;

        let config = parse_mapreduce_workflow(yaml).unwrap();
        assert_eq!(config.map.timeout_per_agent, Some(300));
    }
}
