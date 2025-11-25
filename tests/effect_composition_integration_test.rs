//! Integration tests for effect composition patterns
//!
//! These tests verify the correct composition of pure functions with effects
//! in the MapReduce execution pipeline. They test:
//! - Effect composition with and_then and map
//! - Error propagation through effect chains
//! - Session update effects
//! - Orchestrator planning to execution flow

use prodigy::config::mapreduce::{AgentTemplate, MapPhaseYaml, MapReduceWorkflowConfig};
use prodigy::config::WorkflowConfig;
use prodigy::cook::command::CookCommand;
use prodigy::cook::orchestrator::CookConfig;
use prodigy::core::orchestration::{
    calculate_resources, detect_execution_mode, plan_execution, ExecutionMode, Phase, PhaseType,
};
use prodigy::core::session::updates::{
    apply_session_update, apply_updates, ProgressUpdate, SessionUpdate, StepRecord,
};
use prodigy::core::session::validation::{is_terminal_status, valid_transitions_from};
use prodigy::unified_session::{SessionStatus, UnifiedSession};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// Test Fixtures
// ============================================================================

fn create_workflow_config() -> WorkflowConfig {
    WorkflowConfig {
        name: Some("test-workflow".to_string()),
        commands: vec![],
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
        merge: None,
    }
}

fn create_mapreduce_config(max_parallel: usize) -> MapReduceWorkflowConfig {
    MapReduceWorkflowConfig {
        name: "test-mapreduce".to_string(),
        mode: "mapreduce".to_string(),
        env: None,
        secrets: None,
        env_files: None,
        profiles: None,
        setup: None,
        map: MapPhaseYaml {
            input: "items.json".to_string(),
            json_path: "$.items[*]".to_string(),
            agent_template: AgentTemplate { commands: vec![] },
            max_parallel: max_parallel.to_string(),
            filter: None,
            sort_by: None,
            max_items: None,
            offset: None,
            distinct: None,
            agent_timeout_secs: None,
            timeout_config: None,
        },
        reduce: None,
        error_policy: Default::default(),
        on_item_failure: None,
        continue_on_failure: None,
        max_failures: None,
        failure_threshold: None,
        error_collection: None,
        merge: None,
    }
}

fn create_cook_config(mapreduce: bool, max_parallel: usize, dry_run: bool) -> CookConfig {
    let mapreduce_config = if mapreduce {
        Some(Arc::new(create_mapreduce_config(max_parallel)))
    } else {
        None
    };

    CookConfig {
        command: CookCommand {
            playbook: PathBuf::from("test-workflow.yml"),
            path: None,
            max_iterations: 1,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run,
            params: Default::default(),
        },
        project_path: Arc::new(PathBuf::from(".")),
        workflow: Arc::new(create_workflow_config()),
        mapreduce_config,
    }
}

fn create_test_session() -> UnifiedSession {
    let mut session = UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string());
    if let Some(ref mut wd) = session.workflow_data {
        wd.total_steps = 10;
    }
    session
}

// ============================================================================
// Orchestrator Flow Integration Tests
// ============================================================================

#[test]
fn test_orchestrator_flow_mode_to_plan_integration() {
    // Test the flow: CookConfig → mode detection → resource calculation → execution planning

    // Standard workflow
    let config = create_cook_config(false, 0, false);
    let mode = detect_execution_mode(&config);
    let resources = calculate_resources(&config, &mode);
    let plan = plan_execution(&config);

    assert_eq!(mode, ExecutionMode::Standard);
    assert_eq!(resources.worktrees, 0);
    assert_eq!(plan.mode, ExecutionMode::Standard);
    assert!(plan.has_phase(PhaseType::Commands));
}

#[test]
fn test_orchestrator_flow_mapreduce_integration() {
    // MapReduce workflow
    let config = create_cook_config(true, 10, false);
    let mode = detect_execution_mode(&config);
    let resources = calculate_resources(&config, &mode);
    let plan = plan_execution(&config);

    assert_eq!(mode, ExecutionMode::MapReduce);
    assert_eq!(resources.worktrees, 11); // 10 + 1 parent
    assert_eq!(resources.max_concurrent_commands, 10);
    assert_eq!(plan.mode, ExecutionMode::MapReduce);
    assert!(plan.has_phase(PhaseType::Map));
    assert_eq!(plan.parallel_budget, 10);
}

#[test]
fn test_orchestrator_flow_dryrun_integration() {
    // Dry run mode
    let config = create_cook_config(true, 10, true);
    let mode = detect_execution_mode(&config);
    let resources = calculate_resources(&config, &mode);
    let plan = plan_execution(&config);

    // Dry run takes priority over MapReduce
    assert_eq!(mode, ExecutionMode::DryRun);
    assert_eq!(resources.worktrees, 0);
    assert_eq!(plan.mode, ExecutionMode::DryRun);
    assert!(plan.has_phase(PhaseType::DryRunAnalysis));
    assert_eq!(plan.phases.len(), 1);
}

#[test]
fn test_orchestrator_plan_is_deterministic() {
    // Multiple calls with same config should produce identical plans
    let config = create_cook_config(true, 20, false);

    let plan1 = plan_execution(&config);
    let plan2 = plan_execution(&config);
    let plan3 = plan_execution(&config);

    assert_eq!(plan1, plan2);
    assert_eq!(plan2, plan3);

    // Verify specific values
    assert_eq!(plan1.parallel_budget, 20);
    assert_eq!(plan1.resource_needs.worktrees, 21);
}

// ============================================================================
// Session Update Composition Tests
// ============================================================================

#[test]
fn test_session_update_composition_sequential() {
    // Test sequential composition of updates
    let session = create_test_session();
    let original_id = session.id.clone();

    // Compose multiple updates
    let updates = vec![
        SessionUpdate::Status(SessionStatus::Running),
        SessionUpdate::Progress(ProgressUpdate {
            completed_steps: 3,
            failed_steps: 0,
            current_step: Some("step-1".to_string()),
        }),
        SessionUpdate::Variables({
            let mut m = HashMap::new();
            m.insert("result".to_string(), json!("success"));
            m
        }),
        SessionUpdate::AddStep(StepRecord::started("echo hello")),
    ];

    let result = apply_updates(session, updates);

    assert!(result.is_ok());
    let updated = result.unwrap();

    // Verify all updates were applied
    assert_eq!(updated.id, original_id);
    assert_eq!(updated.status, SessionStatus::Running);
    assert!(updated.metadata.contains_key("result"));
    assert!(updated.metadata.contains_key("current_step"));
    assert!(updated.metadata.contains_key("execution_steps"));
}

#[test]
fn test_session_update_error_stops_chain() {
    // Test that errors stop the update chain
    let session = create_test_session();

    let updates = vec![
        SessionUpdate::Status(SessionStatus::Running),
        // Invalid: can't go from Running back to Initializing
        SessionUpdate::Status(SessionStatus::Initializing),
        // This should never be applied
        SessionUpdate::Progress(ProgressUpdate {
            completed_steps: 100,
            failed_steps: 0,
            current_step: None,
        }),
    ];

    let result = apply_updates(session, updates);

    assert!(result.is_err());
}

#[test]
fn test_session_status_transition_chain() {
    // Test a valid status transition chain
    let mut session = create_test_session();

    // Initializing -> Running
    session = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running)).unwrap();
    assert_eq!(session.status, SessionStatus::Running);

    // Running -> Paused
    session = apply_session_update(session, SessionUpdate::Status(SessionStatus::Paused)).unwrap();
    assert_eq!(session.status, SessionStatus::Paused);

    // Paused -> Running
    session = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running)).unwrap();
    assert_eq!(session.status, SessionStatus::Running);

    // Running -> Completed
    session =
        apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed)).unwrap();
    assert_eq!(session.status, SessionStatus::Completed);
    assert!(session.completed_at.is_some());
    assert!(is_terminal_status(&session.status));
}

#[test]
fn test_session_update_preserves_immutability() {
    // Verify that original session is unchanged after updates
    let original = create_test_session();
    let original_id = original.id.clone();
    let original_status = original.status.clone();

    let updated = apply_session_update(
        original.clone(),
        SessionUpdate::Status(SessionStatus::Running),
    )
    .unwrap();

    // Original unchanged
    assert_eq!(original.id, original_id);
    assert_eq!(original.status, original_status);
    assert_eq!(original.status, SessionStatus::Initializing);

    // Updated has changes
    assert_eq!(updated.id, original_id);
    assert_eq!(updated.status, SessionStatus::Running);
}

// ============================================================================
// Variable Update Composition Tests
// ============================================================================

#[test]
fn test_variable_update_merge_semantics() {
    // Test that variable updates merge correctly
    let mut session = create_test_session();

    // Add first batch of variables
    let mut vars1 = HashMap::new();
    vars1.insert("key1".to_string(), json!("value1"));
    vars1.insert("key2".to_string(), json!(42));
    session = apply_session_update(session, SessionUpdate::Variables(vars1)).unwrap();

    // Add second batch (should merge, not replace)
    let mut vars2 = HashMap::new();
    vars2.insert("key3".to_string(), json!(true));
    vars2.insert("key2".to_string(), json!(100)); // Overwrites key2
    session = apply_session_update(session, SessionUpdate::Variables(vars2)).unwrap();

    // Verify merge semantics
    assert_eq!(session.metadata.get("key1"), Some(&json!("value1")));
    assert_eq!(session.metadata.get("key2"), Some(&json!(100))); // Overwritten
    assert_eq!(session.metadata.get("key3"), Some(&json!(true)));
}

#[test]
fn test_step_record_composition() {
    // Test that step records are properly accumulated
    let mut session = create_test_session();

    let steps = vec![
        StepRecord::started("step 1").complete(Some("output 1".to_string())),
        StepRecord::started("step 2").complete(Some("output 2".to_string())),
        StepRecord::started("step 3").fail("error on step 3"),
    ];

    for step in steps {
        session = apply_session_update(session, SessionUpdate::AddStep(step)).unwrap();
    }

    let execution_steps = session
        .metadata
        .get("execution_steps")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(execution_steps.len(), 3);

    // Verify order and status
    assert_eq!(
        execution_steps[0].get("status").and_then(|v| v.as_str()),
        Some("completed")
    );
    assert_eq!(
        execution_steps[2].get("status").and_then(|v| v.as_str()),
        Some("failed")
    );
}

// ============================================================================
// Pure Function to Effect Boundary Tests
// ============================================================================

#[test]
fn test_planning_pure_to_execution_boundary() {
    // Test that pure planning output is suitable for effect execution

    let config = create_cook_config(true, 5, false);
    let plan = plan_execution(&config);

    // Plan provides all necessary information for effects
    assert!(plan.requires_worktrees());
    assert_eq!(plan.parallel_budget, 5);
    assert!(plan.phase_count() > 0);

    // Verify phase details can be used by effects
    for phase in &plan.phases {
        if let Phase::Map {
            max_parallel,
            has_filter,
            has_sort,
        } = phase
        {
            assert_eq!(*max_parallel, 5);
            assert!(!has_filter); // Not configured in fixture
            assert!(!has_sort);
        }
    }
}

#[test]
fn test_validation_pure_functions_used_in_updates() {
    // Test that pure validation is correctly used in update effects

    // verify valid_transitions_from matches what apply_session_update accepts
    let session = create_test_session();
    let valid_next = valid_transitions_from(&session.status);

    // Should be exactly one valid transition from Initializing
    assert_eq!(valid_next, vec![SessionStatus::Running]);

    // Attempting valid transition should succeed
    let result = apply_session_update(
        session.clone(),
        SessionUpdate::Status(SessionStatus::Running),
    );
    assert!(result.is_ok());

    // Attempting invalid transition should fail
    let result = apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed));
    assert!(result.is_err());
}

// ============================================================================
// Work Planning Integration Tests
// ============================================================================

mod work_planning_integration {
    use prodigy::cook::execution::mapreduce::pure::work_planning::{
        plan_work_assignments, FilterExpression, WorkPlanConfig,
    };
    use serde_json::json;

    #[test]
    fn test_work_planning_full_pipeline() {
        // Test complete work planning flow

        // Simulate items from JSON input
        let items = vec![
            json!({"id": 1, "type": "a", "priority": 10}),
            json!({"id": 2, "type": "b", "priority": 5}),
            json!({"id": 3, "type": "a", "priority": 15}),
            json!({"id": 4, "type": "a", "priority": 3}),
            json!({"id": 5, "type": "b", "priority": 20}),
        ];

        // Filter to type "a" only
        let config = WorkPlanConfig {
            filter: Some(FilterExpression::Equals {
                field: "type".to_string(),
                value: json!("a"),
            }),
            offset: 0,
            max_items: Some(2),
        };

        let assignments = plan_work_assignments(items, &config);

        // Should have 2 items (filtered to type "a", limited to 2)
        assert_eq!(assignments.len(), 2);

        // Check that all are type "a"
        for assignment in &assignments {
            assert_eq!(assignment.item["type"], "a");
        }

        // Check sequential IDs
        assert_eq!(assignments[0].id, 0);
        assert_eq!(assignments[1].id, 1);

        // Check worktree names
        assert_eq!(assignments[0].worktree_name, "agent-0");
        assert_eq!(assignments[1].worktree_name, "agent-1");
    }

    #[test]
    fn test_work_planning_with_offset() {
        let items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
            json!({"id": 5}),
        ];

        let config = WorkPlanConfig {
            filter: None,
            offset: 2,
            max_items: Some(2),
        };

        let assignments = plan_work_assignments(items, &config);

        // Should skip first 2, take next 2
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].item["id"], 3);
        assert_eq!(assignments[1].item["id"], 4);
    }

    #[test]
    fn test_work_planning_complex_filter() {
        let items = vec![
            json!({"status": "active", "priority": 5}),
            json!({"status": "active", "priority": 15}),
            json!({"status": "inactive", "priority": 20}),
            json!({"status": "active", "priority": 25}),
        ];

        // Filter: status == "active" AND priority > 10
        let config = WorkPlanConfig {
            filter: Some(FilterExpression::And(vec![
                FilterExpression::Equals {
                    field: "status".to_string(),
                    value: json!("active"),
                },
                FilterExpression::GreaterThan {
                    field: "priority".to_string(),
                    value: json!(10),
                },
            ])),
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items, &config);

        // Should match items with status=active AND priority > 10
        assert_eq!(assignments.len(), 2);
        for assignment in &assignments {
            assert_eq!(assignment.item["status"], "active");
            assert!(assignment.item["priority"].as_i64().unwrap() > 10);
        }
    }
}

// ============================================================================
// Dependency Analysis Integration Tests
// ============================================================================

mod dependency_analysis_integration {
    use prodigy::cook::execution::mapreduce::pure::dependency_analysis::{
        analyze_dependencies, extract_variable_reads, extract_variable_writes, Command,
    };
    use std::collections::HashSet;

    #[test]
    fn test_dependency_analysis_real_commands() {
        // Simulate real shell commands and verify dependency detection

        let cmd1 = "export RESULT=$(process_item $INPUT)";
        let cmd2 = "echo $RESULT > output.txt";
        let cmd3 = "validate $RESULT && notify";

        let commands = vec![
            Command {
                reads: extract_variable_reads(cmd1),
                writes: extract_variable_writes(cmd1),
            },
            Command {
                reads: extract_variable_reads(cmd2),
                writes: extract_variable_writes(cmd2),
            },
            Command {
                reads: extract_variable_reads(cmd3),
                writes: extract_variable_writes(cmd3),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // cmd1 must be first (writes RESULT)
        // cmd2 and cmd3 both read RESULT but don't conflict, could be parallel
        assert!(!batches.is_empty());
        assert!(batches[0].contains(&0)); // cmd1 must be in first batch
    }

    #[test]
    fn test_dependency_analysis_parallel_independent() {
        // Independent commands should all be in one batch

        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: HashSet::new(),
                writes: ["B".to_string()].into_iter().collect(),
            },
            Command {
                reads: HashSet::new(),
                writes: ["C".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // All 3 commands are independent
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn test_dependency_analysis_diamond_pattern() {
        // Test diamond dependency pattern:
        //      0
        //     / \
        //    1   2
        //     \ /
        //      3

        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["B".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["C".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["B".to_string(), "C".to_string()].into_iter().collect(),
                writes: ["D".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // Expect: [0], [1,2], [3]
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![0]);
        assert_eq!(batches[1].len(), 2);
        assert!(batches[1].contains(&1));
        assert!(batches[1].contains(&2));
        assert_eq!(batches[2], vec![3]);
    }
}

// ============================================================================
// End-to-End Composition Tests
// ============================================================================

#[test]
fn test_end_to_end_workflow_session_lifecycle() {
    // Simulate complete workflow lifecycle with pure functions

    // 1. Create config and plan
    let config = create_cook_config(true, 5, false);
    let plan = plan_execution(&config);

    // 2. Create session
    let mut session = UnifiedSession::new_workflow("test-job".to_string(), "test".to_string());
    if let Some(ref mut wd) = session.workflow_data {
        wd.total_steps = plan.phase_count();
    }

    // 3. Start execution
    session = apply_session_update(session, SessionUpdate::Status(SessionStatus::Running)).unwrap();

    // 4. Simulate phase execution with progress updates
    for (i, phase) in plan.phases.iter().enumerate() {
        // Record step start
        let step = StepRecord::started(format!("{}", phase));
        session = apply_session_update(session, SessionUpdate::AddStep(step)).unwrap();

        // Record progress
        session = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 1,
                failed_steps: 0,
                current_step: Some(format!("phase-{}", i)),
            }),
        )
        .unwrap();
    }

    // 5. Complete session
    session =
        apply_session_update(session, SessionUpdate::Status(SessionStatus::Completed)).unwrap();

    // Verify final state
    assert_eq!(session.status, SessionStatus::Completed);
    assert!(session.completed_at.is_some());
    assert!(is_terminal_status(&session.status));

    // Verify step records
    let steps = session
        .metadata
        .get("execution_steps")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(steps.len(), plan.phase_count());
}
