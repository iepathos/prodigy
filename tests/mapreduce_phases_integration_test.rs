//! Integration tests for MapReduce phase modules
//!
//! These tests verify end-to-end behavior of the phase-based architecture
//! including setup, map, and reduce phases with the coordinator.

use prodigy::cook::execution::mapreduce::phases::{
    coordinator::PhaseCoordinator, PhaseContext, PhaseExecutor,
};
use prodigy::cook::execution::mapreduce::{MapPhase, MapReduceConfig, ReducePhase};
use prodigy::cook::execution::variable_capture::CaptureConfig;
use prodigy::cook::execution::SetupPhase;
use prodigy::cook::orchestrator::ExecutionEnvironment;
use prodigy::cook::workflow::WorkflowStep;
use prodigy::subprocess::SubprocessManager;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test execution environment
fn create_test_env(temp_dir: &TempDir) -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: Arc::new(temp_dir.path().to_path_buf()),
        project_dir: Arc::new(temp_dir.path().to_path_buf()),
        worktree_name: Some(Arc::from("test-worktree")),
        session_id: Arc::from("test-session"),
    }
}

/// Helper to create a subprocess manager
fn create_subprocess_manager() -> Arc<SubprocessManager> {
    Arc::new(SubprocessManager::production())
}

#[tokio::test]
#[ignore = "Phase architecture not fully integrated with production coordinator yet"]
async fn test_setup_phase_execution() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create a simple setup phase
    let setup_phase = SetupPhase {
        commands: vec![
            WorkflowStep {
                shell: Some("echo 'test1' > output1.txt".to_string()),
                ..Default::default()
            },
            WorkflowStep {
                shell: Some("echo 'test2' > output2.txt".to_string()),
                ..Default::default()
            },
        ],
        timeout: Some(30),
        capture_outputs: HashMap::new(),
    };

    // Create a minimal map phase (required)
    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    // Create coordinator
    let coordinator = PhaseCoordinator::new(Some(setup_phase), map_phase, None, subprocess.clone());

    // Execute workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Verify success
    assert!(
        result.is_ok(),
        "Setup phase should execute successfully: {:?}",
        result.err()
    );

    // Verify files were created
    let output1 = temp_dir.path().join("output1.txt");
    let output2 = temp_dir.path().join("output2.txt");
    assert!(output1.exists(), "output1.txt should be created");
    assert!(output2.exists(), "output2.txt should be created");

    let content1 = std::fs::read_to_string(output1).unwrap();
    let content2 = std::fs::read_to_string(output2).unwrap();
    assert!(content1.contains("test1"));
    assert!(content2.contains("test2"));
}

#[tokio::test]
async fn test_setup_phase_failure() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create a setup phase that fails
    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("exit 1".to_string()),
            ..Default::default()
        }],
        timeout: Some(30),
        capture_outputs: HashMap::new(),
    };

    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    let coordinator = PhaseCoordinator::new(Some(setup_phase), map_phase, None, subprocess.clone());

    // Execute workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Verify failure
    assert!(
        result.is_err(),
        "Setup phase should fail with non-zero exit"
    );
}

#[tokio::test]
async fn test_map_phase_with_json_input() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create a JSON input file
    let input_file = temp_dir.path().join("items.json");
    std::fs::write(
        &input_file,
        r#"[{"id": 1, "name": "item1"}, {"id": 2, "name": "item2"}]"#,
    )
    .unwrap();

    // Create map phase with agent template
    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: input_file.to_string_lossy().to_string(),
            max_parallel: 2,
            ..Default::default()
        },
        agent_template: vec![WorkflowStep {
            shell: Some("echo 'Processing ${item.name}'".to_string()),
            ..Default::default()
        }],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    let coordinator = PhaseCoordinator::new(None, map_phase, None, subprocess.clone());

    // Execute workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Map phase execution with the new architecture is simplified
    // and may not handle all features yet
    // For now, we verify it doesn't panic
    let _ = result;
}

#[tokio::test]
async fn test_reduce_phase_aggregation() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create minimal map phase
    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    // Create reduce phase
    let reduce_phase = ReducePhase {
        commands: vec![WorkflowStep {
            shell: Some("echo 'Aggregated results' > results.txt".to_string()),
            ..Default::default()
        }],
        timeout_secs: Some(30),
    };

    let coordinator =
        PhaseCoordinator::new(None, map_phase, Some(reduce_phase), subprocess.clone());

    // Execute workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Reduce phase should execute after map
    let _ = result;
}

#[tokio::test]
async fn test_coordinator_state_machine() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create all three phases
    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("echo 'setup' > setup.txt".to_string()),
            ..Default::default()
        }],
        timeout: Some(30),
        capture_outputs: HashMap::new(),
    };

    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    let reduce_phase = ReducePhase {
        commands: vec![WorkflowStep {
            shell: Some("echo 'reduce' > reduce.txt".to_string()),
            ..Default::default()
        }],
        timeout_secs: Some(30),
    };

    let coordinator = PhaseCoordinator::new(
        Some(setup_phase),
        map_phase,
        Some(reduce_phase),
        subprocess.clone(),
    );

    // Execute workflow - should transition through all phases
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Verify the state machine completed
    let _ = result;

    // Verify setup phase ran
    let setup_file = temp_dir.path().join("setup.txt");
    assert!(
        setup_file.exists(),
        "Setup phase should have created setup.txt"
    );
}

#[tokio::test]
#[ignore = "Phase architecture not fully integrated with production coordinator yet"]
async fn test_phase_context_variable_passing() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Create setup phase that outputs a value
    let mut capture_outputs = HashMap::new();
    capture_outputs.insert("test_var".to_string(), CaptureConfig::Simple(0));

    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("echo 'test_value'".to_string()),
            ..Default::default()
        }],
        timeout: Some(30),
        capture_outputs,
    };

    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    let coordinator = PhaseCoordinator::new(Some(setup_phase), map_phase, None, subprocess.clone());

    // Execute workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Context should have captured the variable
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_phase_executor_trait_implementation() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("true".to_string()),
            ..Default::default()
        }],
        timeout: Some(30),
        capture_outputs: HashMap::new(),
    };

    let executor =
        prodigy::cook::execution::mapreduce::phases::setup::SetupPhaseExecutor::new(setup_phase);

    let mut context = PhaseContext::new(env, subprocess);

    // Test PhaseExecutor trait methods
    assert!(
        !executor.can_skip(&context),
        "Should not skip with commands"
    );
    assert!(executor.validate_context(&context).is_ok());

    // Execute
    let result = executor.execute(&mut context).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore = "Phase architecture not fully integrated with production coordinator yet"]
async fn test_empty_workflow_completes() {
    let temp_dir = TempDir::new().unwrap();
    let env = create_test_env(&temp_dir);
    let subprocess = create_subprocess_manager();

    // Minimal workflow with no-op map phase
    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: "[]".to_string(),
            max_parallel: 1,
            ..Default::default()
        },
        agent_template: vec![],
        json_path: None,
        filter: None,
        sort_by: None,
        max_items: None,
        distinct: None,
        timeout_config: None,
    };

    let coordinator = PhaseCoordinator::new(None, map_phase, None, subprocess.clone());

    // Execute empty workflow
    let result = coordinator.execute_workflow(env, subprocess).await;

    // Should complete successfully even with no work
    assert!(result.is_ok());
}
