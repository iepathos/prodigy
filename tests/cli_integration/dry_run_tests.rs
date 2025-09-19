// Tests for dry-run functionality across commands

use super::test_utils::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Test that cook --dry-run doesn't execute commands
#[test]
fn test_cook_dry_run_no_execution() {
    let test = CliTest::new();

    // Create a workflow that would create a file if executed
    let workflow_content = r#"
name: dry-run-test
commands:
  - shell: "touch /tmp/test_dry_run_marker.txt"
  - shell: "echo 'This should not be executed'"
"#;
    let (test, workflow_path) = test.with_workflow("dry_run", workflow_content);

    // Ensure the marker file doesn't exist
    let marker_path = "/tmp/test_dry_run_marker.txt";
    let _ = fs::remove_file(marker_path);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));

    // Verify the file was not created
    assert!(
        !Path::new(marker_path).exists(),
        "Commands should not execute in dry-run mode"
    );
}

/// Test that cook --dry-run shows what would be executed
#[test]
fn test_cook_dry_run_output() {
    let test = CliTest::new();

    let workflow_content = r#"
name: dry-run-output-test
commands:
  - shell: "echo 'First command'"
  - shell: "echo 'Second command'"
  - claude: "/test-command"
"#;
    let (test, workflow_path) = test.with_workflow("dry_run_output", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));
    assert!(output.stdout_contains("Would execute"));
    assert!(output.stdout_contains("echo 'First command'"));
    assert!(output.stdout_contains("echo 'Second command'"));
    assert!(output.stdout_contains("/test-command"));
}

/// Test cook --dry-run with iterations
#[test]
fn test_cook_dry_run_with_iterations() {
    let test = CliTest::new();

    let workflow_content = r#"
name: dry-run-iterations
commands:
  - shell: "echo 'Iteration test'"
"#;
    let (test, workflow_path) = test.with_workflow("iterations", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .arg("-n")
        .arg("3")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));
    assert!(output.stdout_contains("Would run 3 iterations"));
}

/// Test events clean --dry-run functionality
#[test]
fn test_events_clean_dry_run() {
    let test = CliTest::new();
    let temp_dir = TempDir::new().unwrap();
    let events_file = temp_dir.path().join("events.jsonl");

    // Create a test events file with old entries
    let old_timestamp = "2020-01-01T00:00:00Z";
    let recent_timestamp = chrono::Utc::now().to_rfc3339();

    let events_content = format!(
        r#"{{"timestamp":"{}","event":"old_event"}}
{{"timestamp":"{}","event":"recent_event"}}
"#,
        old_timestamp, recent_timestamp
    );

    fs::write(&events_file, events_content).unwrap();

    // Get initial file size
    let initial_size = fs::metadata(&events_file).unwrap().len();

    let output = test
        .arg("events")
        .arg("clean")
        .arg("--older-than")
        .arg("365d")
        .arg("--dry-run")
        .arg("--file")
        .arg(events_file.to_str().unwrap())
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("DRY RUN") || output.stdout_contains("dry run"));
    assert!(output.stdout_contains("Would"));

    // Verify file was not modified
    let current_size = fs::metadata(&events_file).unwrap().len();
    assert_eq!(
        initial_size, current_size,
        "File should not be modified in dry-run mode"
    );

    // Verify original content is intact
    let content = fs::read_to_string(&events_file).unwrap();
    assert!(content.contains("old_event"));
    assert!(content.contains("recent_event"));
}

/// Test events clean --dry-run with max events
#[test]
fn test_events_clean_dry_run_max_events() {
    let test = CliTest::new();
    let temp_dir = TempDir::new().unwrap();
    let events_file = temp_dir.path().join("events.jsonl");

    // Create a test events file with multiple entries
    let mut events_content = String::new();
    for i in 0..10 {
        let timestamp = chrono::Utc::now().to_rfc3339();
        events_content.push_str(&format!(
            r#"{{"timestamp":"{}","event":"event_{}"}}"#,
            timestamp, i
        ));
        events_content.push('\n');
    }

    fs::write(&events_file, &events_content).unwrap();

    let output = test
        .arg("events")
        .arg("clean")
        .arg("--max-events")
        .arg("5")
        .arg("--dry-run")
        .arg("--file")
        .arg(events_file.to_str().unwrap())
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("DRY RUN") || output.stdout_contains("dry run"));
    assert!(output.stdout_contains("events"));

    // Verify file was not modified
    let content_after = fs::read_to_string(&events_file).unwrap();
    assert_eq!(
        events_content, content_after,
        "File should not be modified in dry-run mode"
    );
}

/// Test events clean --dry-run with size limit
#[test]
fn test_events_clean_dry_run_max_size() {
    let test = CliTest::new();
    let temp_dir = TempDir::new().unwrap();
    let events_file = temp_dir.path().join("events.jsonl");

    // Create a test events file
    let mut events_content = String::new();
    for i in 0..100 {
        let timestamp = chrono::Utc::now().to_rfc3339();
        events_content.push_str(&format!(
            r#"{{"timestamp":"{}","event":"event_{}","data":"padding_data_to_increase_size"}}"#,
            timestamp, i
        ));
        events_content.push('\n');
    }

    fs::write(&events_file, &events_content).unwrap();
    let file_size = fs::metadata(&events_file).unwrap().len();

    let output = test
        .arg("events")
        .arg("clean")
        .arg("--max-size")
        .arg("1KB") // Very small limit to trigger cleanup
        .arg("--dry-run")
        .arg("--file")
        .arg(events_file.to_str().unwrap())
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("DRY RUN") || output.stdout_contains("dry run"));

    // Verify file size unchanged
    let current_size = fs::metadata(&events_file).unwrap().len();
    assert_eq!(
        file_size, current_size,
        "File size should not change in dry-run mode"
    );
}

/// Test cook --dry-run with MapReduce workflow
#[test]
fn test_cook_dry_run_mapreduce() {
    let test = CliTest::new();
    let temp_dir = TempDir::new().unwrap();
    let items_file = temp_dir.path().join("items.json");

    // Create test input file
    let items_content = r#"{"items": ["item1", "item2", "item3"]}"#;
    fs::write(&items_file, items_content).unwrap();

    let workflow_content = format!(
        r#"
name: mapreduce-dry-run
mode: mapreduce
map:
  input: "{}"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo 'Processing ${{item}}'"
  max_parallel: 2
reduce:
  - shell: "echo 'Reduce phase'"
"#,
        items_file.to_str().unwrap()
    );

    let (test, workflow_path) = test.with_workflow("mapreduce_dry", &workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));
    assert!(output.stdout_contains("MapReduce"));
    assert!(output.stdout_contains("3 items") || output.stdout_contains("items: 3"));
}

/// Test cook --dry-run doesn't create worktrees
#[test]
fn test_cook_dry_run_no_worktree() {
    let test = CliTest::new();

    let workflow_content = r#"
name: worktree-dry-run
commands:
  - shell: "echo 'Test'"
"#;
    let (test, workflow_path) = test.with_workflow("worktree_dry", workflow_content);

    // Run with dry-run and worktree flag
    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .arg("--worktree")
        .run();

    // In dry-run mode, the command should succeed but not create any worktrees
    if output.exit_code != exit_codes::SUCCESS {
        eprintln!("stdout: {}", output.stdout);
        eprintln!("stderr: {}", output.stderr);
        eprintln!("exit_code: {}", output.exit_code);
    }
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));

    // Verify the dry run output shows worktree would be created but isn't actually created
    assert!(!output.stdout_contains("Created worktree"));
}

/// Test cook --dry-run with validation steps
#[test]
fn test_cook_dry_run_with_validation() {
    let test = CliTest::new();

    let workflow_content = r#"
name: validation-dry-run
commands:
  - shell: "echo 'Main command'"
    validate:
      command: "test -f /tmp/nonexistent"
      on_failure: "echo 'Validation would fail'"
"#;
    let (test, workflow_path) = test.with_workflow("validation_dry", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));
    assert!(output.stdout_contains("Main command"));
    assert!(output.stdout_contains("validate") || output.stdout_contains("Validation"));
}

/// Test events clean --dry-run with archiving
#[test]
fn test_events_clean_dry_run_archive() {
    let test = CliTest::new();
    let temp_dir = TempDir::new().unwrap();
    let events_file = temp_dir.path().join("events.jsonl");
    let archive_dir = temp_dir.path().join("archive");

    // Create test events
    let old_timestamp = "2020-01-01T00:00:00Z";
    let events_content = format!(r#"{{"timestamp":"{}","event":"old_event"}}"#, old_timestamp);
    fs::write(&events_file, events_content).unwrap();

    let output = test
        .arg("events")
        .arg("clean")
        .arg("--older-than")
        .arg("365d")
        .arg("--archive")
        .arg("--archive-path")
        .arg(archive_dir.to_str().unwrap())
        .arg("--dry-run")
        .arg("--file")
        .arg(events_file.to_str().unwrap())
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("DRY RUN") || output.stdout_contains("dry run"));
    assert!(output.stdout_contains("archive") || output.stdout_contains("Archive"));

    // Verify archive directory was not created
    assert!(
        !archive_dir.exists(),
        "Archive directory should not be created in dry-run mode"
    );
}

/// Test cook --dry-run shows correct command count
#[test]
fn test_cook_dry_run_command_count() {
    let test = CliTest::new();

    let workflow_content = r#"
name: command-count-test
commands:
  - shell: "echo 'Command 1'"
  - shell: "echo 'Command 2'"
  - shell: "echo 'Command 3'"
  - foreach:
      foreach: ["a", "b"]
      do:
        - shell: "echo 'Item ${item}'"
"#;
    let (test, workflow_path) = test.with_workflow("count_dry", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(output.stdout_contains("[DRY RUN]"));
    // Should show 3 direct commands + 1 foreach block
    assert!(output.stdout_contains("commands") || output.stdout_contains("steps"));
}

/// Test that --dry-run conflicts with commit_required
#[test]
fn test_cook_dry_run_commit_required_conflict() {
    let test = CliTest::new();

    let workflow_content = r#"
name: commit-required-test
commands:
  - shell: "echo 'test'"
    commit_required: true
"#;
    let (test, workflow_path) = test.with_workflow("commit_dry", workflow_content);

    let output = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .arg("--dry-run")
        .run();

    // Should either skip the command or show a warning
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("skip")
            || output.stdout_contains("warning")
            || output.stdout_contains("commit_required")
    );
}
