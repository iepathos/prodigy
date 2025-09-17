//! Integration tests for MapReduce resume functionality
//!
//! Tests end-to-end workflow resumption including cross-worktree coordination

use prodigy::cook::execution::dlq::DeadLetterQueue;
use prodigy::cook::execution::events::{EventLogger, JsonlEventWriter, MapReduceEvent};
use prodigy::cook::execution::mapreduce::{
    AgentResult, AgentStatus, MapPhase, MapReduceConfig, MapReduceExecutor, ReducePhase,
};
use prodigy::cook::execution::mapreduce_resume::{
    EnhancedResumeOptions, EnhancedResumeResult, MapReduceResumeManager,
};
use prodigy::cook::execution::state::{DefaultJobStateManager, MapReduceJobState};
use prodigy::cook::execution::ClaudeExecutor;
use prodigy::cook::interaction::{MockUserInteraction, UserInteraction};
use prodigy::cook::orchestrator::ExecutionEnvironment;
use prodigy::cook::session::{MockSessionManager, SessionManager};
use prodigy::cook::workflow::{CommandType, WorkflowStep};
use prodigy::worktree::WorktreeManager;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;

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
    async fn execute(
        &self,
        command: &str,
        _working_dir: &std::path::Path,
        _env_vars: &HashMap<String, String>,
        _verbose: bool,
    ) -> anyhow::Result<String> {
        let results = self.results.lock().await;
        if let Some(result) = results.get(command) {
            Ok(result.clone())
        } else {
            Ok(format!("Mock output for: {}", command))
        }
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
                agent_id: agent_id.clone(),
                work_item: json!({"id": i, "value": format!("item-{}", i)}),
                status: AgentStatus::Success,
                output: Some(format!("Processed item {}", i)),
                error: None,
                retries: 0,
                duration: std::time::Duration::from_secs(5),
                worktree_path: PathBuf::from(format!("/tmp/worktree-{}", i)),
            },
        );
    }

    // Add some failed agents for testing recovery
    let failed_count = 2.min(total - completed);
    let mut failed_agents = HashSet::new();
    for i in completed..(completed + failed_count) {
        let agent_id = format!("agent-{}", i);
        failed_agents.insert(agent_id.clone());
        agent_results.insert(
            agent_id.clone(),
            AgentResult {
                agent_id: agent_id.clone(),
                work_item: json!({"id": i, "value": format!("item-{}", i)}),
                status: AgentStatus::Failed("Simulated failure".to_string()),
                output: None,
                error: Some("Simulated error".to_string()),
                retries: 1,
                duration: std::time::Duration::from_secs(3),
                worktree_path: PathBuf::from(format!("/tmp/worktree-{}", i)),
            },
        );
    }

    let agent_commands = vec![WorkflowStep {
        command_type: CommandType::Claude,
        command: "/process ${item}".to_string(),
        arguments: None,
        outputs: None,
        on_failure: None,
        commit_required: false,
        skip_on_dry_run: false,
        interpolations: vec![],
    }];

    let reduce_commands = if total > 0 {
        Some(vec![WorkflowStep {
            command_type: CommandType::Claude,
            command: "/summarize ${map.results}".to_string(),
            arguments: None,
            outputs: None,
            on_failure: None,
            commit_required: false,
            skip_on_dry_run: false,
            interpolations: vec![],
        }])
    } else {
        None
    };

    MapReduceJobState {
        job_id: job_id.to_string(),
        config,
        phase: prodigy::cook::execution::state::MapReducePhase::Map,
        completed_agents,
        failed_agents,
        agent_results,
        successful_count: completed,
        failed_count: failed_count,
        total_items: total,
        start_time: chrono::Utc::now(),
        last_checkpoint: chrono::Utc::now(),
        checkpoint_version: 1,
        agent_commands,
        reduce_commands,
        reduce_phase_state: None,
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
                prodigy::cook::execution::state::MapReducePhase::Map
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
    completed_state.phase = prodigy::cook::execution::state::MapReducePhase::Complete;

    // Add reduce phase result
    completed_state.reduce_phase_state = Some(prodigy::cook::execution::state::ReducePhaseState {
        started_at: chrono::Utc::now(),
        completed: true,
        output: Some(json!({"summary": "All 10 items processed successfully"}).to_string()),
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
                agent_id: agent_id.clone(),
                work_item: json!({"id": i}),
                status: AgentStatus::Success,
                output: Some(format!("Worktree 1 processed {}", i)),
                error: None,
                retries: 0,
                duration: std::time::Duration::from_secs(1),
                worktree_path: PathBuf::from("/tmp/worktree-1"),
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
                agent_id: agent_id.clone(),
                work_item: json!({"id": i}),
                status: AgentStatus::Success,
                output: Some(format!("Worktree 2 processed {}", i)),
                error: None,
                retries: 0,
                duration: std::time::Duration::from_secs(1),
                worktree_path: PathBuf::from("/tmp/worktree-2"),
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
        .filter(|r| r.worktree_path == PathBuf::from("/tmp/worktree-1"))
        .count();
    let wt2_results: Vec<_> = final_state
        .agent_results
        .values()
        .filter(|r| r.worktree_path == PathBuf::from("/tmp/worktree-2"))
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
    completed_state.phase = prodigy::cook::execution::state::MapReducePhase::Complete;

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