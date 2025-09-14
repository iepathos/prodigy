use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Generate a workflow configuration for exec command
pub fn generate_exec_workflow(
    command: &str,
    retry: u32,
    timeout: Option<u64>,
) -> Result<(Vec<WorkflowStep>, PathBuf)> {
    let mut step = parse_command(command);

    // Add retry and timeout if specified
    if retry > 1 {
        step.max_attempts = Some(retry);
    }
    if let Some(t) = timeout {
        step.timeout_seconds = Some(t);
    }

    let workflow_steps = vec![step];
    let temp_file = create_temp_workflow(&workflow_steps)?;
    Ok((workflow_steps, temp_file))
}

/// Generate a workflow configuration for batch command
pub fn generate_batch_workflow(
    pattern: &str,
    command: &str,
    parallel: usize,
    retry: Option<u32>,
    timeout: Option<u64>,
) -> Result<(Value, PathBuf)> {
    // Create workflow step with functional approach
    let agent_step = create_workflow_step(command, retry, timeout);

    // Build the MapReduce configuration functionally
    let map_config = build_map_config(pattern, parallel, agent_step)?;
    let reduce_config = build_reduce_config();
    let workflow = build_mapreduce_workflow(map_config, reduce_config);

    let temp_file = create_temp_workflow_yaml(&workflow)?;
    Ok((workflow, temp_file))
}

/// Create a workflow step with optional retry and timeout
fn create_workflow_step(command: &str, retry: Option<u32>, timeout: Option<u64>) -> WorkflowStep {
    // Parse command as-is for MapReduce (${item} will be resolved during execution)
    let mut step = parse_command(command);
    step.max_attempts = retry.filter(|&r| r > 1);
    step.timeout_seconds = timeout;
    step
}

/// Build the map configuration for MapReduce
fn build_map_config(
    pattern: &str,
    parallel: usize,
    agent_step: WorkflowStep,
) -> Result<serde_yaml::Mapping> {
    let mut map_config = serde_yaml::Mapping::new();

    // Use find command to generate file list as input
    let find_command = format!("find . -name '{}'", pattern);
    map_config.insert(
        Value::String("input".to_string()),
        Value::String(find_command),
    );
    map_config.insert(
        Value::String("max_parallel".to_string()),
        Value::Number(parallel.into()),
    );

    // Build agent template with commands
    let agent_template = build_agent_template(agent_step)?;
    map_config.insert(
        Value::String("agent_template".to_string()),
        Value::Mapping(agent_template),
    );

    Ok(map_config)
}

/// Build the agent template configuration
fn build_agent_template(agent_step: WorkflowStep) -> Result<serde_yaml::Mapping> {
    let mut agent_template = serde_yaml::Mapping::new();
    let commands = vec![serde_yaml::to_value(agent_step)?];
    agent_template.insert(
        Value::String("commands".to_string()),
        Value::Sequence(commands),
    );
    Ok(agent_template)
}

/// Build the reduce configuration
fn build_reduce_config() -> serde_yaml::Mapping {
    let mut reduce_config = serde_yaml::Mapping::new();
    let summary_step = build_summary_step();
    reduce_config.insert(
        Value::String("commands".to_string()),
        Value::Sequence(vec![Value::Mapping(summary_step)]),
    );
    reduce_config
}

/// Build the summary step for reduce phase
fn build_summary_step() -> serde_yaml::Mapping {
    let mut summary_step = serde_yaml::Mapping::new();
    summary_step.insert(
        Value::String("shell".to_string()),
        Value::String(
            "echo 'Batch processing complete. Processed ${map.successful} items.'".to_string(),
        ),
    );
    summary_step
}

/// Build the complete MapReduce workflow
fn build_mapreduce_workflow(
    map_config: serde_yaml::Mapping,
    reduce_config: serde_yaml::Mapping,
) -> Value {
    let mut workflow_root = serde_yaml::Mapping::new();
    workflow_root.insert(
        Value::String("name".to_string()),
        Value::String(format!("batch-{}", uuid::Uuid::new_v4())),
    );
    workflow_root.insert(
        Value::String("mode".to_string()),
        Value::String("mapreduce".to_string()),
    );
    workflow_root.insert(Value::String("map".to_string()), Value::Mapping(map_config));
    workflow_root.insert(
        Value::String("reduce".to_string()),
        Value::Mapping(reduce_config),
    );

    Value::Mapping(workflow_root)
}

/// Parse a command string into the appropriate command type
fn parse_command(cmd: &str) -> WorkflowStep {
    let mut step = WorkflowStep::default();

    if cmd.starts_with("claude:") {
        let claude_cmd = cmd.strip_prefix("claude:").unwrap().trim();
        step.claude = Some(claude_cmd.to_string());
    } else if cmd.starts_with("shell:") {
        let shell_cmd = cmd.strip_prefix("shell:").unwrap().trim();
        step.shell = Some(shell_cmd.to_string());
    } else if cmd.starts_with('/') {
        // Assume Claude command if starts with /
        step.claude = Some(cmd.to_string());
    } else {
        // Default to shell command
        step.shell = Some(cmd.to_string());
    }

    step
}

/// Create a temporary workflow file from workflow steps
fn create_temp_workflow(steps: &[WorkflowStep]) -> Result<PathBuf> {
    let mut temp_file = NamedTempFile::with_suffix(".yml")?;
    let yaml = serde_yaml::to_string(steps)?;
    use std::io::Write;
    temp_file.write_all(yaml.as_bytes())?;

    // Convert to a persistent temporary file that we manage
    let (_, path) = temp_file.keep()?;
    Ok(path)
}

/// Create a temporary workflow file from YAML values
fn create_temp_workflow_yaml(workflow: &Value) -> Result<PathBuf> {
    let mut temp_file = NamedTempFile::with_suffix(".yml")?;
    let yaml = serde_yaml::to_string(workflow)?;
    use std::io::Write;
    temp_file.write_all(yaml.as_bytes())?;

    // Convert to a persistent temporary file that we manage
    let (_, path) = temp_file.keep()?;
    Ok(path)
}

/// Workflow step structure matching Prodigy's actual format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WorkflowStep {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_required: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_workflow: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// Cleanup temporary workflow file
pub struct TemporaryWorkflow {
    pub path: PathBuf,
}

impl Drop for TemporaryWorkflow {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_claude() {
        let step = parse_command("claude: /refactor app.py");
        assert_eq!(step.claude, Some("/refactor app.py".to_string()));
        assert_eq!(step.shell, None);
    }

    #[test]
    fn test_parse_command_shell() {
        let step = parse_command("shell: npm test");
        assert_eq!(step.shell, Some("npm test".to_string()));
        assert_eq!(step.claude, None);
    }

    #[test]
    fn test_parse_command_slash() {
        let step = parse_command("/add-types");
        assert_eq!(step.claude, Some("/add-types".to_string()));
        assert_eq!(step.shell, None);
    }

    #[test]
    fn test_parse_command_default() {
        let step = parse_command("cargo test");
        assert_eq!(step.shell, Some("cargo test".to_string()));
        assert_eq!(step.claude, None);
    }

    #[test]
    fn test_generate_exec_workflow() {
        let (workflow_steps, _path) =
            generate_exec_workflow("claude: /test", 3, Some(300)).unwrap();
        assert_eq!(workflow_steps.len(), 1);
        assert_eq!(workflow_steps[0].max_attempts, Some(3));
        assert_eq!(workflow_steps[0].timeout_seconds, Some(300));
    }

    #[test]
    fn test_generate_batch_workflow() {
        let (workflow_value, _path) =
            generate_batch_workflow("*.py", "claude: /add-types", 5, Some(2), Some(60)).unwrap();
        // Test that it's a mapreduce workflow
        if let Value::Mapping(ref map) = workflow_value {
            assert!(map.contains_key(&Value::String("name".to_string())));
            assert!(map.contains_key(&Value::String("mode".to_string())));
            assert!(map.contains_key(&Value::String("map".to_string())));
            assert!(map.contains_key(&Value::String("reduce".to_string())));

            // Verify map config has input field
            if let Some(Value::Mapping(map_config)) = map.get(&Value::String("map".to_string())) {
                assert!(map_config.contains_key(&Value::String("input".to_string())));
                // Check that input contains the find command
                if let Some(Value::String(input)) =
                    map_config.get(&Value::String("input".to_string()))
                {
                    assert!(input.contains("find"));
                    assert!(input.contains("*.py"));
                }
            }
        } else {
            panic!("Expected a mapping for batch workflow");
        }
    }
}
