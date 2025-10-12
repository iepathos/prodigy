use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::process::Command;

/// Helper to initialize a git repository with initial commit
async fn setup_git_repo(path: &PathBuf) -> Result<()> {
    Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .await?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output()
        .await?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(path)
        .output()
        .await?;

    // Create initial commit
    fs::write(path.join("README.md"), "# Test Project")?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .await?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output()
        .await?;

    Ok(())
}

/// Test that MapReduce workflow can execute, merge agents, and merge back to parent
#[tokio::test]
#[ignore] // Run with --ignored flag since it requires claude CLI
async fn test_mapreduce_workflow_merge_to_parent() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().to_path_buf();

    // Setup git repository
    setup_git_repo(&repo_path).await?;

    // Create a simple workflow file with one work item
    let workflow_content = r#"
name: test-mapreduce
mode: mapreduce

setup:
  - shell: echo '["item1"]' > items.json

map:
  input: items.json
  json_path: $[*]
  max_parallel: 1
  agent_template:
    - shell: echo "Processing ${item}" > output.txt
    - shell: git add output.txt
    - shell: git commit -m "Process ${item}"

reduce:
  - shell: echo "Map phase complete"
"#;

    let workflow_path = repo_path.join("workflow.yml");
    fs::write(&workflow_path, workflow_content)?;

    // Run the workflow
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "prodigy",
            "--",
            "run",
            workflow_path.to_str().unwrap(),
        ])
        .current_dir(&repo_path)
        .output()
        .await?;

    // Check that the workflow executed successfully
    if !output.status.success() {
        eprintln!("Workflow execution failed:");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Workflow execution failed");
    }

    // Verify that changes were merged back to the repository
    // The output.txt file should exist in the repo after merge
    let output_file = repo_path.join("output.txt");
    assert!(
        output_file.exists(),
        "Output file should exist after MapReduce merge"
    );

    // Verify git log shows the commits
    let log_output = Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&repo_path)
        .output()
        .await?;

    let log = String::from_utf8_lossy(&log_output.stdout);
    assert!(
        log.contains("Process item1"),
        "Commit from agent should be in git log"
    );

    Ok(())
}
