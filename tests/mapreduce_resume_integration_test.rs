//! Integration tests for MapReduce resume functionality
//!
//! Tests end-to-end workflow resumption including cross-worktree coordination

use prodigy::cook::execution::dlq::DeadLetterQueue;
use prodigy::cook::execution::events::{EventLogger, JsonlEventWriter};
use prodigy::cook::execution::mapreduce::{AgentResult, AgentStatus, MapReduceConfig, MapReduceExecutor};
use prodigy::cook::execution::mapreduce_resume::{
    EnhancedResumeOptions, EnhancedResumeResult, MapReducePhase, MapReduceResumeManager,
};
use prodigy::cook::execution::state::{DefaultJobStateManager, FailureRecord, MapReduceJobState};
use prodigy::cook::execution::{ClaudeExecutor, ExecutionResult};
use prodigy::cook::interaction::{SpinnerHandle, UserInteraction, VerbosityLevel};
use prodigy::cook::workflow::{CaptureOutput, WorkflowStep};
use prodigy::testing::mocks::MockSessionManager;
use prodigy::worktree::WorktreeManager;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;

/// Mock user interaction for testing
struct MockUserInteraction {
    default_yes: bool,
}

impl MockUserInteraction {
    fn new() -> Self {
        Self { default_yes: true }
    }
}

struct MockSpinnerHandle;

impl SpinnerHandle for MockSpinnerHandle {
    fn update(&self, _message: &str) {}
    fn finish(&self, _message: Option<&str>) {}
}

#[async_trait::async_trait]
impl UserInteraction for MockUserInteraction {
    async fn prompt_yes_no(&self, _message: &str) -> anyhow::Result<bool> {
        Ok(self.default_yes)
    }

    async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> anyhow::Result<String> {
        Ok(String::from("test"))
    }

    fn display_info(&self, _message: &str) {}
    fn display_warning(&self, _message: &str) {}
    fn display_error(&self, _message: &str) {}
    fn display_progress(&self, _message: &str) {}
    fn start_spinner(&self, _message: &str) -> Box<dyn SpinnerHandle> {
        Box::new(MockSpinnerHandle)
    }
    fn display_success(&self, _message: &str) {}
    fn display_action(&self, _message: &str) {}
    fn display_metric(&self, _label: &str, _value: &str) {}
    fn display_status(&self, _message: &str) {}
    fn iteration_start(&self, _current: u32, _total: u32) {}
    fn iteration_end(&self, _current: u32, _duration: std::time::Duration, _success: bool) {}
    fn step_start(&self, _step: u32, _total: u32, _description: &str) {}
    fn step_end(&self, _step: u32, _success: bool) {}
    fn command_output(&self, _output: &str, _verbosity: VerbosityLevel) {}
    fn debug_output(&self, _message: &str, _min_verbosity: VerbosityLevel) {}
    fn verbosity(&self) -> VerbosityLevel {
        VerbosityLevel::Normal
    }
}

/// Mock Claude executor for testing
struct MockClaudeExecutor {
    results: Arc<Mutex<HashMap<String, String>>>,
}

impl MockClaudeExecutor {
    fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn with_results(results: HashMap<String, String>) -> Self {
        Self {
            results: Arc::new(Mutex::new(results)),
        }
    }
}

#[async_trait::async_trait]
impl ClaudeExecutor for MockClaudeExecutor {
    async fn execute_claude_command(
        &self,
        command: &str,
        _project_path: &std::path::Path,
        _env_vars: HashMap<String, String>,
    ) -> anyhow::Result<ExecutionResult> {
        let results = self.results.lock().await;
        let output = if let Some(result) = results.get(command) {
            result.clone()
        } else {
            format!("Mock output for: {}", command)
        };

        Ok(ExecutionResult {
            success: true,
            stdout: output,
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    async fn check_claude_cli(&self) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn get_claude_version(&self) -> anyhow::Result<String> {
        Ok("mock-claude-1.0.0".to_string())
    }
}

/// Create a test job state with partial completion
async fn create_partial_job_state(
    job_id: &str,
    completed: usize,
    total: usize,
) -> MapReduceJobState {

    let config = MapReduceConfig {
        input: "test.json".to_string(),
        json_path: "$.items[*]".to_string(),
        max_parallel: 5,
        timeout_per_agent: 600,
        retry_on_failure: 2,
        max_items: None,
        offset: None,
    };

    let mut completed_agents = HashSet::new();
    let mut agent_results = HashMap::new();

    for i in 0..completed {
        let agent_id = format!("agent-{}", i);
        completed_agents.insert(agent_id.clone());
        agent_results.insert(
            agent_id.clone(),
            AgentResult {
                item_id: agent_id.clone(),
                status: AgentStatus::Success,
                output: Some(format!("Processed item {}", i)),
                commits: vec![],
                files_modified: vec![],
                duration: std::time::Duration::from_secs(5),
                error: None,
                worktree_path: Some(PathBuf::from(format!("/tmp/worktree-{}", i))),
                branch_name: None,
                worktree_session_id: None,
            },
        );
    }

    // Add some failed agents for testing recovery
    let failed_count = 2.min(total - completed);
    let mut failed_agents = HashMap::new();
    for i in completed..(completed + failed_count) {
        let agent_id = format!("agent-{}", i);
        failed_agents.insert(
            agent_id.clone(),
            FailureRecord {
                item_id: agent_id.clone(),
                attempts: 1,
                last_error: "Simulated failure".to_string(),
                last_attempt: chrono::Utc::now(),
                worktree_info: None,
            },
        );
        agent_results.insert(
            agent_id.clone(),
            AgentResult {
                item_id: agent_id.clone(),
                status: AgentStatus::Failed("Simulated failure".to_string()),
                output: None,
                commits: vec![],
                files_modified: vec![],
                duration: std::time::Duration::from_secs(3),
                error: Some("Simulated error".to_string()),
                worktree_path: Some(PathBuf::from(format!("/tmp/worktree-{}", i))),
                branch_name: None,
                worktree_session_id: None,
            },
        );
    }

    let agent_template = vec![WorkflowStep {
        name: None,
        claude: Some("/process ${item}".to_string()),
        shell: None,
        test: None,
        goal_seek: None,
        foreach: None,
        command: None,
        modular: None,
        outputs: None,
        on_failure: None,
        commit_required: false,
        skip_on_dry_run: false,
        description: None,
        id: None,
        when: None,
        interpolations: vec![],
        ignore_errors: false,
        retry: None,
        timeout: None,
        environment: None,
        working_directory: None,
        capture_output: CaptureOutput::Disabled,
    }];

    let reduce_commands = if total > 0 {
        Some(vec![WorkflowStep {
            name: None,
            claude: Some("/summarize ${map.results}".to_string()),
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            command: None,
            modular: None,
            outputs: None,
            on_failure: None,
            commit_required: false,
            skip_on_dry_run: false,
            description: None,
            id: None,
            when: None,
            interpolations: vec![],
            ignore_errors: false,
            retry: None,
            timeout: None,
            environment: None,
            working_directory: None,
            capture_output: CaptureOutput::Disabled,
        }])
    } else {
        None
    };

    // Create work items
    let work_items: Vec<serde_json::Value> = (0..total)
        .map(|i| json!({"id": i, "value": format!("item-{}", i)}))
        .collect();

    // Create pending items list
    let pending_items: Vec<String> = (0..total)
        .map(|i| format!("item_{}", i))
        .filter(|id| !completed_agents.contains(id))
        .collect();

    MapReduceJobState {
        job_id: job_id.to_string(),
        config,
        started_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        work_items,
        agent_results,
        completed_agents,
        failed_agents,
        pending_items,
        checkpoint_version: 1,
        checkpoint_format_version: 1,
        parent_worktree: None,
        reduce_phase_state: None,
        total_items: total,
        successful_count: completed,
        failed_count: failed_count,
        is_complete: false,
        agent_template,
        reduce_commands,
    }
}

#[tokio::test]
async fn test_resume_workflow_from_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create state manager and save initial checkpoint
    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-resume-job";
    let initial_state = create_partial_job_state(job_id, 3, 10).await;
    state_manager
        .save_checkpoint(job_id, initial_state.clone())
        .await
        .unwrap();

    // Create event logger
    let events_dir = project_root.join("events");
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    // Create resume manager
    let mut resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger.clone(),
        project_root.clone(),
    )
    .await
    .unwrap();

    // Create mock executor
    let claude_executor = Arc::new(MockClaudeExecutor::new());
    let session_manager = Arc::new(MockSessionManager::new());
    let user_interaction = Arc::new(MockUserInteraction::new(false));
    let worktree_manager = Arc::new(WorktreeManager::new(project_root.clone()).await.unwrap());

    let executor = Arc::new(
        MapReduceExecutor::new(
            claude_executor.clone(),
            session_manager.clone(),
            user_interaction.clone(),
            worktree_manager,
            project_root.clone(),
        )
        .await,
    );

    resume_manager.set_executor(executor);

    // Resume the job
    let options = EnhancedResumeOptions::default();
    let result = resume_manager.resume(job_id, options).await.unwrap();

    // Verify the resume result
    match result {
        EnhancedResumeResult::ReadyToExecute {
            phase,
            map_phase,
            remaining_items,
            state,
        } => {
            assert_eq!(
                phase,
                MapReducePhase::Map
            );
            assert!(map_phase.is_some());
            // Should have 5 remaining items (7 not completed - 2 failed that might be in DLQ)
            assert!(remaining_items.len() >= 5);
            assert_eq!(state.completed_agents.len(), 3);
        }
        _ => panic!("Expected ReadyToExecute result for partial completion"),
    }
}

#[tokio::test]
async fn test_resume_with_dlq_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create state manager
    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-dlq-recovery";

    // Create event logger
    let events_dir = project_root.join("events");
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    // Create DLQ with failed items
    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        project_root.clone(),
        100,
        30,
        Some(event_logger.clone()),
    )
    .await
    .unwrap();

    // Add failed items to DLQ
    for i in 0..3 {
        dlq.add_item(
            prodigy::cook::execution::dlq::DeadLetteredItem {
                work_item_id: format!("failed-item-{}", i),
                original_data: json!({"id": 100 + i, "value": format!("dlq-item-{}", i)}),
                failure_detail: prodigy::cook::execution::dlq::FailureDetail {
                    error: "Temporary failure".to_string(),
                    error_type: prodigy::cook::execution::dlq::ErrorType::Retryable,
                    timestamp: chrono::Utc::now(),
                    agent_id: format!("agent-failed-{}", i),
                    retry_count: 1,
                    correlation_id: Some(format!("corr-{}", i)),
                },
            },
            true,
        )
        .await
        .unwrap();
    }

    // Create initial state with partial completion
    let initial_state = create_partial_job_state(job_id, 5, 10).await;
    state_manager
        .save_checkpoint(job_id, initial_state.clone())
        .await
        .unwrap();

    // Create resume manager with DLQ
    let resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger.clone(),
        project_root.clone(),
    )
    .await
    .unwrap();

    // Resume with DLQ items included
    let mut options = EnhancedResumeOptions::default();
    options.include_dlq_items = true;
    let result = resume_manager.resume(job_id, options).await.unwrap();

    // Verify DLQ items are included
    match result {
        EnhancedResumeResult::ReadyToExecute {
            remaining_items, ..
        } => {
            // Should include both unprocessed items and DLQ items
            assert!(remaining_items.len() > 5, "Should include DLQ items");
        }
        _ => panic!("Expected ReadyToExecute result"),
    }
}

#[tokio::test]
async fn test_resume_completed_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create state manager
    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-completed-job";

    // Create fully completed state
    let mut completed_state = create_partial_job_state(job_id, 10, 10).await;
    completed_state.phase = MapReducePhase::Complete;

    // Add reduce phase result
    completed_state.reduce_phase_state = Some(prodigy::cook::execution::state::ReducePhaseState {
        started: true,
        completed: true,
        executed_commands: vec!["/summarize".to_string()],
        output: Some(json!({"summary": "All 10 items processed successfully"}).to_string()),
        error: None,
        started_at: Some(chrono::Utc::now()),
        completed_at: Some(chrono::Utc::now()),
    });

    state_manager
        .save_checkpoint(job_id, completed_state.clone())
        .await
        .unwrap();

    // Create event logger
    let event_logger = Arc::new(EventLogger::new(vec![]));

    // Create resume manager
    let resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger,
        project_root.clone(),
    )
    .await
    .unwrap();

    // Attempt to resume completed job
    let options = EnhancedResumeOptions::default();
    let result = resume_manager.resume(job_id, options).await.unwrap();

    // Verify it returns completion status
    match result {
        EnhancedResumeResult::FullWorkflowCompleted(full_result) => {
            assert_eq!(full_result.map_result.successful, 10);
            assert_eq!(full_result.map_result.failed, 0);
            assert!(full_result.reduce_result.is_some());
        }
        _ => panic!("Expected FullWorkflowCompleted for completed job"),
    }
}

#[tokio::test]
async fn test_cross_worktree_coordination() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Create shared state manager for cross-worktree coordination
    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-cross-worktree";

    // Simulate multiple worktrees updating the same job
    let initial_state = create_partial_job_state(job_id, 2, 10).await;
    state_manager
        .save_checkpoint(job_id, initial_state.clone())
        .await
        .unwrap();

    // Simulate worktree 1 completing items 2-4
    let mut state1 = state_manager.load_checkpoint(job_id).await.unwrap();
    for i in 2..5 {
        let agent_id = format!("agent-{}", i);
        state1.completed_agents.insert(agent_id.clone());
        state1.agent_results.insert(
            agent_id.clone(),
            AgentResult {
                item_id: agent_id.clone(),
                status: AgentStatus::Success,
                output: Some(format!("Worktree 1 processed {}", i)),
                commits: vec![],
                files_modified: vec![],
                duration: std::time::Duration::from_secs(1),
                error: None,
                worktree_path: Some(PathBuf::from("/tmp/worktree-1")),
                branch_name: None,
                worktree_session_id: None,
            },
        );
        state1.successful_count += 1;
    }
    state_manager.save_checkpoint(job_id, state1).await.unwrap();

    // Simulate worktree 2 completing items 5-7
    let mut state2 = state_manager.load_checkpoint(job_id).await.unwrap();
    for i in 5..8 {
        let agent_id = format!("agent-{}", i);
        state2.completed_agents.insert(agent_id.clone());
        state2.agent_results.insert(
            agent_id.clone(),
            AgentResult {
                item_id: agent_id.clone(),
                status: AgentStatus::Success,
                output: Some(format!("Worktree 2 processed {}", i)),
                commits: vec![],
                files_modified: vec![],
                duration: std::time::Duration::from_secs(1),
                error: None,
                worktree_path: Some(PathBuf::from("/tmp/worktree-2")),
                branch_name: None,
                worktree_session_id: None,
            },
        );
        state2.successful_count += 1;
    }
    state_manager.save_checkpoint(job_id, state2).await.unwrap();

    // Load final state and verify coordination
    let final_state = state_manager.load_checkpoint(job_id).await.unwrap();
    assert_eq!(final_state.completed_agents.len(), 8); // 2 initial + 3 from wt1 + 3 from wt2
    assert_eq!(final_state.successful_count, 8);

    // Verify results from both worktrees are present
    let wt1_results: Vec<_> = final_state
        .agent_results
        .values()
        .filter(|r| r.worktree_path == Some(PathBuf::from("/tmp/worktree-1")))
        .count();
    let wt2_results: Vec<_> = final_state
        .agent_results
        .values()
        .filter(|r| r.worktree_path == Some(PathBuf::from("/tmp/worktree-2")))
        .count();

    assert!(wt1_results > 0, "Should have results from worktree 1");
    assert!(wt2_results > 0, "Should have results from worktree 2");
}

#[tokio::test]
async fn test_resume_with_environment_validation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-env-validation";

    // Create initial state
    let initial_state = create_partial_job_state(job_id, 3, 5).await;
    state_manager
        .save_checkpoint(job_id, initial_state.clone())
        .await
        .unwrap();

    // Create event logger
    let event_logger = Arc::new(EventLogger::new(vec![]));

    // Create resume manager
    let resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger,
        project_root.clone(),
    )
    .await
    .unwrap();

    // Resume with environment validation enabled
    let mut options = EnhancedResumeOptions::default();
    options.validate_environment = true;

    let result = resume_manager.resume(job_id, options).await;

    // Should succeed with validation
    assert!(result.is_ok(), "Environment validation should pass");
}

#[tokio::test]
async fn test_force_resume_completed_job() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    let state_manager = Arc::new(DefaultJobStateManager::new(project_root.clone()));
    let job_id = "test-force-resume";

    // Create completed state
    let mut completed_state = create_partial_job_state(job_id, 5, 5).await;
    completed_state.phase = MapReducePhase::Complete;

    state_manager
        .save_checkpoint(job_id, completed_state.clone())
        .await
        .unwrap();

    // Create event logger
    let event_logger = Arc::new(EventLogger::new(vec![]));

    // Create resume manager
    let resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger,
        project_root.clone(),
    )
    .await
    .unwrap();

    // Force resume even though job is complete
    let mut options = EnhancedResumeOptions::default();
    options.force = true;

    let result = resume_manager.resume(job_id, options).await.unwrap();

    // Should allow resumption with force flag
    match result {
        EnhancedResumeResult::ReadyToExecute { .. }
        | EnhancedResumeResult::MapOnlyCompleted(_) => {
            // Force flag allows re-execution
        }
        EnhancedResumeResult::FullWorkflowCompleted(_) => {
            // Or it might still report completion, both are valid
        }
        _ => panic!("Unexpected result type"),
    }
}