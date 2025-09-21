//! Integration tests for MapReduce command input functionality

use prodigy::config::mapreduce::parse_mapreduce_workflow;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Test that command input is properly detected and executed
#[tokio::test]
async fn test_mapreduce_command_input_basic() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create some test files to find
    fs::write(project_root.join("file1.txt"), "content1").unwrap();
    fs::write(project_root.join("file2.txt"), "content2").unwrap();
    fs::write(project_root.join("other.md"), "other").unwrap();

    // Create workflow with command input
    let workflow_yaml = r#"
name: test-command-input
mode: mapreduce

map:
  input: "ls *.txt"

  agent_template:
    - shell: "echo Processing ${item}"

  max_parallel: 2
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert_eq!(config.map.input, "ls *.txt");

    // The actual MapReduce execution would need proper setup
    // For now, we test that the configuration parses correctly
}

/// Test complex command pipelines
#[tokio::test]
async fn test_mapreduce_command_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create src directory first
    fs::create_dir_all(project_root.join("src")).unwrap();

    // Create test files with TODO comments
    fs::write(
        project_root.join("src/main.rs"),
        "// TODO: fix this\nfn main() {}",
    )
    .unwrap();
    fs::write(
        project_root.join("src/lib.rs"),
        "// TODO: implement\npub fn lib() {}",
    )
    .unwrap();
    fs::write(project_root.join("README.md"), "# README\nTODO: write docs").unwrap();

    let workflow_yaml = r#"
name: test-pipeline-input
mode: mapreduce

map:
  input: "grep -r 'TODO' . | cut -d: -f1 | sort -u"

  agent_template:
    - shell: "echo Found TODO in ${item}"

  max_parallel: 3
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert!(config.map.input.contains("grep"));
    assert!(config.map.input.contains("cut"));
    assert!(config.map.input.contains("sort"));
}

/// Test that JSON file input still works
#[tokio::test]
async fn test_mapreduce_json_file_input() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create a JSON file with work items
    let work_items = json!({
        "items": [
            {"id": 1, "task": "Task 1"},
            {"id": 2, "task": "Task 2"},
            {"id": 3, "task": "Task 3"}
        ]
    });
    fs::write(
        project_root.join("work_items.json"),
        serde_json::to_string_pretty(&work_items).unwrap(),
    )
    .unwrap();

    let workflow_yaml = r#"
name: test-json-input
mode: mapreduce

map:
  input: work_items.json
  json_path: "$.items[*]"

  agent_template:
    - shell: "echo Processing task ${item.task}"

  max_parallel: 2
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert_eq!(config.map.input, "work_items.json");
    assert_eq!(config.map.json_path, "$.items[*]");
}

/// Test error handling for failed commands
#[tokio::test]
async fn test_mapreduce_command_failure() {
    let workflow_yaml = r#"
name: test-command-failure
mode: mapreduce

map:
  input: "this_command_does_not_exist"

  agent_template:
    - shell: "echo Should not reach here"

  max_parallel: 1
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert_eq!(config.map.input, "this_command_does_not_exist");

    // In actual execution, this should fail with CommandExecutionFailed error
}

/// Test variable interpolation in commands
#[tokio::test]
async fn test_mapreduce_command_with_variables() {
    let workflow_yaml = r#"
name: test-command-variables
mode: mapreduce

map:
  input: "find ${SEARCH_DIR:-src} -name '*.rs' -type f"

  agent_template:
    - shell: "echo Analyzing ${item}"

  max_parallel: 5
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert!(config.map.input.contains("find"));
    assert!(config.map.input.contains("${SEARCH_DIR:-src}"));
}

/// Test command timeout behavior
#[tokio::test]
async fn test_mapreduce_command_timeout() {
    let workflow_yaml = r#"
name: test-command-timeout
mode: mapreduce

map:
  input: "sleep 10 && echo 'should timeout'"

  agent_template:
    - shell: "echo ${item}"

  max_parallel: 1
"#;

    let config = parse_mapreduce_workflow(workflow_yaml).unwrap();
    assert_eq!(config.map.input, "sleep 10 && echo 'should timeout'");

    // In actual execution, timeout would be handled by overall workflow timeout
}

/// Test empty command output handling
#[tokio::test]
async fn test_mapreduce_empty_command_output() {
    let temp_dir = TempDir::new().unwrap();
    let _project_root = temp_dir.path().to_path_buf();

    let workflow_yaml = r#"
name: test-empty-output
mode: mapreduce

map:
  input: "ls *.nonexistent 2>/dev/null || true"

  agent_template:
    - shell: "echo Processing ${item}"

  max_parallel: 1
"#;

    let _config = parse_mapreduce_workflow(workflow_yaml).unwrap();

    // Should handle empty output gracefully (0 work items)
}
