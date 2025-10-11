//! Test to reproduce setup phase working directory bug
//!
//! This test demonstrates that the setup phase executor should execute commands
//! in the MapReduce worktree, not in the main repository.
//!
//! ## The Bug
//!
//! When MapReduce creates a worktree for the setup phase and passes an ExecutionEnvironment
//! with working_dir set to the worktree path, the WorkflowExecutorImpl's execute_step method
//! overrides this with step.working_dir if it's set. This causes commands to execute in the
//! main repository instead of the isolated worktree.
//!
//! ## Expected Behavior (Spec 127)
//!
//! All MapReduce phases (setup, map, reduce) must execute in isolated worktrees, never in
//! the main repository. The ExecutionEnvironment's working_dir should be respected.

use crate::cook::execution::setup_executor::SetupPhaseExecutor;
use crate::cook::execution::SetupPhase;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{StepResult, WorkflowContext, WorkflowStep};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// Mock executor that captures the working directory used during execution
struct WorkingDirCapturingExecutor {
    captured_working_dir: Option<PathBuf>,
}

impl WorkingDirCapturingExecutor {
    fn new() -> Self {
        Self {
            captured_working_dir: None,
        }
    }

    fn get_captured_dir(&self) -> Option<&PathBuf> {
        self.captured_working_dir.as_ref()
    }
}

#[async_trait]
impl crate::cook::workflow::StepExecutor for WorkingDirCapturingExecutor {
    async fn execute_step(
        &mut self,
        _step: &WorkflowStep,
        env: &ExecutionEnvironment,
        _context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Capture the working directory passed to the executor
        self.captured_working_dir = Some(env.working_dir.as_ref().clone());

        // Return a successful result
        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: "test output".to_string(),
            stderr: String::new(),
            json_log_location: None,
        })
    }
}

#[tokio::test]
async fn test_setup_phase_uses_worktree_directory() {
    // Setup: Define paths
    let main_repo = PathBuf::from("/Users/test/project");
    let worktree_path = PathBuf::from("/Users/test/.prodigy/worktrees/session-123/worktree");

    // Create setup phase with a simple shell command
    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("echo 'test'".to_string()),
            ..Default::default()
        }],
        timeout: None,
        capture_outputs: Default::default(),
    };

    // Create setup executor
    let mut setup_executor = SetupPhaseExecutor::new(&setup_phase);

    // Create mock executor to capture working directory
    let mut mock_executor = WorkingDirCapturingExecutor::new();

    // Create execution environment with worktree path
    // This simulates what MapReduce does: creates an environment with the worktree path
    let env = ExecutionEnvironment {
        working_dir: Arc::new(worktree_path.clone()),
        project_dir: Arc::new(main_repo.clone()),
        session_id: Arc::from("test-session"),
        worktree_name: Some(Arc::from("test-worktree")),
    };

    let mut context = WorkflowContext::default();

    // Execute setup phase
    let result = setup_executor
        .execute(&setup_phase.commands, &mut mock_executor, &env, &mut context)
        .await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Setup phase should succeed");

    // BUG: The captured working directory should be the worktree path,
    // but it's currently the main repo path due to the environment manager override
    let captured_dir = mock_executor
        .get_captured_dir()
        .expect("Working directory should be captured");

    assert_eq!(
        captured_dir, &worktree_path,
        "Setup phase should execute in the worktree directory, not the main repo.\n\
         Expected: {}\n\
         Got: {}",
        worktree_path.display(),
        captured_dir.display()
    );
}

#[tokio::test]
async fn test_setup_phase_preserves_environment_working_dir() {
    // This test verifies that when an ExecutionEnvironment is passed with a specific
    // working_dir, that directory is used for command execution, not overridden

    let worktree_path = PathBuf::from("/tmp/test-worktree");
    let main_repo = PathBuf::from("/tmp/main-repo");

    let setup_phase = SetupPhase {
        commands: vec![WorkflowStep {
            shell: Some("pwd".to_string()),
            ..Default::default()
        }],
        timeout: None,
        capture_outputs: Default::default(),
    };

    let mut setup_executor = SetupPhaseExecutor::new(&setup_phase);
    let mut mock_executor = WorkingDirCapturingExecutor::new();

    // Environment explicitly set to worktree
    let env = ExecutionEnvironment {
        working_dir: Arc::new(worktree_path.clone()),
        project_dir: Arc::new(main_repo),
        session_id: Arc::from("test"),
        worktree_name: Some(Arc::from("test-worktree")),
    };

    let mut context = WorkflowContext::default();

    setup_executor
        .execute(&setup_phase.commands, &mut mock_executor, &env, &mut context)
        .await
        .unwrap();

    let captured = mock_executor.get_captured_dir().unwrap();

    assert_eq!(
        captured, &worktree_path,
        "Setup phase must not override the ExecutionEnvironment's working_dir.\n\
         The worktree isolation guarantee depends on this."
    );
}

/// Note on the Real Bug Location
///
/// These tests currently PASS because SetupPhaseExecutor itself doesn't have the bug.
/// The bug is in `WorkflowExecutorImpl::execute_step` (src/cook/workflow/executor.rs:1481):
///
/// ```rust
/// let working_dir_override = step.working_dir.clone();
/// ```
///
/// This line takes the step's working_dir field and uses it to override the
/// ExecutionEnvironment's working_dir. When the EnvironmentManager is initialized
/// with the main repo path (which it often is), it sets working_dir back to the
/// main repo even though the ExecutionEnvironment correctly points to the worktree.
///
/// ## The Fix
///
/// The fix should be in one of these places:
///
/// 1. **EnvironmentManager** (src/cook/environment/manager.rs:140-144):
///    Don't use self.current_dir as fallback if the ExecutionEnvironment already
///    has a working_dir that's different. Respect the environment.
///
/// 2. **WorkflowExecutor** (src/cook/workflow/executor.rs:1481):
///    Check if we're in a worktree context before applying step.working_dir override.
///    Don't override if env.worktree_name.is_some().
///
/// 3. **Setup Phase Execution** (where WorkflowExecutorImpl is created):
///    Ensure the EnvironmentManager is initialized with the worktree path, not main repo.
///
/// The logs show:
/// ```
/// INFO Executing in worktree: /.../.prodigy/worktrees/.../session-mapreduce-... (validated)
/// INFO Working directory overridden to: /Users/glen/memento-mori/prodigy
/// ```
///
/// The first line is from SetupPhaseExecutor validation - correct!
/// The second line is from WorkflowExecutorImpl - BUG!
#[allow(dead_code)]
const BUG_DOCUMENTATION: () = ();
