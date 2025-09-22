//! Tests for retry state management during checkpoint and resume operations

use super::retry_state::*;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::{UserInteraction, VerbosityLevel};
use crate::cook::retry_v2::{BackoffStrategy, RetryConfig};
use crate::cook::session::{SessionManager, SessionState, SessionSummary, SessionUpdate};
use crate::cook::workflow::checkpoint::{
    CheckpointManager, CompletedStep, ExecutionState, WorkflowCheckpoint, WorkflowStatus,
};
// Remove executor imports - module is private
use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

/// Mock ClaudeExecutor for testing
#[allow(dead_code)]
struct MockClaudeExecutor;

#[async_trait]
impl ClaudeExecutor for MockClaudeExecutor {
    async fn execute_claude_command(
        &self,
        _command: &str,
        _project_path: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<crate::cook::execution::ExecutionResult> {
        Ok(crate::cook::execution::ExecutionResult {
            success: true,
            exit_code: Some(0),
            stdout: "Mock execution".to_string(),
            stderr: String::new(),
        })
    }

    async fn check_claude_cli(&self) -> Result<bool> {
        Ok(true)
    }

    async fn get_claude_version(&self) -> Result<String> {
        Ok("Mock Claude v1.0".to_string())
    }
}

/// Mock SessionManager for testing
#[allow(dead_code)]
struct MockSessionManager;

#[async_trait]
impl SessionManager for MockSessionManager {
    async fn save_state(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    async fn load_state(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    async fn load_session(&self, _session_id: &str) -> Result<SessionState> {
        Ok(SessionState::new(
            "test".to_string(),
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp")),
        ))
    }
    async fn start_session(&self, _session_id: &str) -> Result<()> {
        Ok(())
    }

    async fn update_session(&self, _update: SessionUpdate) -> Result<()> {
        Ok(())
    }

    async fn complete_session(&self) -> Result<SessionSummary> {
        Ok(SessionSummary {
            iterations: 1,
            files_changed: 0,
        })
    }

    fn get_state(&self) -> Result<SessionState> {
        Ok(SessionState::new(
            "test".to_string(),
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp")),
        ))
    }

    async fn save_checkpoint(&self, _state: &SessionState) -> Result<()> {
        Ok(())
    }

    async fn list_resumable(&self) -> Result<Vec<crate::cook::session::SessionInfo>> {
        Ok(Vec::new())
    }

    async fn get_last_interrupted(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Mock UserInteraction for testing
#[allow(dead_code)]
struct MockUserInteraction;

#[async_trait]
impl UserInteraction for MockUserInteraction {
    async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
        Ok(true)
    }

    async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
        Ok("test".to_string())
    }

    fn display_info(&self, _message: &str) {}
    fn display_warning(&self, _message: &str) {}
    fn display_error(&self, _message: &str) {}
    fn display_success(&self, _message: &str) {}
    fn display_progress(&self, _message: &str) {}
    fn display_metric(&self, _label: &str, _value: &str) {}
    fn display_action(&self, _message: &str) {}
    fn display_status(&self, _message: &str) {}
    fn iteration_start(&self, _current: u32, _total: u32) {}
    fn iteration_end(&self, _current: u32, _duration: Duration, _success: bool) {}
    fn step_start(&self, _step: u32, _total: u32, _description: &str) {}
    fn step_end(&self, _step: u32, _success: bool) {}
    fn command_output(&self, _output: &str, _verbosity: VerbosityLevel) {}
    fn debug_output(&self, _message: &str, _min_verbosity: VerbosityLevel) {}
    fn verbosity(&self) -> VerbosityLevel {
        VerbosityLevel::Normal
    }
    fn start_spinner(&self, _message: &str) -> Box<dyn crate::cook::interaction::SpinnerHandle> {
        Box::new(MockSpinnerHandle)
    }
}

#[allow(dead_code)]
struct MockSpinnerHandle;
impl crate::cook::interaction::SpinnerHandle for MockSpinnerHandle {
    fn update_message(&mut self, _message: &str) {}
    fn success(&mut self, _message: &str) {}
    fn fail(&mut self, _message: &str) {}
}

#[tokio::test]
async fn test_retry_state_persistence_and_restoration() {
    let manager = RetryStateManager::new();

    // Create a retry attempt
    let attempt = RetryAttempt {
        attempt_number: 1,
        executed_at: Utc::now(),
        duration: Duration::from_secs(2),
        success: false,
        error: Some("Test error".to_string()),
        backoff_applied: Duration::from_secs(0),
        exit_code: Some(1),
    };

    let config = RetryConfig::default();
    manager
        .update_retry_state("test_cmd", attempt.clone(), &config)
        .await
        .unwrap();

    // Create checkpoint state
    let checkpoint = manager.create_checkpoint_state().await.unwrap();
    assert_eq!(checkpoint.command_retry_states.len(), 1);
    assert!(checkpoint.command_retry_states.contains_key("test_cmd"));

    // Create new manager and restore
    let new_manager = RetryStateManager::new();
    new_manager
        .restore_from_checkpoint(&checkpoint)
        .await
        .unwrap();

    // Verify state was restored
    let restored = new_manager.get_command_retry_state("test_cmd").await;
    assert!(restored.is_some());
    let state = restored.unwrap();
    assert_eq!(state.attempt_count, 1);
    assert_eq!(state.retry_history.len(), 1);
}

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let manager = RetryStateManager::new();

    // Simulate failures to open circuit
    for i in 0..5 {
        let attempt = RetryAttempt {
            attempt_number: i + 1,
            executed_at: Utc::now(),
            duration: Duration::from_secs(1),
            success: false,
            error: Some(format!("Failed attempt {}", i + 1)),
            backoff_applied: Duration::from_secs(i as u64),
            exit_code: Some(1),
        };

        let config = RetryConfig {
            attempts: 10, // High limit to test circuit breaker
            ..Default::default()
        };
        manager
            .update_retry_state("test_cmd", attempt, &config)
            .await
            .unwrap();
    }

    // Check circuit is open
    let can_retry = manager.can_retry("test_cmd").await.unwrap();
    assert!(!can_retry, "Circuit should be open after 5 failures");

    // Verify state in checkpoint
    let checkpoint = manager.create_checkpoint_state().await.unwrap();
    let cb_state = checkpoint.circuit_breaker_states.get("test_cmd").unwrap();
    assert_eq!(cb_state.state, CircuitState::Open);
    assert_eq!(cb_state.failure_count, 5);
}

#[tokio::test]
async fn test_retry_budget_enforcement() {
    let manager = RetryStateManager::new();

    let config = RetryConfig {
        retry_budget: Some(Duration::from_secs(5)),
        attempts: 100, // High limit to test budget
        ..Default::default()
    };

    // Create initial attempt
    let attempt = RetryAttempt {
        attempt_number: 1,
        executed_at: Utc::now() - ChronoDuration::seconds(10),
        duration: Duration::from_secs(1),
        success: false,
        error: Some("Failed".to_string()),
        backoff_applied: Duration::from_secs(0),
        exit_code: Some(1),
    };

    manager
        .update_retry_state("budget_cmd", attempt, &config)
        .await
        .unwrap();

    // Manually expire budget for testing
    {
        let command_states = manager.get_command_states();
        let mut states = command_states.write().await;
        if let Some(state) = states.get_mut("budget_cmd") {
            state.retry_budget_expires_at = Some(Utc::now() - ChronoDuration::seconds(1));
        }
    }

    // Check retry is not allowed
    let can_retry = manager.can_retry("budget_cmd").await.unwrap();
    assert!(!can_retry, "Should not retry after budget expired");
}

#[tokio::test]
async fn test_backoff_strategy_calculation() {
    let manager = RetryStateManager::new();

    // Test exponential backoff
    let config = RetryConfig {
        attempts: 5,
        backoff: BackoffStrategy::Exponential { base: 2.0 },
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        jitter: false,
        ..Default::default()
    };

    // Create multiple attempts
    for i in 0..3 {
        let attempt = RetryAttempt {
            attempt_number: i + 1,
            executed_at: Utc::now(),
            duration: Duration::from_secs(1),
            success: false,
            error: Some(format!("Attempt {}", i + 1)),
            backoff_applied: Duration::from_secs(0),
            exit_code: Some(1),
        };

        manager
            .update_retry_state("backoff_cmd", attempt, &config)
            .await
            .unwrap();
    }

    // Check backoff state
    let state = manager
        .get_command_retry_state("backoff_cmd")
        .await
        .unwrap();
    assert_eq!(state.attempt_count, 3);
    // The backoff delay should have increased exponentially
    assert!(state.backoff_state.current_delay > Duration::from_secs(1));
}

#[tokio::test]
async fn test_checkpoint_with_retry_state_integration() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());

    // Create a checkpoint with retry state
    let retry_state = RetryCheckpointState {
        command_retry_states: HashMap::from([(
            "test_cmd".to_string(),
            CommandRetryState {
                command_id: "test_cmd".to_string(),
                attempt_count: 2,
                max_attempts: 3,
                last_attempt_at: Some(Utc::now()),
                next_retry_at: Some(Utc::now() + ChronoDuration::seconds(5)),
                backoff_state: BackoffState {
                    strategy: BackoffStrategy::Fixed,
                    current_delay: Duration::from_secs(5),
                    base_delay: Duration::from_secs(5),
                    max_delay: Duration::from_secs(60),
                    multiplier: 2.0,
                    jitter_enabled: false,
                    jitter_factor: 0.0,
                    fibonacci_prev: None,
                    fibonacci_curr: None,
                },
                retry_history: vec![
                    RetryAttempt {
                        attempt_number: 1,
                        executed_at: Utc::now() - ChronoDuration::seconds(10),
                        duration: Duration::from_secs(1),
                        success: false,
                        error: Some("First failure".to_string()),
                        backoff_applied: Duration::from_secs(0),
                        exit_code: Some(1),
                    },
                    RetryAttempt {
                        attempt_number: 2,
                        executed_at: Utc::now() - ChronoDuration::seconds(5),
                        duration: Duration::from_secs(1),
                        success: false,
                        error: Some("Second failure".to_string()),
                        backoff_applied: Duration::from_secs(5),
                        exit_code: Some(1),
                    },
                ],
                retry_config: None,
                is_circuit_broken: false,
                retry_budget_expires_at: None,
                total_retry_duration: Duration::from_secs(2),
            },
        )]),
        global_retry_config: None,
        retry_execution_history: Vec::new(),
        circuit_breaker_states: HashMap::new(),
        retry_correlation_map: HashMap::new(),
        checkpointed_at: Utc::now(),
    };

    let checkpoint = WorkflowCheckpoint {
        workflow_id: "test-workflow".to_string(),
        execution_state: ExecutionState {
            current_step_index: 1,
            total_steps: 3,
            status: WorkflowStatus::Interrupted,
            start_time: Utc::now(),
            last_checkpoint: Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps: vec![CompletedStep {
            step_index: 0,
            command: "claude: test command 1".to_string(),
            success: true,
            output: Some("Success".to_string()),
            captured_variables: HashMap::new(),
            duration: Duration::from_secs(5),
            completed_at: Utc::now(),
            retry_state: None,
        }],
        variable_state: HashMap::new(),
        mapreduce_state: None,
        timestamp: Utc::now(),
        version: 1,
        workflow_hash: "test-hash".to_string(),
        total_steps: 3,
        workflow_name: Some("test-workflow".to_string()),
        workflow_path: None,
        error_recovery_state: None,
        retry_checkpoint_state: Some(retry_state.clone()),
        variable_checkpoint_state: None,
    };

    // Save checkpoint
    checkpoint_manager
        .save_checkpoint(&checkpoint)
        .await
        .unwrap();

    // Load checkpoint
    let loaded = checkpoint_manager
        .load_checkpoint("test-workflow")
        .await
        .unwrap();
    assert!(loaded.retry_checkpoint_state.is_some());

    let loaded_retry_state = loaded.retry_checkpoint_state.unwrap();
    assert_eq!(loaded_retry_state.command_retry_states.len(), 1);
    assert!(loaded_retry_state
        .command_retry_states
        .contains_key("test_cmd"));

    let cmd_state = &loaded_retry_state.command_retry_states["test_cmd"];
    assert_eq!(cmd_state.attempt_count, 2);
    assert_eq!(cmd_state.retry_history.len(), 2);
}

#[tokio::test]
async fn test_retry_state_consistency_validation() {
    let manager = RetryStateManager::new();

    // Create inconsistent checkpoint state
    let checkpoint = RetryCheckpointState {
        command_retry_states: HashMap::from([(
            "bad_cmd".to_string(),
            CommandRetryState {
                command_id: "bad_cmd".to_string(),
                attempt_count: 10, // More than max
                max_attempts: 3,
                last_attempt_at: Some(Utc::now()),
                next_retry_at: None,
                backoff_state: BackoffState {
                    strategy: BackoffStrategy::Fixed,
                    current_delay: Duration::from_secs(5),
                    base_delay: Duration::from_secs(5),
                    max_delay: Duration::from_secs(60),
                    multiplier: 2.0,
                    jitter_enabled: false,
                    jitter_factor: 0.0,
                    fibonacci_prev: None,
                    fibonacci_curr: None,
                },
                retry_history: vec![], // Empty history despite attempts
                retry_config: None,
                is_circuit_broken: false,
                retry_budget_expires_at: None,
                total_retry_duration: Duration::from_secs(0),
            },
        )]),
        global_retry_config: None,
        retry_execution_history: Vec::new(),
        circuit_breaker_states: HashMap::new(),
        retry_correlation_map: HashMap::new(),
        checkpointed_at: Utc::now(),
    };

    // Should fail validation
    let result = manager.restore_from_checkpoint(&checkpoint).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Inconsistent retry state"));
}

#[tokio::test]
async fn test_circuit_breaker_half_open_transition() {
    let manager = RetryStateManager::new();

    // Create a checkpoint with an open circuit breaker that should transition to half-open
    let checkpoint = RetryCheckpointState {
        command_retry_states: HashMap::new(),
        global_retry_config: None,
        retry_execution_history: Vec::new(),
        circuit_breaker_states: HashMap::from([(
            "test_cmd".to_string(),
            CircuitBreakerState {
                state: CircuitState::Open,
                failure_count: 5,
                failure_threshold: 5,
                last_failure_at: Some(Utc::now() - ChronoDuration::seconds(120)), // 2 minutes ago
                recovery_timeout: Duration::from_secs(60), // 1 minute recovery
                half_open_max_calls: 3,
                half_open_success_count: 0,
            },
        )]),
        retry_correlation_map: HashMap::new(),
        checkpointed_at: Utc::now() - ChronoDuration::seconds(120),
    };

    // Restore checkpoint
    manager.restore_from_checkpoint(&checkpoint).await.unwrap();

    // Check circuit breaker state - should be half-open
    let breakers_arc = manager.get_circuit_breakers();
    let breakers = breakers_arc.read().await;
    let breaker = breakers.get("test_cmd").unwrap();
    assert_eq!(breaker.state, CircuitState::HalfOpen);
}

#[tokio::test]
async fn test_retry_summary_generation() {
    let manager = RetryStateManager::new();

    // Create retry states for multiple commands
    for i in 0..3 {
        let cmd_id = format!("cmd_{}", i);
        for j in 0..i + 1 {
            let attempt = RetryAttempt {
                attempt_number: j + 1,
                executed_at: Utc::now(),
                duration: Duration::from_secs(1),
                success: j == i,
                error: if j == i {
                    None
                } else {
                    Some("Failed".to_string())
                },
                backoff_applied: Duration::from_secs(j as u64),
                exit_code: if j == i { Some(0) } else { Some(1) },
            };

            let config = RetryConfig {
                attempts: 5,
                ..Default::default()
            };
            manager
                .update_retry_state(&cmd_id, attempt, &config)
                .await
                .unwrap();
        }
    }

    // Get retry summary
    let summary = manager.get_retry_summary().await;
    assert_eq!(summary.len(), 3);

    // Check summary details
    assert_eq!(summary["cmd_0"], (1, 5, false));
    assert_eq!(summary["cmd_1"], (2, 5, false));
    assert_eq!(summary["cmd_2"], (3, 5, false));
}
