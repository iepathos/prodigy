//! Integration tests demonstrating I/O separation from business logic

use chrono::Utc;
use prodigy::core::{config, mapreduce, session, validation, workflow};
use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
mod io_separation_tests {
    use super::*;

    #[test]
    fn test_pure_config_parsing() {
        // Pure function test - no file I/O needed
        let yaml_content = r#"
commands:
  - prodigy-code-review
  - prodigy-implement-spec
  - prodigy-lint
"#;

        let result = config::parse_workflow_config(yaml_content).unwrap();
        assert_eq!(result.commands.len(), 3);

        // No mocking needed, just pure data transformation
    }

    #[test]
    fn test_pure_workflow_variable_interpolation() {
        // Pure function test - no I/O
        let mut variables = HashMap::new();
        variables.insert("project".to_string(), "prodigy".to_string());
        variables.insert("version".to_string(), "1.0".to_string());

        let template = "Building ${project} version ${version}";
        let result = workflow::interpolate_variables(template, &variables);

        assert_eq!(result, "Building prodigy version 1.0");
    }

    #[test]
    fn test_pure_session_state_updates() {
        // Pure function test - no database needed
        let session = session::SessionState {
            id: "test-123".to_string(),
            status: session::SessionStatus::InProgress,
            started_at: Utc::now(),
            completed_at: None,
            metadata: HashMap::new(),
            iterations_completed: 0,
            files_changed: 0,
            current_step: 0,
            total_steps: 10,
            error: None,
        };

        // Apply updates without any I/O
        let updated = session::apply_session_update(
            session,
            session::SessionUpdate::Progress {
                current: 5,
                total: 10,
            },
        );

        assert_eq!(updated.current_step, 5);
        assert_eq!(updated.total_steps, 10);
    }

    #[test]
    fn test_pure_validation_logic() {
        // Pure validation without file system access
        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin".to_string());
        env_vars.insert("HOME".to_string(), "/home/user".to_string());

        let required = vec!["PATH", "HOME"];
        let result = validation::validate_environment(&required, &env_vars);

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_pure_mapreduce_work_distribution() {
        // Pure function test - no file I/O
        let items = vec![
            json!({"id": 1, "task": "task1"}),
            json!({"id": 2, "task": "task2"}),
            json!({"id": 3, "task": "task3"}),
            json!({"id": 4, "task": "task4"}),
            json!({"id": 5, "task": "task5"}),
        ];

        let distribution = mapreduce::distribute_work(items, 2, None, None);

        assert_eq!(distribution.len(), 2);
        let total_items: usize = distribution.iter().map(|d| d.items.len()).sum();
        assert_eq!(total_items, 5);

        // No file operations needed, pure computation
    }

    #[test]
    fn test_pure_workflow_validation() {
        // Pure validation logic
        let commands = vec![
            "shell: echo hello".to_string(),
            "claude: /implement".to_string(),
            "test: cargo test".to_string(),
        ];

        let result = workflow::validate_workflow_structure(&commands, Some(5));

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_pure_command_parsing() {
        // Test command type parsing without execution
        assert_eq!(
            workflow::parse_command_type("shell: ls -la"),
            Some(workflow::CommandType::Shell)
        );
        assert_eq!(
            workflow::parse_command_type("claude: /help"),
            Some(workflow::CommandType::Claude)
        );

        // Extract content without executing
        assert_eq!(workflow::extract_command_content("shell: ls -la"), "ls -la");
    }

    #[test]
    fn test_pure_session_summary_generation() {
        // Generate summary without database queries
        let session = session::SessionState {
            id: "session-456".to_string(),
            status: session::SessionStatus::Completed,
            started_at: Utc::now() - chrono::Duration::minutes(30),
            completed_at: Some(Utc::now()),
            metadata: HashMap::new(),
            iterations_completed: 5,
            files_changed: 12,
            current_step: 10,
            total_steps: 10,
            error: None,
        };

        let summary = session::generate_session_summary(&session);

        assert_eq!(summary.id, "session-456");
        assert_eq!(summary.iterations, 5);
        assert_eq!(summary.files_changed, 12);
        assert_eq!(summary.progress_percentage, 100.0);
    }

    #[test]
    fn test_pure_mapreduce_filtering() {
        // Test filtering without I/O
        let items = vec![
            json!({"score": 10, "name": "high"}),
            json!({"score": 5, "name": "low"}),
            json!({"score": 8, "name": "medium"}),
        ];

        let filtered = mapreduce::filter_work_items(items, Some("score >= 7"));

        let passed_count = filtered.iter().filter(|f| f.passed).count();
        assert_eq!(passed_count, 2);
    }

    #[test]
    fn test_pure_resource_validation() {
        // Validate resources without system checks
        let limits = validation::ResourceLimits {
            memory_mb: 2048,
            cpu_cores: 4,
            timeout_seconds: 300,
        };

        let result = validation::validate_resource_limits(&limits);

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }
}

/// Demonstrate how I/O wrappers use pure functions
#[cfg(test)]
mod io_wrapper_examples {
    use super::*;

    /// Example of how an I/O wrapper would use pure functions
    async fn _load_and_parse_config(path: &std::path::Path) -> anyhow::Result<()> {
        // I/O operation: read file
        let content = tokio::fs::read_to_string(path).await?;

        // Pure operation: parse content
        let _workflow = config::parse_workflow_config(&content)?;

        // Pure operation: validate structure
        // let validation = workflow::validate_workflow_structure(&workflow.commands, None);

        Ok(())
    }

    /// Example of session update with I/O separation
    async fn _update_session_with_io(
        _session_id: &str,
        _update: session::SessionUpdate,
    ) -> anyhow::Result<()> {
        // I/O: Load session from storage
        // let session = load_session_from_db(session_id).await?;

        // Pure: Apply update
        // let updated = session::apply_session_update(session, update);

        // I/O: Save back to storage
        // save_session_to_db(updated).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_io_wrapper_pattern() {
        // This test shows the pattern but doesn't execute real I/O
        // In real usage, the I/O operations would be thin wrappers
        // around the pure business logic functions

        // Pure logic can be tested without mocks
        let session = session::SessionState {
            id: "test".to_string(),
            status: session::SessionStatus::InProgress,
            started_at: Utc::now(),
            completed_at: None,
            metadata: HashMap::new(),
            iterations_completed: 0,
            files_changed: 0,
            current_step: 0,
            total_steps: 0,
            error: None,
        };

        // Test the pure transformation
        let updated =
            session::apply_session_update(session, session::SessionUpdate::IncrementIteration);

        assert_eq!(updated.iterations_completed, 1);
    }
}
