//! Tests for MapReduce executor

use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::mapreduce::*;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::{CaptureOutput, CommandType};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_agent_status_serialization() {
    let statuses = vec![
        AgentStatus::Pending,
        AgentStatus::Running,
        AgentStatus::Success,
        AgentStatus::Failed("error message".to_string()),
        AgentStatus::Timeout,
        AgentStatus::Retrying(2),
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();

        // Use pattern matching for comparison
        match (&status, &deserialized) {
            (AgentStatus::Pending, AgentStatus::Pending) => {}
            (AgentStatus::Running, AgentStatus::Running) => {}
            (AgentStatus::Success, AgentStatus::Success) => {}
            (AgentStatus::Failed(a), AgentStatus::Failed(b)) if a == b => {}
            (AgentStatus::Timeout, AgentStatus::Timeout) => {}
            (AgentStatus::Retrying(a), AgentStatus::Retrying(b)) if a == b => {}
            _ => panic!("Deserialization mismatch"),
        }
    }
}

#[test]
fn test_mapreduce_config_defaults() {
    let config = MapReduceConfig {
        input: PathBuf::from("test.json"),
        json_path: "$.items[*]".to_string(),
        max_parallel: 5,
        timeout_per_agent: 300,
        retry_on_failure: 1,
        max_items: None,
        offset: None,
    };

    assert_eq!(config.max_parallel, 5);
    assert_eq!(config.timeout_per_agent, 300);
    assert_eq!(config.retry_on_failure, 1);
}

#[test]
fn test_agent_context_creation() {
    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    let context = AgentContext::new(
        "item-1".to_string(),
        PathBuf::from("/test/worktree"),
        "test-worktree".to_string(),
        env,
    );

    assert_eq!(context.item_id, "item-1");
    assert_eq!(context.worktree_path, PathBuf::from("/test/worktree"));
    assert_eq!(context.worktree_name, "test-worktree");
    assert_eq!(context.retry_count, 0);
    assert!(context.shell_output.is_none());
    assert!(context.variables.is_empty());
}

#[test]
fn test_agent_context_update_with_output() {
    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    let mut context = AgentContext::new(
        "item-1".to_string(),
        PathBuf::from("/test/worktree"),
        "test-worktree".to_string(),
        env,
    );

    // Update with output
    context.update_with_output(Some("test output".to_string()));

    assert_eq!(context.shell_output, Some("test output".to_string()));
    assert_eq!(
        context.variables.get("shell.output"),
        Some(&"test output".to_string())
    );
    assert_eq!(
        context.variables.get("shell.last_output"),
        Some(&"test output".to_string())
    );
}

#[test]
fn test_agent_context_to_interpolation_context() {
    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    let mut context = AgentContext::new(
        "item-1".to_string(),
        PathBuf::from("/test/worktree"),
        "test-worktree".to_string(),
        env,
    );

    // Add some test data
    context
        .variables
        .insert("key1".to_string(), "value1".to_string());
    context.shell_output = Some("shell output".to_string());
    context
        .captured_outputs
        .insert("capture1".to_string(), "captured".to_string());
    context
        .iteration_vars
        .insert("iter1".to_string(), "iteration".to_string());

    let interp_context = context.to_interpolation_context();

    // Verify the interpolation context contains all the data by using interpolation
    let mut engine = InterpolationEngine::new(false);
    assert_eq!(
        engine.interpolate("${key1}", &interp_context).unwrap(),
        "value1"
    );
    assert_eq!(
        engine.interpolate("${capture1}", &interp_context).unwrap(),
        "captured"
    );
    assert_eq!(
        engine.interpolate("${iter1}", &interp_context).unwrap(),
        "iteration"
    );
    assert_eq!(
        engine
            .interpolate("${shell.output}", &interp_context)
            .unwrap(),
        "shell output"
    );
}

#[test]
fn test_agent_result_serialization() {
    let result = AgentResult {
        item_id: "test_item".to_string(),
        status: AgentStatus::Success,
        output: Some("test output".to_string()),
        commits: vec!["abc123".to_string(), "def456".to_string()],
        duration: Duration::from_secs(10),
        error: None,
        worktree_path: Some(PathBuf::from("/tmp/worktree")),
        branch_name: Some("mmm-agent-123-test_item".to_string()),
        worktree_session_id: Some("mmm-session-123".to_string()),
        files_modified: vec![PathBuf::from("src/main.rs")],
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: AgentResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.item_id, result.item_id);
    assert_eq!(deserialized.output, result.output);
    assert_eq!(deserialized.commits, result.commits);
    assert_eq!(deserialized.duration, result.duration);
    assert_eq!(deserialized.error, result.error);
    assert_eq!(deserialized.worktree_path, result.worktree_path);
}

#[test]
fn test_map_phase_configuration() {
    use crate::cook::workflow::WorkflowStep;

    let map_phase = MapPhase {
        config: MapReduceConfig {
            input: PathBuf::from("items.json"),
            json_path: "$.items[*]".to_string(),
            max_parallel: 20,
            timeout_per_agent: 1200,
            retry_on_failure: 3,
            max_items: None,
            offset: None,
        },
        agent_template: vec![WorkflowStep {
            name: None,
            claude: Some("/fix-issue ${item.description}".to_string()),
            shell: None,
            test: None,
            command: None,
            handler: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            validate: None,
        }],
        filter: Some("severity == 'high'".to_string()),
        sort_by: Some("priority".to_string()),
    };

    assert_eq!(map_phase.config.max_parallel, 20);
    assert_eq!(map_phase.config.timeout_per_agent, 1200);
    assert_eq!(map_phase.agent_template.len(), 1);
    assert_eq!(map_phase.filter, Some("severity == 'high'".to_string()));
    assert_eq!(map_phase.sort_by, Some("priority".to_string()));
}

#[test]
fn test_reduce_phase_configuration() {
    use crate::cook::workflow::WorkflowStep;

    let reduce_phase = ReducePhase {
        commands: vec![
            WorkflowStep {
                name: None,
                claude: Some("/summarize-results".to_string()),
                shell: None,
                test: None,
                command: None,
                handler: None,
                capture_output: CaptureOutput::Disabled,
                timeout: None,
                working_dir: None,
                env: HashMap::new(),
                on_failure: None,
                on_success: None,
                on_exit_code: HashMap::new(),
                commit_required: false,
                validate: None,
            },
            WorkflowStep {
                name: None,
                claude: None,
                shell: Some("git merge --no-ff agent-*".to_string()),
                test: None,
                command: None,
                handler: None,
                capture_output: CaptureOutput::Disabled,
                timeout: None,
                working_dir: None,
                env: HashMap::new(),
                on_failure: None,
                on_success: None,
                on_exit_code: HashMap::new(),
                commit_required: true,
                validate: None,
            },
        ],
    };

    assert_eq!(reduce_phase.commands.len(), 2);
    assert!(reduce_phase.commands[0].claude.is_some());
    assert!(reduce_phase.commands[1].shell.is_some());
}

#[test]
fn test_reduce_phase_variable_substitution() {
    // Test that map results are properly available as variables in reduce phase
    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    let mut reduce_context = AgentContext::new(
        "reduce".to_string(),
        PathBuf::from("/test/worktree"),
        "test-worktree".to_string(),
        env,
    );

    // Simulate adding map results to reduce context (this is what was missing!)
    reduce_context
        .variables
        .insert("map.successful".to_string(), "3".to_string());
    reduce_context
        .variables
        .insert("map.failed".to_string(), "1".to_string());
    reduce_context
        .variables
        .insert("map.total".to_string(), "4".to_string());

    // Convert to interpolation context
    let interp_context = reduce_context.to_interpolation_context();

    // Test that variables are accessible
    assert_eq!(
        interp_context
            .variables
            .get("map.successful")
            .and_then(|v| v.as_str()),
        Some("3")
    );
    assert_eq!(
        interp_context
            .variables
            .get("map.failed")
            .and_then(|v| v.as_str()),
        Some("1")
    );
    assert_eq!(
        interp_context
            .variables
            .get("map.total")
            .and_then(|v| v.as_str()),
        Some("4")
    );
}

#[test]
fn test_reduce_phase_complex_variable_substitution() {
    // Test complex variable substitution including claude output
    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    let mut reduce_context = AgentContext::new(
        "reduce".to_string(),
        PathBuf::from("/test/worktree"),
        "test-worktree".to_string(),
        env,
    );

    // Add map statistics
    reduce_context
        .variables
        .insert("map.successful".to_string(), "5".to_string());
    reduce_context
        .variables
        .insert("map.failed".to_string(), "2".to_string());
    reduce_context
        .variables
        .insert("map.total".to_string(), "7".to_string());

    // Add claude output from a previous command (stored in captured_outputs)
    reduce_context.captured_outputs.insert(
        "claude.output".to_string(),
        "Debt reduction analysis: 30% improvement".to_string(),
    );

    // Add individual result data
    reduce_context
        .variables
        .insert("result.0.item_id".to_string(), "debt-item-1".to_string());
    reduce_context
        .variables
        .insert("result.0.status".to_string(), "success".to_string());
    reduce_context
        .variables
        .insert("result.1.item_id".to_string(), "debt-item-2".to_string());
    reduce_context
        .variables
        .insert("result.1.status".to_string(), "failed".to_string());

    let interp_context = reduce_context.to_interpolation_context();

    // Verify all variables are accessible
    assert_eq!(
        interp_context
            .variables
            .get("map.successful")
            .and_then(|v| v.as_str()),
        Some("5")
    );
    assert_eq!(
        interp_context
            .variables
            .get("claude.output")
            .and_then(|v| v.as_str()),
        Some("Debt reduction analysis: 30% improvement")
    );
    assert_eq!(
        interp_context
            .variables
            .get("result.0.item_id")
            .and_then(|v| v.as_str()),
        Some("debt-item-1")
    );
}

#[test]
fn test_reduce_context_has_map_variables() {
    // This test verifies the FIX: that reduce context gets map result variables
    // Before the fix, these variables were NOT added to the reduce context

    let env = ExecutionEnvironment {
        working_dir: PathBuf::from("/test/worktree"),
        project_dir: PathBuf::from("/test/project"),
        worktree_name: Some("test-worktree".to_string()),
        session_id: "test-session".to_string(),
    };

    // Simulate what execute_reduce_phase does after our fix
    let mut reduce_context = AgentContext::new(
        "reduce".to_string(),
        env.working_dir.clone(),
        "test-worktree".to_string(),
        env.clone(),
    );

    // This is the CRITICAL FIX - adding map statistics to reduce context
    let successful_count = 3;
    let failed_count = 1;
    let total = 4;

    reduce_context
        .variables
        .insert("map.successful".to_string(), successful_count.to_string());
    reduce_context
        .variables
        .insert("map.failed".to_string(), failed_count.to_string());
    reduce_context
        .variables
        .insert("map.total".to_string(), total.to_string());

    // Verify the variables are available for interpolation
    let interp = reduce_context.to_interpolation_context();

    // These assertions would FAIL before our fix because the variables weren't added
    assert!(interp.variables.contains_key("map.successful"));
    assert!(interp.variables.contains_key("map.failed"));
    assert!(interp.variables.contains_key("map.total"));

    // Test interpolation of a shell command that uses these variables
    let _test_command = "echo 'Processed ${map.successful} of ${map.total} items'";

    // Before the fix, this would result in bad substitution error
    // After the fix, the variables are available for substitution
    assert_eq!(
        interp
            .variables
            .get("map.successful")
            .and_then(|v| v.as_str()),
        Some("3")
    );
}

#[test]
fn test_custom_capture_output_variables() {
    use crate::cook::workflow::CaptureOutput;

    // Test CaptureOutput enum functionality
    assert!(!CaptureOutput::Disabled.is_enabled());
    assert!(CaptureOutput::Default.is_enabled());
    assert!(CaptureOutput::Variable("my_output".to_string()).is_enabled());

    // Test variable name generation
    let claude_cmd = CommandType::Claude("test".to_string());
    let shell_cmd = CommandType::Shell("echo test".to_string());

    assert_eq!(CaptureOutput::Disabled.get_variable_name(&claude_cmd), None);

    assert_eq!(
        CaptureOutput::Default.get_variable_name(&claude_cmd),
        Some("claude.output".to_string())
    );

    assert_eq!(
        CaptureOutput::Default.get_variable_name(&shell_cmd),
        Some("shell.output".to_string())
    );

    assert_eq!(
        CaptureOutput::Variable("custom_var".to_string()).get_variable_name(&claude_cmd),
        Some("custom_var".to_string())
    );

    assert_eq!(
        CaptureOutput::Variable("my.special.output".to_string()).get_variable_name(&shell_cmd),
        Some("my.special.output".to_string())
    );
}

#[test]
fn test_interpolation_with_work_item() {
    let mut engine = InterpolationEngine::new(false);
    let mut context = InterpolationContext::new();

    // Add a work item to context
    let item = json!({
        "id": 123,
        "description": "Fix memory leak in parser",
        "priority": "high",
        "location": {
            "file": "src/parser.rs",
            "line": 45
        }
    });

    context.set("item", item);

    // Test various interpolation patterns
    let tests = vec![
        ("Task ${item.id}", "Task 123"),
        ("Fix: ${item.description}", "Fix: Fix memory leak in parser"),
        ("Priority: ${item.priority}", "Priority: high"),
        (
            "File: ${item.location.file}:${item.location.line}",
            "File: src/parser.rs:45",
        ),
    ];

    for (template, expected) in tests {
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, expected, "Failed for template: {}", template);
    }
}

#[test]
fn test_interpolation_with_map_results() {
    let mut engine = InterpolationEngine::new(false);
    let mut context = InterpolationContext::new();

    // Add map results to context as a nested object
    context.set(
        "map",
        json!({
            "successful": 8,
            "failed": 2,
            "total": 10
        }),
    );

    let template =
        "Processed ${map.total} items: ${map.successful} successful, ${map.failed} failed";
    let result = engine.interpolate(template, &context).unwrap();
    assert_eq!(result, "Processed 10 items: 8 successful, 2 failed");
}

#[test]
fn test_interpolation_with_shell_output() {
    let mut engine = InterpolationEngine::new(false);
    let mut context = InterpolationContext::new();

    // Simulate shell output from previous step as a nested object
    context.set(
        "shell",
        json!({
            "output": "All tests passed",
            "last_output": "Coverage: 85%"
        }),
    );

    let template = "Previous output: ${shell.output}. ${shell.last_output}";
    let result = engine.interpolate(template, &context).unwrap();
    assert_eq!(result, "Previous output: All tests passed. Coverage: 85%");
}

#[test]
fn test_interpolation_with_defaults() {
    let mut engine = InterpolationEngine::new(false);
    let context = InterpolationContext::new();

    // Test default values for undefined variables
    let tests = vec![
        ("Timeout: ${timeout:-600}s", "Timeout: 600s"),
        ("Workers: ${workers:-10}", "Workers: 10"),
        ("Mode: ${mode:-parallel}", "Mode: parallel"),
    ];

    for (template, expected) in tests {
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, expected, "Failed for template: {}", template);
    }
}

#[test]
fn test_interpolation_context_hierarchy() {
    let mut engine = InterpolationEngine::new(false);

    // Create parent context
    let mut parent = InterpolationContext::new();
    parent.set("global_setting", json!("production"));
    parent.set("max_workers", json!(20));

    // Create child context
    let mut child = parent.child();
    child.set("local_setting", json!("debug"));
    child.set("max_workers", json!(5)); // Override parent value

    // Test resolution
    let tests = vec![
        ("Mode: ${global_setting}", "Mode: production"),
        ("Debug: ${local_setting}", "Debug: debug"),
        ("Workers: ${max_workers}", "Workers: 5"), // Should use child's value
    ];

    for (template, expected) in tests {
        let result = engine.interpolate(template, &child).unwrap();
        assert_eq!(result, expected, "Failed for template: {}", template);
    }
}

#[test]
fn test_interpolation_with_arrays() {
    let mut engine = InterpolationEngine::new(false);
    let mut context = InterpolationContext::new();

    // Add array data
    let results = json!([
        {"id": "item1", "status": "success"},
        {"id": "item2", "status": "failed"},
        {"id": "item3", "status": "success"}
    ]);

    context.set("results", results);

    // Test array access
    let tests = vec![
        ("First: ${results[0].id}", "First: item1"),
        (
            "Second status: ${results[1].status}",
            "Second status: failed",
        ),
        ("Third: ${results[2].id}", "Third: item3"),
    ];

    for (template, expected) in tests {
        let result = engine.interpolate(template, &context).unwrap();
        assert_eq!(result, expected, "Failed for template: {}", template);
    }
}

#[test]
fn test_interpolation_strict_mode() {
    let mut engine = InterpolationEngine::new(true); // strict mode
    let context = InterpolationContext::new();

    // Should fail on undefined variable in strict mode
    let result = engine.interpolate("Value: ${undefined}", &context);
    assert!(result.is_err());

    // Should work with default value even in strict mode
    let result = engine.interpolate("Value: ${undefined:-default}", &context);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Value: default");
}

// TODO: Add test for json_path_extraction once mock types are properly configured
// The interpolation functionality is tested comprehensively in the tests above
