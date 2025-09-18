//! Unit tests for the phase coordinator

use super::*;
use crate::cook::execution::mapreduce::{MapPhase, MapReduceConfig, ReducePhase, SetupPhase};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use crate::subprocess::SubprocessManager;
use std::path::PathBuf;
use std::sync::Arc;

fn create_test_environment() -> ExecutionEnvironment {
    ExecutionEnvironment {
        working_dir: PathBuf::from("/tmp/test"),
        project_dir: PathBuf::from("/tmp/test"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    }
}

fn create_test_setup_phase() -> SetupPhase {
    SetupPhase {
        commands: vec![
            WorkflowStep {
                command: "shell: echo 'Setup'".to_string(),
                on_failure: None,
                on_success: None,
                timeout: None,
                commit_required: false,
                capture_output: Default::default(),
            },
        ],
    }
}

fn create_test_map_phase() -> MapPhase {
    MapPhase {
        config: MapReduceConfig {
            input: "test_items.json".to_string(),
            json_path: "$.items[*]".to_string(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: Some(10),
            offset: None,
        },
        agent_template: vec![],
        filter: None,
        sort_by: None,
        commit_required: false,
        fail_on_error: false,
    }
}

fn create_test_reduce_phase() -> ReducePhase {
    ReducePhase {
        commands: vec![
            WorkflowStep {
                command: "shell: echo 'Reduce'".to_string(),
                on_failure: None,
                on_success: None,
                timeout: None,
                commit_required: false,
                capture_output: Default::default(),
            },
        ],
    }
}

#[test]
fn test_phase_coordinator_creation() {
    let setup = create_test_setup_phase();
    let map = create_test_map_phase();
    let reduce = create_test_reduce_phase();

    let coordinator = PhaseCoordinator::new(
        Some(setup),
        map,
        Some(reduce),
        Arc::new(SubprocessManager::production()),
    );

    assert!(coordinator.setup_executor.is_some());
    assert!(coordinator.reduce_executor.is_some());
}

#[test]
fn test_phase_coordinator_creation_without_optional_phases() {
    let map = create_test_map_phase();

    let coordinator = PhaseCoordinator::new(
        None,
        map,
        None,
        Arc::new(SubprocessManager::production()),
    );

    assert!(coordinator.setup_executor.is_none());
    assert!(coordinator.reduce_executor.is_none());
}

#[test]
fn test_phase_coordinator_with_custom_transition_handler() {
    struct TestTransitionHandler;

    impl PhaseTransitionHandler for TestTransitionHandler {
        fn on_phase_complete(&self, _phase: PhaseType, _result: &PhaseResult) {
            // Custom logic
        }

        fn on_phase_error(&self, _phase: PhaseType, _error: &PhaseError) -> PhaseTransition {
            PhaseTransition::Continue
        }
    }

    let map = create_test_map_phase();
    let coordinator = PhaseCoordinator::new(
        None,
        map,
        None,
        Arc::new(SubprocessManager::production()),
    )
    .with_transition_handler(Box::new(TestTransitionHandler));

    // Coordinator created with custom handler
    assert!(coordinator.setup_executor.is_none());
}

#[test]
fn test_default_transition_handler_on_complete() {
    let handler = DefaultTransitionHandler;
    let result = PhaseResult {
        phase_type: PhaseType::Map,
        success: true,
        data: None,
        error_message: None,
        metrics: PhaseMetrics {
            duration_secs: 1.0,
            items_processed: 10,
            items_successful: 8,
            items_failed: 2,
        },
    };

    // Should not panic
    handler.on_phase_complete(PhaseType::Map, &result);
}

#[test]
fn test_default_transition_handler_on_error() {
    let handler = DefaultTransitionHandler;
    let error = PhaseError::ExecutionFailed {
        message: "Test error".to_string(),
    };

    let transition = handler.on_phase_error(PhaseType::Map, &error);
    assert!(matches!(transition, PhaseTransition::Error(_)));
}

#[test]
fn test_default_transition_handler_on_validation_error() {
    let handler = DefaultTransitionHandler;
    let error = PhaseError::ValidationError {
        message: "Validation failed".to_string(),
    };

    let transition = handler.on_phase_error(PhaseType::Setup, &error);
    assert!(matches!(transition, PhaseTransition::Error(_)));
}

#[test]
fn test_phase_transition_variants() {
    // Test Continue variant
    let cont = PhaseTransition::Continue(PhaseType::Map);
    assert!(matches!(cont, PhaseTransition::Continue(_)));

    // Test Skip variant
    let skip = PhaseTransition::Skip(PhaseType::Reduce);
    if let PhaseTransition::Skip(phase) = skip {
        assert_eq!(phase, PhaseType::Reduce);
    } else {
        panic!("Expected Skip variant");
    }

    // Test Retry variant
    let retry = PhaseTransition::Retry { max_attempts: 3 };
    if let PhaseTransition::Retry { max_attempts } = retry {
        assert_eq!(max_attempts, 3);
    } else {
        panic!("Expected Retry variant");
    }

    // Test Error variant
    let error = PhaseTransition::Error("Test error".to_string());
    if let PhaseTransition::Error(msg) = error {
        assert_eq!(msg, "Test error");
    } else {
        panic!("Expected Error variant");
    }
}

#[test]
fn test_phase_type_equality() {
    assert_eq!(PhaseType::Setup, PhaseType::Setup);
    assert_eq!(PhaseType::Map, PhaseType::Map);
    assert_eq!(PhaseType::Reduce, PhaseType::Reduce);
    assert_ne!(PhaseType::Setup, PhaseType::Map);
    assert_ne!(PhaseType::Map, PhaseType::Reduce);
    assert_ne!(PhaseType::Setup, PhaseType::Reduce);
}

#[test]
fn test_phase_metrics_creation() {
    let metrics = PhaseMetrics {
        duration_secs: 10.5,
        items_processed: 100,
        items_successful: 95,
        items_failed: 5,
    };

    assert_eq!(metrics.duration_secs, 10.5);
    assert_eq!(metrics.items_processed, 100);
    assert_eq!(metrics.items_successful, 95);
    assert_eq!(metrics.items_failed, 5);
}

#[test]
fn test_phase_result_success() {
    let result = PhaseResult {
        phase_type: PhaseType::Map,
        success: true,
        data: Some(serde_json::json!({"test": "data"})),
        error_message: None,
        metrics: PhaseMetrics {
            duration_secs: 5.0,
            items_processed: 50,
            items_successful: 50,
            items_failed: 0,
        },
    };

    assert!(result.success);
    assert!(result.error_message.is_none());
    assert!(result.data.is_some());
}

#[test]
fn test_phase_result_failure() {
    let result = PhaseResult {
        phase_type: PhaseType::Setup,
        success: false,
        data: None,
        error_message: Some("Setup failed".to_string()),
        metrics: PhaseMetrics {
            duration_secs: 2.0,
            items_processed: 0,
            items_successful: 0,
            items_failed: 1,
        },
    };

    assert!(!result.success);
    assert!(result.error_message.is_some());
    assert!(result.data.is_none());
}