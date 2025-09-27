//! Tests for dry-run mode functionality
//!
//! This module contains unit and integration tests for the dry-run validation system.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::cook::execution::mapreduce::{MapPhase, MapReduceConfig, ReducePhase, SetupPhase};
    use crate::cook::workflow::WorkflowStep;
    use serde_json::json;
    use std::time::Duration;

    /// Helper function to create a WorkflowStep with defaults
    fn create_workflow_step() -> WorkflowStep {
        WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            command: None,
            handler: None,
            capture: None,
            capture_format: None,
            on_failure: None,
            commit_required: None,
            timeout: None,
            env: None,
            cwd: None,
            persist_env: None,
            stream_output: None,
            ignore_exit_code: None,
            expect_exit_code: None,
            is_async: None,
            hidden: None,
            auto_commit: None,
            skip_if: None,
            run_if: None,
            retry: None,
            catch: None,
            r#finally: None,
        }
    }

    /// Create a test MapPhase configuration
    fn create_test_map_phase() -> MapPhase {
        MapPhase {
            config: MapReduceConfig {
                input: "test_data.json".to_string(),
                json_path: "$.items[*]".to_string(),
                max_parallel: 5,
                agent_timeout_secs: Some(300),
                continue_on_failure: false,
                batch_size: None,
                enable_checkpoints: true,
                max_items: Some(10),
                offset: None,
            },
            json_path: Some("$.items[*]".to_string()),
            agent_template: vec![
                {
                    let mut step = create_workflow_step();
                    step.claude = Some("/process ${item.id}".to_string());
                    step
                },
                {
                    let mut step = create_workflow_step();
                    step.shell = Some("echo 'Processing ${item.name}'".to_string());
                    step
                },
            ],
            filter: None,
            sort_by: None,
            max_items: Some(10),
            distinct: None,
        }
    }

    /// Create a test SetupPhase configuration
    fn create_test_setup_phase() -> SetupPhase {
        SetupPhase {
            commands: vec![
                {
                    let mut step = create_workflow_step();
                    step.shell = Some("echo 'Starting setup'".to_string());
                    step
                },
                {
                    let mut step = create_workflow_step();
                    step.shell = Some("mkdir -p output".to_string());
                    step
                },
            ],
            timeout: 60,
            capture_outputs: std::collections::HashMap::new(),
        }
    }

    /// Create a test ReducePhase configuration
    fn create_test_reduce_phase() -> ReducePhase {
        ReducePhase {
            commands: vec![{
                let mut step = create_workflow_step();
                step.claude = Some("/summarize ${map.results}".to_string());
                step
            }],
            timeout_secs: Some(120),
        }
    }

    #[tokio::test]
    async fn test_input_validator_file() {
        let validator = input_validator::InputValidator::new();

        // Create a temporary test file
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.json");
        std::fs::write(&test_file, r#"{"items": [{"id": 1}, {"id": 2}]}"#).unwrap();

        let result = validator
            .validate_input_source(test_file.to_str().unwrap())
            .await
            .unwrap();

        assert!(result.valid);
        assert_eq!(result.item_count_estimate, 2);
        assert!(result.data_structure.contains("items"));
    }

    #[tokio::test]
    async fn test_input_validator_shell_command() {
        let validator = input_validator::InputValidator::new();

        let result = validator
            .validate_input_source("shell: ls -la")
            .await
            .unwrap();

        assert!(result.valid);
        assert!(result.data_structure.contains("command output"));
    }

    #[test]
    fn test_command_validator_claude() {
        let validator = command_validator::CommandValidator::new();

        let mut command = create_workflow_step();
        command.claude = Some("/process-file".to_string());

        let result = validator.validate_command(&command);

        assert!(result.valid);
        assert_eq!(result.command_type, types::CommandType::Claude);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_command_validator_shell() {
        let validator = command_validator::CommandValidator::new();

        let mut command = create_workflow_step();
        command.shell = Some("echo 'test'".to_string());

        let result = validator.validate_command(&command);

        assert!(result.valid);
        assert_eq!(result.command_type, types::CommandType::Shell);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_command_validator_dangerous_shell() {
        let validator = command_validator::CommandValidator::new();

        let mut command = create_workflow_step();
        command.shell = Some("rm -rf /".to_string());

        let result = validator.validate_command(&command);

        assert!(!result.valid);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_command_validator_empty_command() {
        let validator = command_validator::CommandValidator::new();

        let mut command = create_workflow_step();
        command.shell = Some("".to_string());

        let result = validator.validate_command(&command);

        assert!(!result.valid);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_command_validator_variable_extraction() {
        let validator = command_validator::CommandValidator::new();

        let mut command = create_workflow_step();
        command.shell = Some("echo '${item.name}' > ${shell.output}".to_string());

        let result = validator.validate_command(&command);

        assert!(result.valid);
        assert_eq!(result.variable_references.len(), 2);
        assert!(result
            .variable_references
            .iter()
            .any(|v| v.name == "item.name"));
        assert!(result
            .variable_references
            .iter()
            .any(|v| v.name == "shell.output"));
    }

    #[test]
    fn test_resource_estimator() {
        let estimator = resource_estimator::ResourceEstimator::new();

        let map_phase = create_test_map_phase();
        let work_items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        let result = estimator.estimate_resources(&map_phase, &work_items, None, None);

        assert!(result.memory_usage.total_mb > 0);
        assert!(result.disk_usage.total_mb > 0);
        assert_eq!(result.worktree_count, 3); // min(5 max_parallel, 3 work items)
        assert!(result.checkpoint_storage.total_mb >= 0);
    }

    #[test]
    fn test_variable_processor() {
        let processor = variable_processor::VariableProcessor::new();

        let map_phase = create_test_map_phase();
        let work_items = vec![
            json!({"id": 1, "name": "Item 1"}),
            json!({"id": 2, "name": "Item 2"}),
        ];

        let result = processor
            .create_preview(&map_phase, &work_items, None, None)
            .unwrap();

        assert!(!result.item_variables.is_empty());
        assert!(result.item_variables[0].contains_key("item.id"));
        assert!(result.item_variables[0].contains_key("item.name"));
    }

    #[tokio::test]
    async fn test_dry_run_validator_complete_workflow() {
        let validator = DryRunValidator::new();

        // Create test data file
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test_data.json");
        std::fs::write(
            &test_file,
            r#"{
                "items": [
                    {"id": 1, "name": "Item 1"},
                    {"id": 2, "name": "Item 2"},
                    {"id": 3, "name": "Item 3"}
                ]
            }"#,
        )
        .unwrap();

        let mut map_phase = create_test_map_phase();
        map_phase.config.input = test_file.to_str().unwrap().to_string();

        let setup_phase = Some(create_test_setup_phase());
        let reduce_phase = Some(create_test_reduce_phase());

        let result = validator
            .validate_workflow_phases(setup_phase, map_phase, reduce_phase)
            .await
            .unwrap();

        assert!(result.validation_results.is_valid);
        assert!(result.errors.is_empty());
        assert_eq!(result.work_item_preview.total_count, 3);
        assert!(result.resource_estimates.memory_usage.total_mb > 0);
        assert!(result.estimated_duration > Duration::ZERO);
    }

    #[test]
    fn test_output_formatter_human() {
        let formatter = output_formatter::OutputFormatter::new();

        let report = DryRunReport {
            validation_results: ValidationResults {
                setup_phase: Some(PhaseValidation {
                    valid: true,
                    command_count: 2,
                    estimated_duration: Duration::from_secs(60),
                    dependencies_met: true,
                    issues: vec![],
                }),
                map_phase: PhaseValidation {
                    valid: true,
                    command_count: 2,
                    estimated_duration: Duration::from_secs(180),
                    dependencies_met: true,
                    issues: vec![],
                },
                reduce_phase: Some(PhaseValidation {
                    valid: true,
                    command_count: 1,
                    estimated_duration: Duration::from_secs(120),
                    dependencies_met: true,
                    issues: vec![],
                }),
                is_valid: true,
            },
            work_item_preview: WorkItemPreview {
                total_count: 3,
                sample_items: vec![],
                distribution: [(0, 1), (1, 1), (2, 1)].iter().cloned().collect(),
                filtered_count: None,
                sort_description: None,
            },
            resource_estimates: ResourceEstimates {
                memory_usage: types::MemoryEstimate {
                    total_mb: 300,
                    per_agent_mb: 100,
                    peak_concurrent_agents: 3,
                },
                disk_usage: types::DiskEstimate {
                    total_mb: 500,
                    per_worktree_mb: 150,
                    temp_space_mb: 50,
                },
                network_usage: types::NetworkEstimate {
                    data_transfer_mb: 100,
                    api_calls: 10,
                    parallel_operations: 3,
                },
                worktree_count: 3,
                checkpoint_storage: types::StorageEstimate {
                    checkpoint_size_kb: 100,
                    checkpoint_count: 2,
                    total_mb: 1,
                },
            },
            variable_preview: VariablePreview::default(),
            warnings: vec![],
            errors: vec![],
            estimated_duration: Duration::from_secs(360),
        };

        let output = formatter.format_human(&report);

        assert!(output.contains("MapReduce Workflow Dry-Run Report"));
        assert!(output.contains("✅ READY"));
        assert!(output.contains("Setup Phase"));
        assert!(output.contains("Map Phase"));
        assert!(output.contains("Reduce Phase"));
        assert!(output.contains("300 MB total"));
        assert!(output.contains("6m 0s"));
    }

    #[test]
    fn test_output_formatter_json() {
        let formatter = output_formatter::OutputFormatter::new();

        let report = DryRunReport {
            validation_results: ValidationResults {
                setup_phase: None,
                map_phase: PhaseValidation {
                    valid: true,
                    command_count: 1,
                    estimated_duration: Duration::from_secs(60),
                    dependencies_met: true,
                    issues: vec![],
                },
                reduce_phase: None,
                is_valid: true,
            },
            work_item_preview: WorkItemPreview::default(),
            resource_estimates: ResourceEstimates {
                memory_usage: types::MemoryEstimate {
                    total_mb: 100,
                    per_agent_mb: 100,
                    peak_concurrent_agents: 1,
                },
                disk_usage: types::DiskEstimate {
                    total_mb: 200,
                    per_worktree_mb: 200,
                    temp_space_mb: 0,
                },
                network_usage: types::NetworkEstimate {
                    data_transfer_mb: 10,
                    api_calls: 1,
                    parallel_operations: 1,
                },
                worktree_count: 1,
                checkpoint_storage: types::StorageEstimate {
                    checkpoint_size_kb: 10,
                    checkpoint_count: 1,
                    total_mb: 0,
                },
            },
            variable_preview: VariablePreview::default(),
            warnings: vec![],
            errors: vec![],
            estimated_duration: Duration::from_secs(60),
        };

        let json = formatter.format_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["validation_results"]["is_valid"].as_bool().unwrap());
        assert_eq!(
            parsed["resource_estimates"]["memory_usage"]["total_mb"]
                .as_u64()
                .unwrap(),
            100
        );
    }
}
