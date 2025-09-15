//! Tests for foreach execution functionality

use super::foreach::*;
use crate::config::command::{
    ForeachConfig, ForeachInput, ParallelConfig, TestCommand, WorkflowStepCommand,
};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[cfg(test)]
mod foreach_execution_tests {
    use super::*;

    /// Test basic foreach with list input
    #[tokio::test]
    async fn test_foreach_with_list_input() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["item1".to_string(), "item2".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_items, 2);
        assert_eq!(result.failed_items, 0);
        assert!(result.errors.is_empty());
    }

    /// Test foreach with command input
    #[tokio::test]
    async fn test_foreach_with_command_input() {
        let config = ForeachConfig {
            input: ForeachInput::Command("echo -e 'item1\\nitem2\\nitem3'".to_string()),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 3);
        assert_eq!(result.successful_items, 3);
        assert_eq!(result.failed_items, 0);
    }

    /// Test parallel execution
    #[tokio::test]
    async fn test_foreach_parallel_execution() {
        // Create a counter to track concurrent executions
        let _counter = Arc::new(AtomicUsize::new(0));
        let _max_concurrent = Arc::new(AtomicUsize::new(0));

        let config = ForeachConfig {
            input: ForeachInput::List(vec![
                "item1".to_string(),
                "item2".to_string(),
                "item3".to_string(),
                "item4".to_string(),
            ]),
            parallel: ParallelConfig::Count(2), // Limit to 2 parallel
            do_block: vec![Box::new(WorkflowStepCommand {
                // This command will help us test parallelism
                shell: Some("sleep 0.1 && echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let start = std::time::Instant::now();
        let result = execute_foreach(&config).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(result.total_items, 4);
        assert_eq!(result.successful_items, 4);
        assert_eq!(result.failed_items, 0);

        // With parallelism of 2, 4 items with 0.1s sleep each should take ~0.2s, not 0.4s
        assert!(
            duration.as_secs_f32() < 0.35,
            "Parallel execution should be faster than sequential"
        );
    }

    /// Test max_items limit
    #[tokio::test]
    async fn test_foreach_max_items() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec![
                "item1".to_string(),
                "item2".to_string(),
                "item3".to_string(),
                "item4".to_string(),
                "item5".to_string(),
            ]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: Some(3), // Limit to 3 items
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 3); // Should process only 3 items
        assert_eq!(result.successful_items, 3);
        assert_eq!(result.failed_items, 0);
    }

    /// Test continue_on_error behavior
    #[tokio::test]
    async fn test_foreach_continue_on_error() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec![
                "item1".to_string(),
                "fail".to_string(),
                "item3".to_string(),
            ]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                // This will fail for "fail" item
                shell: Some("test \"${item}\" != \"fail\" && echo Success || exit 1".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: true, // Continue despite failures
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 3);
        assert_eq!(result.successful_items, 2);
        assert_eq!(result.failed_items, 1);
        assert_eq!(result.errors.len(), 1);
    }

    /// Test fail-fast behavior (stop on first error)
    #[tokio::test]
    async fn test_foreach_fail_fast() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec![
                "item1".to_string(),
                "fail".to_string(),
                "item3".to_string(),
            ]),
            parallel: ParallelConfig::Boolean(false), // Sequential to ensure order
            do_block: vec![Box::new(WorkflowStepCommand {
                // This will fail for "fail" item
                shell: Some("test \"${item}\" != \"fail\" && echo Success || exit 1".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false, // Stop on first error
            max_items: None,
        };

        let result = execute_foreach(&config).await;

        // Should fail with error
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Foreach execution failed"));
    }

    /// Test variable interpolation with index and total
    #[tokio::test]
    async fn test_foreach_variable_interpolation() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["item1".to_string(), "item2".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo 'Processing ${item} (${index}/${total})'".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_items, 2);
        assert_eq!(result.failed_items, 0);
    }

    /// Test empty input list
    #[tokio::test]
    async fn test_foreach_empty_input() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec![]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 0);
        assert_eq!(result.successful_items, 0);
        assert_eq!(result.failed_items, 0);
        assert!(result.errors.is_empty());
    }

    /// Test multiple commands in do block
    #[tokio::test]
    async fn test_foreach_multiple_commands() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["item1".to_string(), "item2".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![
                Box::new(WorkflowStepCommand {
                    shell: Some("echo 'Starting ${item}'".to_string()),
                    claude: None,
                    analyze: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    id: None,
                    commit_required: false,
                    analysis: None,
                    outputs: None,
                    validate: None,
                    timeout: None,
                    when: None,
                    capture_format: None,
                    capture_streams: None,
                    output_file: None,
                    capture_output: None,
                    on_failure: None,
                    on_success: None,
                }),
                Box::new(WorkflowStepCommand {
                    shell: Some("echo 'Processing ${item}'".to_string()),
                    claude: None,
                    analyze: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    id: None,
                    commit_required: false,
                    analysis: None,
                    outputs: None,
                    validate: None,
                    timeout: None,
                    when: None,
                    capture_format: None,
                    capture_streams: None,
                    output_file: None,
                    capture_output: None,
                    on_failure: None,
                    on_success: None,
                }),
                Box::new(WorkflowStepCommand {
                    shell: Some("echo 'Finished ${item}'".to_string()),
                    claude: None,
                    analyze: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    id: None,
                    commit_required: false,
                    analysis: None,
                    outputs: None,
                    validate: None,
                    timeout: None,
                    when: None,
                    capture_format: None,
                    capture_streams: None,
                    output_file: None,
                    capture_output: None,
                    on_failure: None,
                    on_success: None,
                }),
            ],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_items, 2);
        assert_eq!(result.failed_items, 0);
    }

    /// Test deprecated test command type
    #[tokio::test]
    async fn test_foreach_deprecated_test_command() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["item1".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                test: Some(TestCommand {
                    command: "echo 'Testing ${item}'".to_string(),
                    on_failure: None,
                }),
                shell: None,
                claude: None,
                analyze: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();

        assert_eq!(result.total_items, 1);
        assert_eq!(result.successful_items, 1);
        assert_eq!(result.failed_items, 0);
    }
}

#[cfg(test)]
mod foreach_item_source_tests {
    use super::*;

    /// Test command that produces no output
    #[tokio::test]
    async fn test_command_input_empty_output() {
        let input = ForeachInput::Command("echo ''".to_string());
        let items = get_items(&input).await.unwrap();
        assert_eq!(items.len(), 0);
    }

    /// Test command that produces items with spaces
    #[tokio::test]
    async fn test_command_input_with_spaces() {
        let input = ForeachInput::Command("printf 'item with spaces\\nanother item'".to_string());
        let items = get_items(&input).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], "item with spaces");
        assert_eq!(items[1], "another item");
    }

    /// Test command that fails
    #[tokio::test]
    async fn test_command_input_failure() {
        let input = ForeachInput::Command("exit 1".to_string());
        let result = get_items(&input).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Foreach command failed"));
    }

    /// Test list input preservation
    #[tokio::test]
    async fn test_list_input_preservation() {
        let original_items = vec![
            "item1".to_string(),
            "item with spaces".to_string(),
            "item-with-dashes".to_string(),
            "item_with_underscores".to_string(),
        ];
        let input = ForeachInput::List(original_items.clone());
        let items = get_items(&input).await.unwrap();
        assert_eq!(items, original_items);
    }
}
