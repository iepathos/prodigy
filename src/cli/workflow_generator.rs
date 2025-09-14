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
    // Create a MapReduce workflow structure
    let mut agent_step = parse_command(&command.replace("${item}", "${item.path}"));
    if let Some(r) = retry {
        agent_step.max_attempts = Some(r);
    }
    if let Some(t) = timeout {
        agent_step.timeout_seconds = Some(t);
    }

    // Build the MapReduce configuration as a YAML Value
    let mut map_config = serde_yaml::Mapping::new();
    map_config.insert(
        Value::String("path".to_string()),
        Value::String(pattern.to_string()),
    );
    map_config.insert(
        Value::String("max_parallel".to_string()),
        Value::Number(parallel.into()),
    );

    // agent_template needs a commands field
    let mut agent_template_config = serde_yaml::Mapping::new();
    let commands = vec![serde_yaml::to_value(agent_step)?];
    agent_template_config.insert(
        Value::String("commands".to_string()),
        Value::Sequence(commands),
    );
    map_config.insert(
        Value::String("agent_template".to_string()),
        Value::Mapping(agent_template_config),
    );

    let mut reduce_config = serde_yaml::Mapping::new();
    let mut reduce_steps = Vec::new();
    let mut summary_step = serde_yaml::Mapping::new();
    summary_step.insert(
        Value::String("shell".to_string()),
        Value::String(
            "echo 'Batch processing complete. Processed ${map.results.length} files.'".to_string(),
        ),
    );
    reduce_steps.push(Value::Mapping(summary_step));
    reduce_config.insert(
        Value::String("steps".to_string()),
        Value::Sequence(reduce_steps),
    );

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

    let workflow = Value::Mapping(workflow_root);

    let temp_file = create_temp_workflow_yaml(&workflow)?;
    Ok((workflow, temp_file))
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
        } else {
            panic!("Expected a mapping for batch workflow");
        }
    }
}
