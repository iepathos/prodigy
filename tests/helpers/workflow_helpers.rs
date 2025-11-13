//! Helper utilities for creating test workflows

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Create a simple workflow file for testing
pub async fn create_simple_workflow(dir: &Path) -> Result<PathBuf> {
    let workflow_path = dir.join("workflow.yml");
    let content = r#"
name: test-workflow
commands:
  - shell: echo "Step 1"
  - shell: echo "Step 2"
  - shell: echo "Step 3"
"#;
    tokio::fs::write(&workflow_path, content).await?;
    Ok(workflow_path)
}

/// Create a workflow that fails at a specific step
pub async fn create_failing_workflow(dir: &Path, fail_at_step: usize) -> Result<PathBuf> {
    let workflow_path = dir.join("failing_workflow.yml");

    let mut commands = Vec::new();
    for i in 0..5 {
        if i == fail_at_step {
            commands.push(format!("  - shell: exit 1  # Fail at step {}", i));
        } else {
            commands.push(format!("  - shell: echo \"Step {}\"", i));
        }
    }

    let content = format!(
        r#"
name: failing-workflow
commands:
{}
"#,
        commands.join("\n")
    );

    tokio::fs::write(&workflow_path, content).await?;
    Ok(workflow_path)
}

/// Create a MapReduce workflow for testing
pub async fn create_mapreduce_workflow(
    dir: &Path,
    num_items: usize,
) -> Result<(PathBuf, PathBuf)> {
    let workflow_path = dir.join("mapreduce_workflow.yml");
    let items_path = dir.join("items.json");

    // Create work items
    let items: Vec<serde_json::Value> = (0..num_items)
        .map(|i| {
            serde_json::json!({
                "id": format!("item-{}", i),
                "value": i,
            })
        })
        .collect();

    tokio::fs::write(&items_path, serde_json::to_string_pretty(&items)?).await?;

    // Create workflow
    let workflow_content = format!(
        r#"
name: mapreduce-test
mode: mapreduce

setup:
  - shell: echo "Setup complete"

map:
  input: "{}"
  json_path: "$[*]"
  max_parallel: 2

  agent_template:
    - shell: echo "Processing ${{item.id}}"

reduce:
  - shell: echo "Reduce complete"
"#,
        items_path.display()
    );

    tokio::fs::write(&workflow_path, workflow_content).await?;
    Ok((workflow_path, items_path))
}

/// Create a workflow with environment variables
pub async fn create_workflow_with_env(dir: &Path) -> Result<PathBuf> {
    let workflow_path = dir.join("env_workflow.yml");
    let content = r#"
name: env-workflow

env:
  TEST_VAR: "original_value"
  API_KEY:
    secret: true
    value: "secret123"

commands:
  - shell: echo "TEST_VAR=$TEST_VAR"
  - shell: echo "API_KEY=$API_KEY"
"#;
    tokio::fs::write(&workflow_path, content).await?;
    Ok(workflow_path)
}

/// Modify a workflow file to change its hash
pub async fn modify_workflow_file(workflow_path: &Path) -> Result<()> {
    let mut content = tokio::fs::read_to_string(workflow_path).await?;
    content.push_str("\n# Modified\n");
    tokio::fs::write(workflow_path, content).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_simple_workflow() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workflow_path = create_simple_workflow(temp_dir.path()).await?;

        assert!(workflow_path.exists());
        let content = tokio::fs::read_to_string(&workflow_path).await?;
        assert!(content.contains("test-workflow"));
        assert!(content.contains("Step 1"));

        Ok(())
    }

    #[tokio::test]
    async fn test_create_failing_workflow() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workflow_path = create_failing_workflow(temp_dir.path(), 2).await?;

        assert!(workflow_path.exists());
        let content = tokio::fs::read_to_string(&workflow_path).await?;
        assert!(content.contains("exit 1"));
        assert!(content.contains("Fail at step 2"));

        Ok(())
    }

    #[tokio::test]
    async fn test_create_mapreduce_workflow() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let (workflow_path, items_path) = create_mapreduce_workflow(temp_dir.path(), 5).await?;

        assert!(workflow_path.exists());
        assert!(items_path.exists());

        let items_content = tokio::fs::read_to_string(&items_path).await?;
        let items: Vec<serde_json::Value> = serde_json::from_str(&items_content)?;
        assert_eq!(items.len(), 5);

        Ok(())
    }
}
