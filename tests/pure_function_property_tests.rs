//! Property tests for pure function modules
//!
//! This file contains comprehensive property tests that verify:
//! - Determinism: same input → same output
//! - Idempotence: f(f(x)) == f(x) where applicable
//! - Laws: semigroup, monoid laws where applicable
//!
//! These tests use proptest for automated property verification.

use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

// ============================================================================
// Orchestration Module Property Tests
// ============================================================================

mod execution_planning {
    use super::*;
    use prodigy::config::mapreduce::{AgentTemplate, MapPhaseYaml, MapReduceWorkflowConfig};
    use prodigy::config::WorkflowConfig;
    use prodigy::cook::command::CookCommand;
    use prodigy::cook::orchestrator::CookConfig;
    use prodigy::core::orchestration::{plan_execution, ExecutionMode, Phase};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_default_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            name: None,
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn create_config_with_parallel(max_parallel: usize, dry_run: bool) -> CookConfig {
        let mr_config = MapReduceWorkflowConfig {
            name: "test".to_string(),
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
        };

        CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("workflow.yml"),
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
            workflow: Arc::new(create_default_workflow_config()),
            mapreduce_config: Some(Arc::new(mr_config)),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Execution planning is deterministic
        #[test]
        fn prop_planning_is_deterministic(
            max_parallel in 1usize..100,
            dry_run: bool,
        ) {
            let config = create_config_with_parallel(max_parallel, dry_run);

            let plan1 = plan_execution(&config);
            let plan2 = plan_execution(&config);

            prop_assert_eq!(plan1, plan2, "Planning must be deterministic");
        }

        /// Property: Parallel budget never exceeds max_parallel
        #[test]
        fn prop_parallel_budget_bounded(max_parallel in 1usize..100) {
            let config = create_config_with_parallel(max_parallel, false);
            let plan = plan_execution(&config);

            prop_assert!(plan.parallel_budget <= max_parallel);
        }

        /// Property: MapReduce mode always has Map phase
        #[test]
        fn prop_mapreduce_has_map_phase(max_parallel in 1usize..100) {
            let config = create_config_with_parallel(max_parallel, false);
            let plan = plan_execution(&config);

            prop_assert_eq!(plan.mode, ExecutionMode::MapReduce);
            let has_map_phase = plan.phases.iter().any(|p| matches!(p, Phase::Map { .. }));
            prop_assert!(has_map_phase);
        }

        /// Property: DryRun mode has exactly one DryRunAnalysis phase
        #[test]
        fn prop_dryrun_has_analysis_phase(max_parallel in 1usize..100) {
            let config = create_config_with_parallel(max_parallel, true);
            let plan = plan_execution(&config);

            prop_assert_eq!(plan.mode, ExecutionMode::DryRun);
            prop_assert_eq!(plan.phases.len(), 1);
            let is_dry_run_phase = matches!(plan.phases[0], Phase::DryRunAnalysis);
            prop_assert!(is_dry_run_phase);
        }

        /// Property: Worktree count is max_parallel + 1 for MapReduce
        #[test]
        fn prop_worktree_count_correct(max_parallel in 1usize..100) {
            let config = create_config_with_parallel(max_parallel, false);
            let plan = plan_execution(&config);

            prop_assert_eq!(plan.resource_needs.worktrees, max_parallel + 1);
        }
    }
}

mod mode_detection {
    use super::*;
    use prodigy::config::WorkflowConfig;
    use prodigy::cook::command::CookCommand;
    use prodigy::cook::orchestrator::CookConfig;
    use prodigy::core::orchestration::{detect_execution_mode, ExecutionMode};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_default_config() -> CookConfig {
        CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("workflow.yml"),
                path: None,
                max_iterations: 1,
                map: vec![],
                args: vec![],
                fail_fast: false,
                auto_accept: false,
                resume: None,
                verbosity: 0,
                quiet: false,
                dry_run: false,
                params: Default::default(),
            },
            project_path: Arc::new(PathBuf::from(".")),
            workflow: Arc::new(WorkflowConfig {
                name: None,
                commands: vec![],
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: None,
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Mode detection is deterministic
        #[test]
        fn prop_mode_detection_deterministic(
            dry_run: bool,
            has_args: bool,
        ) {
            let mut config = create_default_config();
            config.command.dry_run = dry_run;
            if has_args {
                config.command.args = vec!["arg1".to_string()];
            }

            let mode1 = detect_execution_mode(&config);
            let mode2 = detect_execution_mode(&config);

            prop_assert_eq!(mode1, mode2);
        }

        /// Property: DryRun takes priority over all other modes
        #[test]
        fn prop_dryrun_priority(has_args: bool) {
            let mut config = create_default_config();
            config.command.dry_run = true;
            if has_args {
                config.command.args = vec!["arg1".to_string()];
            }

            let mode = detect_execution_mode(&config);
            prop_assert_eq!(mode, ExecutionMode::DryRun);
        }

        /// Property: Standard mode is default when no special config
        #[test]
        fn prop_standard_is_default(_dummy: bool) {
            let config = create_default_config();
            let mode = detect_execution_mode(&config);
            prop_assert_eq!(mode, ExecutionMode::Standard);
        }
    }
}

mod resource_allocation {
    use super::*;
    use prodigy::config::mapreduce::{AgentTemplate, MapPhaseYaml, MapReduceWorkflowConfig};
    use prodigy::config::WorkflowConfig;
    use prodigy::cook::command::CookCommand;
    use prodigy::cook::orchestrator::CookConfig;
    use prodigy::core::orchestration::{calculate_resources, ExecutionMode, ResourceRequirements};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_config_with_parallel(max_parallel: usize) -> CookConfig {
        let mr_config = MapReduceWorkflowConfig {
            name: "test".to_string(),
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
        };

        CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("workflow.yml"),
                path: None,
                max_iterations: 1,
                map: vec![],
                args: vec![],
                fail_fast: false,
                auto_accept: false,
                resume: None,
                verbosity: 0,
                quiet: false,
                dry_run: false,
                params: Default::default(),
            },
            project_path: Arc::new(PathBuf::from(".")),
            workflow: Arc::new(WorkflowConfig {
                name: None,
                commands: vec![],
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: Some(Arc::new(mr_config)),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Resource calculation is deterministic
        #[test]
        fn prop_resource_calculation_deterministic(max_parallel in 1usize..100) {
            let config = create_config_with_parallel(max_parallel);
            let mode = ExecutionMode::MapReduce;

            let res1 = calculate_resources(&config, &mode);
            let res2 = calculate_resources(&config, &mode);

            prop_assert_eq!(res1, res2);
        }

        /// Property: Memory estimate scales with parallelism
        #[test]
        fn prop_memory_scales_with_parallelism(
            parallel1 in 1usize..50,
            parallel2 in 51usize..100,
        ) {
            let config1 = create_config_with_parallel(parallel1);
            let config2 = create_config_with_parallel(parallel2);
            let mode = ExecutionMode::MapReduce;

            let res1 = calculate_resources(&config1, &mode);
            let res2 = calculate_resources(&config2, &mode);

            prop_assert!(res1.memory_estimate < res2.memory_estimate);
        }

        /// Property: fits_within is symmetric for equality boundary
        #[test]
        fn prop_fits_within_boundary(
            worktrees in 1usize..20,
            memory in 100_000_000usize..1_000_000_000,
            disk in 1_000_000_000usize..10_000_000_000,
        ) {
            let resources = ResourceRequirements {
                worktrees,
                memory_estimate: memory,
                disk_space: disk,
                max_concurrent_commands: 1,
            };

            // Exact match should fit
            prop_assert!(resources.fits_within(worktrees, memory, disk));

            // Larger limits should fit
            prop_assert!(resources.fits_within(worktrees + 1, memory + 1, disk + 1));
        }
    }
}

// ============================================================================
// Workflow Pure Module Property Tests
// ============================================================================

mod variable_expansion {
    use super::*;
    use prodigy::cook::workflow::pure::variable_expansion::{
        expand_variables, extract_variable_references,
    };

    // Generator for valid variable names
    fn valid_var_name() -> impl Strategy<Value = String> {
        r"[a-zA-Z_][a-zA-Z0-9_]{0,20}".prop_filter("non-empty", |s| !s.is_empty())
    }

    // Generator for safe variable values (no special chars)
    fn safe_value() -> impl Strategy<Value = String> {
        r"[a-zA-Z0-9 _-]{0,50}"
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Variable expansion is deterministic
        #[test]
        fn prop_expansion_deterministic(
            template in ".*",
            vars in prop::collection::hash_map(valid_var_name(), safe_value(), 0..5),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&template, &vars);

            prop_assert_eq!(result1, result2);
        }

        /// Property: Empty template always returns empty string
        #[test]
        fn prop_empty_template_returns_empty(
            vars in prop::collection::hash_map(valid_var_name(), safe_value(), 0..5),
        ) {
            let result = expand_variables("", &vars);
            prop_assert_eq!(result, "");
        }

        /// Property: Empty variables returns template unchanged (for non-variable templates)
        #[test]
        fn prop_no_vars_preserves_template(template in "[a-zA-Z0-9 ]{0,50}") {
            let empty_vars: HashMap<String, String> = HashMap::new();
            let result = expand_variables(&template, &empty_vars);
            prop_assert_eq!(result, template);
        }

        /// Property: Extracted references are valid identifiers
        #[test]
        fn prop_extracted_refs_valid(template in ".*") {
            let refs = extract_variable_references(&template);

            for var_name in refs {
                prop_assert!(!var_name.is_empty());
                let first = var_name.chars().next().unwrap();
                prop_assert!(first.is_ascii_alphabetic() || first == '_');
                prop_assert!(var_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            }
        }

        /// Property: Variable reference extraction is deterministic
        #[test]
        fn prop_reference_extraction_deterministic(template in ".*") {
            let refs1 = extract_variable_references(&template);
            let refs2 = extract_variable_references(&template);

            prop_assert_eq!(refs1, refs2);
        }
    }
}

// ============================================================================
// Session Module Property Tests
// ============================================================================

mod session_validation {
    use super::*;
    use prodigy::core::session::validation::{
        is_terminal_status, valid_transitions_from, validate_status_transition,
    };
    use prodigy::unified_session::SessionStatus;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Terminal statuses have no valid transitions
        #[test]
        fn prop_terminal_no_transitions(_dummy: bool) {
            let terminal = vec![
                SessionStatus::Completed,
                SessionStatus::Failed,
                SessionStatus::Cancelled,
            ];

            for status in terminal {
                prop_assert!(is_terminal_status(&status));
                prop_assert!(valid_transitions_from(&status).is_empty());
            }
        }

        /// Property: Non-terminal statuses have at least one valid transition
        #[test]
        fn prop_non_terminal_has_transitions(_dummy: bool) {
            let non_terminal = vec![
                SessionStatus::Initializing,
                SessionStatus::Running,
                SessionStatus::Paused,
            ];

            for status in non_terminal {
                prop_assert!(!is_terminal_status(&status));
                prop_assert!(!valid_transitions_from(&status).is_empty());
            }
        }

        /// Property: Self-transitions are always invalid
        #[test]
        fn prop_self_transition_invalid(_dummy: bool) {
            let all_statuses = vec![
                SessionStatus::Initializing,
                SessionStatus::Running,
                SessionStatus::Paused,
                SessionStatus::Completed,
                SessionStatus::Failed,
                SessionStatus::Cancelled,
            ];

            for status in all_statuses {
                let result = validate_status_transition(&status, &status);
                prop_assert!(result.is_err());
            }
        }

        /// Property: validate_status_transition is deterministic
        #[test]
        fn prop_validation_deterministic(_dummy: bool) {
            let statuses = vec![
                SessionStatus::Initializing,
                SessionStatus::Running,
                SessionStatus::Paused,
                SessionStatus::Completed,
            ];

            for from in &statuses {
                for to in &statuses {
                    let result1 = validate_status_transition(from, to);
                    let result2 = validate_status_transition(from, to);

                    prop_assert_eq!(result1.is_ok(), result2.is_ok());
                }
            }
        }
    }
}

// ============================================================================
// Work Planning Property Tests
// ============================================================================

mod work_planning {
    use super::*;
    use prodigy::cook::execution::mapreduce::pure::work_planning::{
        plan_work_assignments, WorkPlanConfig,
    };
    use serde_json::json;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Work planning is deterministic
        #[test]
        fn prop_planning_deterministic(
            item_count in 0usize..20,
            offset in 0usize..5,
            max_items in prop::option::of(1usize..10),
        ) {
            let items: Vec<_> = (0..item_count).map(|i| json!({"id": i})).collect();
            let config = WorkPlanConfig {
                filter: None,
                offset,
                max_items,
            };

            let result1 = plan_work_assignments(items.clone(), &config);
            let result2 = plan_work_assignments(items, &config);

            prop_assert_eq!(result1, result2);
        }

        /// Property: Result count respects max_items limit
        #[test]
        fn prop_max_items_respected(
            item_count in 5usize..20,
            max_items in 1usize..5,
        ) {
            let items: Vec<_> = (0..item_count).map(|i| json!({"id": i})).collect();
            let config = WorkPlanConfig {
                filter: None,
                offset: 0,
                max_items: Some(max_items),
            };

            let result = plan_work_assignments(items, &config);

            prop_assert!(result.len() <= max_items);
        }

        /// Property: Offset skips correct number of items
        #[test]
        fn prop_offset_correct(
            item_count in 10usize..20,
            offset in 0usize..5,
        ) {
            let items: Vec<_> = (0..item_count).map(|i| json!({"id": i})).collect();
            let config = WorkPlanConfig {
                filter: None,
                offset,
                max_items: None,
            };

            let result = plan_work_assignments(items, &config);

            // First result should be item at index 'offset' (if it exists)
            if offset < item_count && !result.is_empty() {
                prop_assert_eq!(&result[0].item["id"], &json!(offset));
            }
        }

        /// Property: Each assignment has unique worktree name
        #[test]
        fn prop_unique_worktree_names(item_count in 1usize..20) {
            let items: Vec<_> = (0..item_count).map(|i| json!({"id": i})).collect();
            let config = WorkPlanConfig {
                filter: None,
                offset: 0,
                max_items: None,
            };

            let result = plan_work_assignments(items, &config);
            let names: HashSet<_> = result.iter().map(|a| &a.worktree_name).collect();

            prop_assert_eq!(names.len(), result.len());
        }

        /// Property: Assignment IDs are sequential starting from 0
        #[test]
        fn prop_sequential_ids(item_count in 1usize..20) {
            let items: Vec<_> = (0..item_count).map(|i| json!({"id": i})).collect();
            let config = WorkPlanConfig {
                filter: None,
                offset: 0,
                max_items: None,
            };

            let result = plan_work_assignments(items, &config);

            for (expected_id, assignment) in result.iter().enumerate() {
                prop_assert_eq!(assignment.id, expected_id);
            }
        }
    }
}

// ============================================================================
// Dependency Analysis Property Tests
// ============================================================================

mod dependency_analysis {
    use super::*;
    use prodigy::cook::execution::mapreduce::pure::dependency_analysis::{
        analyze_dependencies, extract_variable_reads, extract_variable_writes, Command,
    };

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property: Dependency analysis is structurally deterministic
        /// (same batch count and elements, order within batches may vary)
        #[test]
        fn prop_analysis_deterministic(
            cmd_count in 1usize..10,
            vars_per_cmd in 0usize..5,
        ) {
            // Create commands with some overlapping variables
            let commands: Vec<_> = (0..cmd_count)
                .map(|i| {
                    let reads: HashSet<_> = (0..vars_per_cmd)
                        .map(|j| format!("var_{}", (i + j) % 10))
                        .collect();
                    let writes: HashSet<_> = (0..vars_per_cmd)
                        .map(|j| format!("var_{}", (i + j + 1) % 10))
                        .collect();
                    Command { reads, writes }
                })
                .collect();

            let graph1 = analyze_dependencies(&commands);
            let graph2 = analyze_dependencies(&commands);

            // Compare by checking parallel batches
            let batches1 = graph1.parallel_batches();
            let batches2 = graph2.parallel_batches();

            // Same number of batches
            prop_assert_eq!(batches1.len(), batches2.len());

            // Each batch contains the same elements (order may vary)
            for (b1, b2) in batches1.iter().zip(batches2.iter()) {
                let set1: HashSet<_> = b1.iter().collect();
                let set2: HashSet<_> = b2.iter().collect();
                prop_assert_eq!(set1, set2);
            }
        }

        /// Property: Independent commands can be parallelized
        #[test]
        fn prop_independent_parallel(cmd_count in 2usize..10) {
            // Create completely independent commands (no variable overlap)
            let commands: Vec<_> = (0..cmd_count)
                .map(|i| Command {
                    reads: HashSet::new(),
                    writes: [format!("unique_{}", i)].into_iter().collect(),
                })
                .collect();

            let graph = analyze_dependencies(&commands);
            let batches = graph.parallel_batches();

            // All commands should be in a single batch
            prop_assert_eq!(batches.len(), 1);
            prop_assert_eq!(batches[0].len(), cmd_count);
        }

        /// Property: Sequential dependencies create sequential batches
        #[test]
        fn prop_sequential_deps_sequential_batches(cmd_count in 2usize..6) {
            // Create a chain: each command reads what previous wrote
            let commands: Vec<_> = (0..cmd_count)
                .map(|i| {
                    let reads = if i == 0 {
                        HashSet::new()
                    } else {
                        [format!("var_{}", i - 1)].into_iter().collect()
                    };
                    Command {
                        reads,
                        writes: [format!("var_{}", i)].into_iter().collect(),
                    }
                })
                .collect();

            let graph = analyze_dependencies(&commands);
            let batches = graph.parallel_batches();

            // Should have exactly cmd_count batches (fully sequential)
            prop_assert_eq!(batches.len(), cmd_count);
        }

        /// Property: Variable extraction handles $ prefix
        #[test]
        fn prop_extract_reads_with_dollar(var_name in "[a-zA-Z][a-zA-Z0-9_]{0,10}") {
            let cmd = format!("echo ${}", var_name);
            let reads = extract_variable_reads(&cmd);

            prop_assert!(reads.contains(&var_name) || var_name.is_empty());
        }

        /// Property: Variable extraction handles ${} syntax
        #[test]
        fn prop_extract_reads_with_braces(var_name in "[a-zA-Z][a-zA-Z0-9_]{0,10}") {
            let cmd = format!("echo ${{{}}}", var_name);
            let reads = extract_variable_reads(&cmd);

            prop_assert!(reads.contains(&var_name) || var_name.is_empty());
        }

        /// Property: Write extraction handles assignment
        #[test]
        fn prop_extract_writes_assignment(var_name in "[a-zA-Z][a-zA-Z0-9_]{0,10}") {
            let cmd = format!("{}=value", var_name);
            let writes = extract_variable_writes(&cmd);

            if !var_name.is_empty() {
                prop_assert!(writes.contains(&var_name));
            }
        }
    }
}
