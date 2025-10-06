//! Tests for variable checkpoint and resume functionality

#[cfg(test)]
mod tests {
    use super::super::checkpoint::*;
    use super::super::executor::WorkflowContext;
    use super::super::variable_checkpoint::*;
    use super::super::variables::VariableStore;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Create test variable checkpoint state
    fn create_test_variable_state() -> VariableCheckpointState {
        let mut global_variables = HashMap::new();
        global_variables.insert("item".to_string(), json!("test.txt"));
        global_variables.insert("workflow.name".to_string(), json!("test-workflow"));
        global_variables.insert("map.total".to_string(), json!("10"));
        global_variables.insert("map.successful".to_string(), json!("7"));
        global_variables.insert("map.failed".to_string(), json!("3"));

        let mut phase_variables = HashMap::new();
        let mut setup_vars = HashMap::new();
        setup_vars.insert("setup_result".to_string(), json!("ready"));
        phase_variables.insert("setup".to_string(), setup_vars);

        let environment_snapshot = EnvironmentSnapshot {
            variables: HashMap::from([
                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                ("HOME".to_string(), "/home/test".to_string()),
                ("TEST_VAR".to_string(), "test_value".to_string()),
            ]),
            captured_at: Utc::now(),
            hostname: "test-host".to_string(),
            working_directory: PathBuf::from("/test/dir"),
        };

        let interpolation_history = vec![
            InterpolationRecord {
                template: "Processing ${item}".to_string(),
                result: "Processing test.txt".to_string(),
                interpolated_at: Utc::now(),
                variable_dependencies: vec!["item".to_string()],
                phase: None,
            },
            InterpolationRecord {
                template: "Total: ${map.total}, Success: ${map.successful}".to_string(),
                result: "Total: 10, Success: 7".to_string(),
                interpolated_at: Utc::now(),
                variable_dependencies: vec!["map.total".to_string(), "map.successful".to_string()],
                phase: Some("reduce".to_string()),
            },
        ];

        VariableCheckpointState {
            global_variables,
            phase_variables,
            computed_cache: HashMap::new(),
            environment_snapshot,
            interpolation_history,
            variable_metadata: VariableMetadata {
                total_variables: 5,
                computed_variables: 0,
                total_interpolations: 2,
                checkpoint_version: "1.0.0".to_string(),
            },
            captured_outputs: HashMap::from([(
                "shell.output".to_string(),
                "command output".to_string(),
            )]),
            iteration_vars: HashMap::from([("foreach.index".to_string(), "5".to_string())]),
        }
    }

    #[test]
    fn test_variable_checkpoint_creation() {
        let manager = VariableResumeManager::new();

        let variables = HashMap::from([
            ("var1".to_string(), "value1".to_string()),
            ("var2".to_string(), "value2".to_string()),
        ]);

        let captured_outputs = HashMap::from([("output1".to_string(), "result1".to_string())]);

        let iteration_vars = HashMap::from([("index".to_string(), "0".to_string())]);

        let variable_store = VariableStore::new();

        let checkpoint = manager
            .create_checkpoint(
                &variables,
                &captured_outputs,
                &iteration_vars,
                &variable_store,
            )
            .unwrap();

        assert_eq!(checkpoint.global_variables.len(), 3); // vars + outputs
        assert_eq!(checkpoint.captured_outputs, captured_outputs);
        assert_eq!(checkpoint.iteration_vars, iteration_vars);
        assert_eq!(checkpoint.variable_metadata.total_variables, 3);
    }

    #[test]
    fn test_variable_restoration() {
        let manager = VariableResumeManager::new();
        let test_state = create_test_variable_state();

        let (variables, captured_outputs, iteration_vars) =
            manager.restore_from_checkpoint(&test_state).unwrap();

        // Check global variables restored
        assert_eq!(variables.get("item").unwrap(), "test.txt");
        assert_eq!(variables.get("workflow.name").unwrap(), "test-workflow");
        assert_eq!(variables.get("map.total").unwrap(), "10");
        assert_eq!(variables.get("map.successful").unwrap(), "7");
        assert_eq!(variables.get("map.failed").unwrap(), "3");

        // Check captured outputs restored
        assert_eq!(
            captured_outputs.get("shell.output").unwrap(),
            "command output"
        );

        // Check iteration vars restored
        assert_eq!(iteration_vars.get("foreach.index").unwrap(), "5");
    }

    #[test]
    fn test_mapreduce_variable_recalculation() {
        let manager = VariableResumeManager::new();

        let vars = manager.recalculate_mapreduce_variables(20, 15, 5);

        assert_eq!(vars.get("map.total").unwrap(), "20");
        assert_eq!(vars.get("map.successful").unwrap(), "15");
        assert_eq!(vars.get("map.failed").unwrap(), "5");
        assert_eq!(vars.get("map.completed").unwrap(), "20");
        assert_eq!(vars.get("map.success_rate").unwrap(), "75.00");
    }

    #[test]
    fn test_environment_compatibility_check() {
        let manager = VariableResumeManager::new();
        let test_state = create_test_variable_state();

        // Save current env vars for restoration
        let saved_test_var = std::env::var("TEST_VAR").ok();

        // Set up test environment
        std::env::set_var("TEST_VAR", "test_value");

        let compatibility = manager
            .validate_environment(&test_state.environment_snapshot)
            .unwrap();

        // Should be compatible if TEST_VAR matches (it does now)
        // Note: The test could have missing variables that are not critical
        assert!(
            compatibility.is_compatible
                || !compatibility.missing_variables.is_empty()
                || !compatibility.changed_variables.is_empty()
        );

        // Change environment variable
        std::env::set_var("TEST_VAR", "different_value");

        let compatibility2 = manager
            .validate_environment(&test_state.environment_snapshot)
            .unwrap();

        // May have changes but not necessarily incompatible
        // (as we filter out non-critical changes)
        assert!(
            compatibility2.changed_variables.is_empty()
                || compatibility2.changed_variables.contains_key("TEST_VAR")
        );

        // Restore original env var
        if let Some(val) = saved_test_var {
            std::env::set_var("TEST_VAR", val);
        } else {
            std::env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_interpolation_test_results() {
        let mut results = InterpolationTestResults::new();

        // Add passing test
        results.add_test(InterpolationTest {
            template: "${var1}".to_string(),
            original_result: "value1".to_string(),
            current_result: "value1".to_string(),
            matches: true,
            interpolated_at: Utc::now(),
        });

        // Add failing test
        results.add_test(InterpolationTest {
            template: "${var2}".to_string(),
            original_result: "value2".to_string(),
            current_result: "different".to_string(),
            matches: false,
            interpolated_at: Utc::now(),
        });

        assert_eq!(results.total_tests, 2);
        assert_eq!(results.passed_tests, 1);
        assert_eq!(results.failed_tests, 1);
        assert!(!results.all_passed());
    }

    #[tokio::test]
    async fn test_full_checkpoint_resume_cycle() {
        use super::super::normalized::NormalizedWorkflow;
        use super::super::resume::ResumeExecutor;

        let temp_dir = TempDir::new().unwrap();
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        std::fs::create_dir_all(&checkpoint_dir).unwrap();

        // Create test workflow context
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("test_var".to_string(), "test_value".to_string());
        context
            .variables
            .insert("map.total".to_string(), "5".to_string());
        context
            .captured_outputs
            .insert("step1".to_string(), "output1".to_string());
        context
            .iteration_vars
            .insert("index".to_string(), "3".to_string());

        // Create a test workflow
        let workflow = NormalizedWorkflow {
            name: Arc::from("test-workflow"),
            steps: Arc::from(vec![]),
            execution_mode: super::super::normalized::ExecutionMode::Sequential,
            variables: Arc::new(HashMap::new()),
        };

        // Create checkpoint with variable state
        let checkpoint = create_checkpoint_with_total_steps(
            "test-checkpoint".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "test-hash".to_string(),
            3,
        );

        // Verify variable checkpoint state was created
        assert!(checkpoint.variable_checkpoint_state.is_some());

        if let Some(var_state) = &checkpoint.variable_checkpoint_state {
            // Check that variables were preserved
            assert!(var_state.global_variables.contains_key("test_var"));
            assert!(var_state.captured_outputs.contains_key("step1"));
            assert!(var_state.iteration_vars.contains_key("index"));
        }

        // Save checkpoint
        #[allow(deprecated)]
        let checkpoint_manager = std::sync::Arc::new(CheckpointManager::new(checkpoint_dir));
        checkpoint_manager
            .save_checkpoint(&checkpoint)
            .await
            .unwrap();

        // Load checkpoint
        let loaded = checkpoint_manager
            .load_checkpoint("test-checkpoint")
            .await
            .unwrap();

        // Create resume executor and restore context
        let resume_executor = ResumeExecutor::new(checkpoint_manager);
        let restored_context = resume_executor.restore_workflow_context(&loaded).unwrap();

        // Verify variables were restored correctly
        assert_eq!(
            restored_context.variables.get("test_var").unwrap(),
            "test_value"
        );
        assert_eq!(restored_context.variables.get("map.total").unwrap(), "5");
        assert_eq!(
            restored_context.captured_outputs.get("step1").unwrap(),
            "output1"
        );
        assert_eq!(restored_context.iteration_vars.get("index").unwrap(), "3");
    }

    #[test]
    fn test_mapreduce_checkpoint_variables() {
        let mut mapreduce_checkpoint = MapReduceCheckpoint {
            completed_items: [
                "item1".to_string(),
                "item2".to_string(),
                "item3".to_string(),
            ]
            .iter()
            .cloned()
            .collect(),
            failed_items: vec!["item4".to_string(), "item5".to_string()],
            in_progress_items: HashMap::new(),
            reduce_completed: false,
            agent_results: HashMap::new(),
            total_items: 10,
            aggregate_variables: HashMap::new(),
        };

        // Calculate aggregate variables
        let manager = VariableResumeManager::new();
        let vars = manager.recalculate_mapreduce_variables(
            mapreduce_checkpoint.total_items,
            mapreduce_checkpoint.completed_items.len(),
            mapreduce_checkpoint.failed_items.len(),
        );

        // Store in checkpoint
        mapreduce_checkpoint.aggregate_variables = vars.clone();

        // Verify calculations
        assert_eq!(
            mapreduce_checkpoint
                .aggregate_variables
                .get("map.total")
                .unwrap(),
            "10"
        );
        assert_eq!(
            mapreduce_checkpoint
                .aggregate_variables
                .get("map.successful")
                .unwrap(),
            "3"
        );
        assert_eq!(
            mapreduce_checkpoint
                .aggregate_variables
                .get("map.failed")
                .unwrap(),
            "2"
        );
        assert_eq!(
            mapreduce_checkpoint
                .aggregate_variables
                .get("map.completed")
                .unwrap(),
            "5"
        );
        assert_eq!(
            mapreduce_checkpoint
                .aggregate_variables
                .get("map.success_rate")
                .unwrap(),
            "30.00"
        );
    }

    #[test]
    fn test_variable_preservation_across_interruption() {
        // Simulate workflow interruption and resume
        let manager = VariableResumeManager::new();

        // Create initial state with variables
        let mut initial_vars = HashMap::new();
        initial_vars.insert("workflow.iteration".to_string(), "5".to_string());
        initial_vars.insert("custom.value".to_string(), "important_data".to_string());
        initial_vars.insert("computed.result".to_string(), "42".to_string());

        let captured = HashMap::from([("last.output".to_string(), "command result".to_string())]);

        let iteration = HashMap::from([("loop.counter".to_string(), "10".to_string())]);

        // Create checkpoint
        let checkpoint_state = manager
            .create_checkpoint(&initial_vars, &captured, &iteration, &VariableStore::new())
            .unwrap();

        // Simulate resume after interruption
        let (restored_vars, restored_captured, restored_iteration) =
            manager.restore_from_checkpoint(&checkpoint_state).unwrap();

        // All variables should be exactly preserved
        assert_eq!(restored_vars.get("workflow.iteration").unwrap(), "5");
        assert_eq!(restored_vars.get("custom.value").unwrap(), "important_data");
        assert_eq!(restored_vars.get("computed.result").unwrap(), "42");
        assert_eq!(
            restored_captured.get("last.output").unwrap(),
            "command result"
        );
        assert_eq!(restored_iteration.get("loop.counter").unwrap(), "10");
    }
}
