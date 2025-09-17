//! Tests for enhanced variable interpolation system

#[cfg(test)]
mod tests {
    use super::super::variables::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mapreduce_aggregate_variables() {
        let mut context = VariableContext::new();

        // Set up MapReduce aggregate variables
        context.set_phase(
            "map",
            Variable::Static(json!({
                "total": 100,
                "successful": 85,
                "failed": 15,
                "skipped": 0,
                "duration": 120.5,
                "success_rate": 85.0
            })),
        );

        // Test interpolation of aggregate variables
        let template = "Processed ${map.total} items: ${map.successful} successful (${map.success_rate}% success rate), ${map.failed} failed in ${map.duration} seconds";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(
            result,
            "Processed 100 items: 85 successful (85.0% success rate), 15 failed in 120.5 seconds"
        );
    }

    #[tokio::test]
    async fn test_phase_variables() {
        let mut context = VariableContext::new();

        // Set up setup phase variables
        context.set_phase(
            "setup",
            Variable::Static(json!({
                "output": "Files analyzed: 50",
                "variables": {
                    "project_root": "/home/project",
                    "config_file": "config.yaml"
                }
            })),
        );

        // Set workflow metadata
        context.set_global(
            "workflow",
            Variable::Static(json!({
                "name": "refactoring-workflow",
                "id": "wf-123456",
                "start_time": "2024-01-15T10:00:00Z"
            })),
        );

        // Test cross-phase variable access
        let template = "Workflow ${workflow.name} (${workflow.id}) - ${setup.output}";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(
            result,
            "Workflow refactoring-workflow (wf-123456) - Files analyzed: 50"
        );
    }

    #[tokio::test]
    async fn test_item_context_variables() {
        let mut context = VariableContext::new();

        // Set up item context for MapReduce agent
        context.set_local(
            "item",
            Variable::Static(json!({
                "id": "file-001",
                "path": "src/main.rs",
                "index": 0,
                "attempt": 2
            })),
        );

        context.set_local(
            "agent",
            Variable::Static(json!({
                "id": "agent-001",
                "worktree": "<test-worktree-path>"
            })),
        );

        let template = "Processing ${item.path} (attempt ${item.attempt}) in ${agent.worktree}";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(
            result,
            "Processing src/main.rs (attempt 2) in <test-worktree-path>"
        );
    }

    #[tokio::test]
    async fn test_environment_variables() {
        // Set an environment variable for testing
        std::env::set_var("TEST_PRODIGY_VAR", "test_value_123");

        let context = VariableContext::new();
        let template = "Environment value: ${env.TEST_PRODIGY_VAR}";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(result, "Environment value: test_value_123");

        // Clean up
        std::env::remove_var("TEST_PRODIGY_VAR");
    }

    #[tokio::test]
    async fn test_file_content_variable() {
        use std::fs;
        use tempfile::NamedTempFile;

        // Create a temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();
        fs::write(file_path, "Hello from file!").unwrap();

        let context = VariableContext::new();
        let template = format!("File content: ${{file:{}}}", file_path);
        let result = context.interpolate(&template).await.unwrap();

        assert_eq!(result, "File content: Hello from file!");
    }

    #[tokio::test]
    async fn test_command_output_variable() {
        let context = VariableContext::new();
        let template = "Command output: ${cmd:echo 'Hello from command'}";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(result, "Command output: Hello from command");
    }

    #[tokio::test]
    async fn test_date_formatting() {
        let context = VariableContext::new();

        // Test basic date format (year)
        let template = "Year: ${date:%Y}";
        let result = context.interpolate(template).await.unwrap();

        // Check that it's a 4-digit year
        assert!(result.starts_with("Year: 20"));
        assert_eq!(result.len(), "Year: 2024".len());
    }

    #[tokio::test]
    async fn test_uuid_generation() {
        let context = VariableContext::new();

        // Generate two UUIDs
        let template = "UUID: ${uuid}";
        let result1 = context.interpolate(template).await.unwrap();
        let result2 = context.interpolate(template).await.unwrap();

        // They should be different
        assert_ne!(result1, result2);

        // They should have UUID format
        assert!(result1.starts_with("UUID: "));
        assert!(result1.len() > "UUID: ".len() + 30); // UUIDs are 36 chars
    }

    #[tokio::test]
    async fn test_variable_scoping() {
        let mut context = VariableContext::new();

        // Set same variable at different scope levels
        context.set_global("test_var", Variable::Static(json!("global_value")));
        context.set_phase("test_var", Variable::Static(json!("phase_value")));
        context.set_local("test_var", Variable::Static(json!("local_value")));

        // Local should take precedence
        let result = context.interpolate("Value: ${test_var}").await.unwrap();
        assert_eq!(result, "Value: local_value");

        // Remove local variable
        context.remove_local("test_var");

        // Phase should take precedence
        let result = context.interpolate("Value: ${test_var}").await.unwrap();
        assert_eq!(result, "Value: phase_value");

        // Remove phase variable
        context.remove_phase("test_var");

        // Global should be used
        let result = context.interpolate("Value: ${test_var}").await.unwrap();
        assert_eq!(result, "Value: global_value");
    }

    #[tokio::test]
    async fn test_nested_json_path() {
        let mut context = VariableContext::new();

        context.set_global(
            "results",
            Variable::Static(json!({
                "summary": {
                    "total": 50,
                    "details": {
                        "passed": 45,
                        "failed": 5
                    }
                },
                "items": ["item1", "item2", "item3"]
            })),
        );

        // Test nested path access
        let template = "Total: ${results.summary.total}, Passed: ${results.summary.details.passed}";
        let result = context.interpolate(template).await.unwrap();

        assert_eq!(result, "Total: 50, Passed: 45");
    }

    #[tokio::test]
    async fn test_array_handling() {
        let mut context = VariableContext::new();

        // String array should be comma-separated
        context.set_global(
            "tags",
            Variable::Static(json!(["bug", "urgent", "backend"])),
        );
        let result = context.interpolate("Tags: ${tags}").await.unwrap();
        assert_eq!(result, "Tags: bug, urgent, backend");

        // Mixed array should be JSON
        context.set_global("mixed", Variable::Static(json!(["string", 123, true])));
        let result = context.interpolate("Mixed: ${mixed}").await.unwrap();
        assert_eq!(result, r#"Mixed: ["string",123,true]"#);
    }

    #[tokio::test]
    async fn test_variable_persistence() {
        let mut context = VariableContext::new();

        // Set various variables
        context.set_global("global_var", Variable::Static(json!("global_value")));
        context.set_phase("phase_var", Variable::Static(json!("phase_value")));
        context.set_local("local_var", Variable::Static(json!("local_value")));

        // Export variables
        let exported = context.export();

        // Check that all variables are exported with scope prefixes
        assert!(exported.contains_key("global.global_var"));
        assert!(exported.contains_key("phase.phase_var"));
        assert!(exported.contains_key("local.local_var"));

        // Create new context and import
        let mut new_context = VariableContext::new();
        new_context.import(exported);

        // Verify variables are restored
        let result = new_context
            .interpolate("${global_var} ${phase_var} ${local_var}")
            .await
            .unwrap();
        assert_eq!(result, "global_value phase_value local_value");
    }

    #[tokio::test]
    async fn test_undefined_variable_error() {
        let context = VariableContext::new();

        // Attempting to use undefined variable should error
        let result = context.interpolate("Value: ${undefined_var}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_complex_mapreduce_scenario() {
        let mut context = VariableContext::new();

        // Simulate a complete MapReduce context
        context.set_global(
            "workflow",
            Variable::Static(json!({
                "name": "code-refactoring",
                "id": "wf-789",
                "start_time": "2024-01-15T14:30:00Z"
            })),
        );

        context.set_phase(
            "setup",
            Variable::Static(json!({
                "output": "Found 150 files to process",
                "variables": {
                    "file_count": 150
                }
            })),
        );

        context.set_phase(
            "map",
            Variable::Static(json!({
                "total": 150,
                "successful": 142,
                "failed": 8,
                "skipped": 0,
                "results": [
                    {"file": "src/main.rs", "status": "success"},
                    {"file": "src/lib.rs", "status": "success"},
                    {"file": "src/error.rs", "status": "failed"}
                ],
                "duration": 300.5,
                "success_rate": 94.67
            })),
        );

        // Complex template using multiple variable types
        let template = r#"
Workflow: ${workflow.name} (${workflow.id})
Setup: ${setup.output}
Results: ${map.successful}/${map.total} files processed successfully (${map.success_rate}% success rate)
Duration: ${map.duration} seconds
Environment: ${env.USER}
"#;

        // Set a USER env var for testing
        std::env::set_var("USER", "test_user");

        let result = context.interpolate(template).await.unwrap();

        assert!(result.contains("Workflow: code-refactoring (wf-789)"));
        assert!(result.contains("Setup: Found 150 files to process"));
        assert!(
            result.contains("Results: 142/150 files processed successfully (94.67% success rate)")
        );
        assert!(result.contains("Duration: 300.5 seconds"));
        assert!(result.contains("Environment: test_user"));

        // Clean up
        std::env::remove_var("USER");
    }

    #[tokio::test]
    async fn test_child_context_inheritance() {
        let mut parent = VariableContext::new();
        parent.set_global("parent_var", Variable::Static(json!("parent_value")));
        parent.set_phase("shared_var", Variable::Static(json!("parent_shared")));

        let mut child = parent.child();
        child.set_local("child_var", Variable::Static(json!("child_value")));
        child.set_phase("shared_var", Variable::Static(json!("child_shared")));

        // Child should have access to parent's global variables
        let result = child.interpolate("${parent_var}").await.unwrap();
        assert_eq!(result, "parent_value");

        // Child's phase variable should override parent's
        let result = child.interpolate("${shared_var}").await.unwrap();
        assert_eq!(result, "child_shared");

        // Child has its own local variables
        let result = child.interpolate("${child_var}").await.unwrap();
        assert_eq!(result, "child_value");
    }
}
