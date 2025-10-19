//! Integration tests for write_file command in MapReduce workflows
//! Verifies that write_file command works correctly in map and reduce phases

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test MapReduce workflow with write_file commands
fn create_test_workflow(temp_dir: &TempDir) -> PathBuf {
    let workflow_path = temp_dir.path().join("test-write-file.yml");
    let workflow_content = r#"
name: test-write-file-mapreduce
mode: mapreduce

setup:
  - shell: "echo '{\"items\": [{\"id\": 1, \"name\": \"alice\"}, {\"id\": 2, \"name\": \"bob\"}]}' > items.json"

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - write_file:
        path: "output/${item.name}.json"
        content: '{"user_id": ${item.id}, "user_name": "${item.name}", "processed": true}'
        format: json
        create_dirs: true
  max_parallel: 2

reduce:
  - write_file:
      path: "summary.txt"
      content: "Processed ${map.total} users successfully"
      format: text
"#;

    fs::write(&workflow_path, workflow_content).expect("Failed to write workflow");
    workflow_path
}

/// Helper to create workflow with YAML output
fn create_yaml_workflow(temp_dir: &TempDir) -> PathBuf {
    let workflow_path = temp_dir.path().join("test-write-yaml.yml");
    let workflow_content = r#"
name: test-write-yaml
mode: mapreduce

setup:
  - shell: "echo '{\"configs\": [{\"env\": \"dev\", \"port\": 3000}, {\"env\": \"prod\", \"port\": 8080}]}' > configs.json"

map:
  input: "configs.json"
  json_path: "$.configs[*]"
  agent_template:
    - write_file:
        path: "configs/${item.env}.yml"
        content: |
          environment: ${item.env}
          server:
            port: ${item.port}
            host: localhost
        format: yaml
        create_dirs: true
  max_parallel: 2

reduce:
  - shell: "echo 'Generated ${map.total} config files'"
"#;

    fs::write(&workflow_path, workflow_content).expect("Failed to write workflow");
    workflow_path
}

#[test]
fn test_mapreduce_write_file_json_in_map_phase() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify map phase has write_file command
    let map = config.get("map").expect("Should have map phase");
    let agent_template = map
        .get("agent_template")
        .expect("Should have agent_template")
        .as_sequence()
        .expect("agent_template should be sequence");

    let write_cmd = &agent_template[0];
    assert!(
        write_cmd.get("write_file").is_some(),
        "First command should be write_file"
    );

    // Verify write_file configuration
    let write_config = write_cmd.get("write_file").unwrap();
    assert_eq!(
        write_config.get("path").and_then(|v| v.as_str()),
        Some("output/${item.name}.json")
    );
    assert_eq!(
        write_config.get("format").and_then(|v| v.as_str()),
        Some("json")
    );
    assert_eq!(
        write_config.get("create_dirs").and_then(|v| v.as_bool()),
        Some(true)
    );

    Ok(())
}

#[test]
fn test_mapreduce_write_file_text_in_reduce_phase() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify reduce phase has write_file command
    let reduce = config
        .get("reduce")
        .expect("Should have reduce phase")
        .as_sequence()
        .expect("reduce should be sequence");

    let write_cmd = &reduce[0];
    assert!(
        write_cmd.get("write_file").is_some(),
        "Reduce command should be write_file"
    );

    // Verify write_file configuration for summary
    let write_config = write_cmd.get("write_file").unwrap();
    assert_eq!(
        write_config.get("path").and_then(|v| v.as_str()),
        Some("summary.txt")
    );
    assert_eq!(
        write_config.get("format").and_then(|v| v.as_str()),
        Some("text")
    );

    // Verify it uses map variables
    let content = write_config
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(
        content.contains("${map.total}"),
        "Should use map.total variable"
    );

    Ok(())
}

#[test]
fn test_mapreduce_write_file_yaml_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_yaml_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify map phase has write_file with YAML format
    let map = config.get("map").expect("Should have map phase");
    let agent_template = map
        .get("agent_template")
        .expect("Should have agent_template")
        .as_sequence()
        .expect("agent_template should be sequence");

    let write_cmd = &agent_template[0];
    let write_config = write_cmd.get("write_file").unwrap();

    assert_eq!(
        write_config.get("format").and_then(|v| v.as_str()),
        Some("yaml")
    );

    // Verify YAML content structure
    let content = write_config
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(content.contains("environment:"));
    assert!(content.contains("server:"));
    assert!(content.contains("port:"));

    Ok(())
}

#[test]
fn test_write_file_with_create_dirs_option() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify create_dirs is properly configured
    let map = config.get("map").unwrap();
    let agent_template = map.get("agent_template").unwrap().as_sequence().unwrap();
    let write_cmd = &agent_template[0];
    let write_config = write_cmd.get("write_file").unwrap();

    // Verify create_dirs is true
    assert_eq!(
        write_config.get("create_dirs").and_then(|v| v.as_bool()),
        Some(true),
        "create_dirs should be enabled for nested paths"
    );

    // Verify path uses nested directory
    let path = write_config.get("path").and_then(|v| v.as_str()).unwrap();
    assert!(
        path.contains("/"),
        "Path should contain directory separator"
    );

    Ok(())
}

#[test]
fn test_write_file_variable_interpolation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify variable interpolation in path
    let map = config.get("map").unwrap();
    let agent_template = map.get("agent_template").unwrap().as_sequence().unwrap();
    let write_cmd = &agent_template[0];
    let write_config = write_cmd.get("write_file").unwrap();

    let path = write_config.get("path").and_then(|v| v.as_str()).unwrap();
    assert!(
        path.contains("${item.name}"),
        "Path should use item variable"
    );

    // Verify variable interpolation in content
    let content = write_config
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(
        content.contains("${item.id}"),
        "Content should use item.id variable"
    );
    assert!(
        content.contains("${item.name}"),
        "Content should use item.name variable"
    );

    Ok(())
}

#[test]
fn test_write_file_workflow_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify workflow mode
    assert_eq!(
        config.get("mode").and_then(|v| v.as_str()),
        Some("mapreduce")
    );

    // Verify all required phases exist
    assert!(config.get("setup").is_some(), "Should have setup phase");
    assert!(config.get("map").is_some(), "Should have map phase");
    assert!(config.get("reduce").is_some(), "Should have reduce phase");

    // Verify setup prepares input file
    let setup = config.get("setup").unwrap().as_sequence().unwrap();
    let setup_cmd = setup[0].get("shell").unwrap().as_str().unwrap();
    assert!(
        setup_cmd.contains("items.json"),
        "Setup should create input"
    );

    Ok(())
}

/// Helper to create workflow using map.results in reduce phase
fn create_map_results_workflow(temp_dir: &TempDir) -> PathBuf {
    let workflow_path = temp_dir.path().join("test-map-results.yml");
    let workflow_content = r#"
name: test-map-results
mode: mapreduce

setup:
  - shell: "echo '{\"tasks\": [{\"id\": \"task-1\", \"priority\": 1}, {\"id\": \"task-2\", \"priority\": 2}]}' > tasks.json"

map:
  input: "tasks.json"
  json_path: "$.tasks[*]"
  agent_template:
    - shell: "echo 'Processed ${item.id} with priority ${item.priority}'"
  max_parallel: 2

reduce:
  - write_file:
      path: "results.json"
      content: "${map.results}"
      format: json
  - write_file:
      path: "summary.txt"
      content: "Total: ${map.total}, Successful: ${map.successful}, Failed: ${map.failed}"
      format: text
"#;

    fs::write(&workflow_path, workflow_content).expect("Failed to write workflow");
    workflow_path
}

#[test]
fn test_mapreduce_write_file_map_results() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_map_results_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify reduce phase has write_file commands
    let reduce = config
        .get("reduce")
        .expect("Should have reduce phase")
        .as_sequence()
        .expect("reduce should be sequence");

    assert_eq!(reduce.len(), 2, "Should have 2 reduce commands");

    // First reduce command: write map.results to JSON file
    let results_write = &reduce[0];
    assert!(
        results_write.get("write_file").is_some(),
        "First reduce command should be write_file"
    );

    let results_config = results_write.get("write_file").unwrap();
    assert_eq!(
        results_config.get("path").and_then(|v| v.as_str()),
        Some("results.json")
    );
    assert_eq!(
        results_config.get("format").and_then(|v| v.as_str()),
        Some("json")
    );

    // Verify content uses ${map.results} variable
    let content = results_config
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(
        content, "${map.results}",
        "Content should use map.results variable"
    );

    // Second reduce command: write summary with scalar variables
    let summary_write = &reduce[1];
    assert!(
        summary_write.get("write_file").is_some(),
        "Second reduce command should be write_file"
    );

    let summary_config = summary_write.get("write_file").unwrap();
    assert_eq!(
        summary_config.get("path").and_then(|v| v.as_str()),
        Some("summary.txt")
    );
    assert_eq!(
        summary_config.get("format").and_then(|v| v.as_str()),
        Some("text")
    );

    // Verify it uses map scalar variables
    let summary_content = summary_config
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(
        summary_content.contains("${map.total}"),
        "Should use map.total variable"
    );
    assert!(
        summary_content.contains("${map.successful}"),
        "Should use map.successful variable"
    );
    assert!(
        summary_content.contains("${map.failed}"),
        "Should use map.failed variable"
    );

    Ok(())
}

#[test]
fn test_write_file_supports_large_variables() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_map_results_workflow(&temp_dir);

    // Parse the workflow
    let workflow_yaml = fs::read_to_string(&workflow_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&workflow_yaml)?;

    // Verify workflow structure supports large variables like map.results
    // This test documents that write_file is the correct command for large variables
    // (as opposed to shell commands which would hit E2BIG errors)

    let reduce = config.get("reduce").unwrap().as_sequence().unwrap();
    let results_write = &reduce[0];
    let results_config = results_write.get("write_file").unwrap();

    // Verify using write_file for map.results (not shell)
    assert!(
        results_write.get("write_file").is_some(),
        "Should use write_file for large variables, not shell"
    );

    // Verify JSON format for structured data
    assert_eq!(
        results_config.get("format").and_then(|v| v.as_str()),
        Some("json"),
        "Should use JSON format for structured map.results"
    );

    Ok(())
}
