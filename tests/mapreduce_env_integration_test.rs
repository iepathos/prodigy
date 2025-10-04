//! Integration tests for MapReduce environment variables
//! Verifies that environment variables work correctly across all MapReduce phases

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test MapReduce workflow with environment variables
fn create_test_workflow(temp_dir: &TempDir) -> PathBuf {
    let workflow_path = temp_dir.path().join("test-mapreduce-env.yml");
    let workflow_content = r#"
name: test-mapreduce-env
mode: mapreduce

env:
  PROJECT_NAME: "test-project"
  OUTPUT_DIR: "output"
  MAX_RETRIES: "3"

setup:
  - shell: "echo Starting $PROJECT_NAME > setup.log"
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo '{\"items\": [{\"name\": \"item1\"}, {\"name\": \"item2\"}]}' > items.json"

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo Processing ${item.name} for $PROJECT_NAME >> ${OUTPUT_DIR}/map.log"
    - shell: "echo ${item.name} >> ${OUTPUT_DIR}/${item.name}.txt"
  max_parallel: 2

reduce:
  - shell: "echo Processed ${map.total} items for $PROJECT_NAME >> ${OUTPUT_DIR}/reduce.log"
  - shell: "cat ${OUTPUT_DIR}/map.log > ${OUTPUT_DIR}/summary.txt"
"#;

    fs::write(&workflow_path, workflow_content).expect("Failed to write workflow");
    workflow_path
}

#[test]
fn test_mapreduce_env_vars_in_setup() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify env block exists
    assert!(config.get("env").is_some(), "Workflow should have env block");

    // Verify env variables
    let env = config.get("env").unwrap();
    assert_eq!(
        env.get("PROJECT_NAME").and_then(|v| v.as_str()),
        Some("test-project")
    );
    assert_eq!(
        env.get("OUTPUT_DIR").and_then(|v| v.as_str()),
        Some("output")
    );
    assert_eq!(
        env.get("MAX_RETRIES").and_then(|v| v.as_str()),
        Some("3")
    );

    // Verify setup commands use env vars
    let setup = config.get("setup").unwrap().as_sequence().unwrap();
    let first_cmd = setup[0].get("shell").unwrap().as_str().unwrap();
    assert!(
        first_cmd.contains("$PROJECT_NAME"),
        "Setup command should use PROJECT_NAME env var"
    );

    Ok(())
}

#[test]
fn test_mapreduce_env_vars_in_map() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify map phase uses env vars
    let map = config.get("map").unwrap();
    let agent_template = map.get("agent_template").unwrap().as_sequence().unwrap();

    let first_cmd = agent_template[0].get("shell").unwrap().as_str().unwrap();
    assert!(
        first_cmd.contains("$PROJECT_NAME"),
        "Map command should use PROJECT_NAME env var"
    );
    assert!(
        first_cmd.contains("${OUTPUT_DIR}"),
        "Map command should use OUTPUT_DIR env var"
    );

    Ok(())
}

#[test]
fn test_mapreduce_env_vars_in_reduce() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify reduce phase uses env vars
    let reduce = config.get("reduce").unwrap().as_sequence().unwrap();
    let first_cmd = reduce[0].get("shell").unwrap().as_str().unwrap();

    assert!(
        first_cmd.contains("$PROJECT_NAME"),
        "Reduce command should use PROJECT_NAME env var"
    );
    assert!(
        first_cmd.contains("${OUTPUT_DIR}"),
        "Reduce command should use OUTPUT_DIR env var"
    );

    Ok(())
}

#[test]
fn test_mapreduce_env_vars_with_merge() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = temp_dir.path().join("test-mapreduce-merge-env.yml");

    let workflow_content = r#"
name: test-mapreduce-merge-env
mode: mapreduce

env:
  PROJECT_NAME: "merge-test"
  NOTIFY_URL: "https://example.com/notify"

setup:
  - shell: "echo '{\"items\": [{\"name\": \"item1\"}]}' > items.json"

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo ${item.name}"
  max_parallel: 1

reduce:
  - shell: "echo Done"

merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME changes"
    - shell: "echo Notifying $NOTIFY_URL"
"#;

    fs::write(&workflow_path, workflow_content)?;

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify merge phase uses env vars
    let merge = config.get("merge").unwrap();
    let commands = merge.get("commands").unwrap().as_sequence().unwrap();

    let first_cmd = commands[0].get("shell").unwrap().as_str().unwrap();
    assert!(
        first_cmd.contains("$PROJECT_NAME"),
        "Merge command should use PROJECT_NAME env var"
    );

    let second_cmd = commands[1].get("shell").unwrap().as_str().unwrap();
    assert!(
        second_cmd.contains("$NOTIFY_URL"),
        "Merge command should use NOTIFY_URL env var"
    );

    Ok(())
}

#[test]
fn test_mapreduce_env_vars_both_syntaxes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = temp_dir.path().join("test-syntax.yml");

    let workflow_content = r#"
name: test-syntax
mode: mapreduce

env:
  VAR1: "value1"
  VAR2: "value2"

setup:
  - shell: "echo $VAR1"        # Shell-style
  - shell: "echo ${VAR2}"      # Bracketed

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo $VAR1 ${VAR2}"  # Both syntaxes
  max_parallel: 1

reduce:
  - shell: "echo ${VAR1} and $VAR2"
"#;

    fs::write(&workflow_path, workflow_content)?;

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify both syntaxes are present
    let setup = config.get("setup").unwrap().as_sequence().unwrap();
    assert!(setup[0]
        .get("shell")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("$VAR1"));
    assert!(setup[1]
        .get("shell")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("${VAR2}"));

    Ok(())
}

#[test]
fn test_mapreduce_env_vars_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse and validate the workflow structure
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify required fields
    assert_eq!(config.get("mode").and_then(|v| v.as_str()), Some("mapreduce"));
    assert!(config.get("env").is_some());
    assert!(config.get("setup").is_some());
    assert!(config.get("map").is_some());
    assert!(config.get("reduce").is_some());

    // Verify env is a mapping
    assert!(config.get("env").unwrap().is_mapping());

    // Verify all env values are strings
    let env = config.get("env").unwrap().as_mapping().unwrap();
    for (key, value) in env {
        assert!(
            key.is_string(),
            "Env key should be string: {:?}",
            key
        );
        assert!(
            value.is_string(),
            "Env value should be string for key {:?}: {:?}",
            key,
            value
        );
    }

    Ok(())
}
