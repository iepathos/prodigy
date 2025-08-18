//! Tests for MapReduce executor

use super::*;
use std::collections::HashMap;

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
    };

    assert_eq!(config.max_parallel, 5);
    assert_eq!(config.timeout_per_agent, 300);
    assert_eq!(config.retry_on_failure, 1);
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
        },
        agent_template: vec![WorkflowStep {
            name: None,
            claude: Some("/fix-issue ${item.description}".to_string()),
            shell: None,
            test: None,
            command: None,
            handler: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
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
                capture_output: false,
                timeout: None,
                working_dir: None,
                env: HashMap::new(),
                on_failure: None,
                on_success: None,
                on_exit_code: HashMap::new(),
                commit_required: false,
            },
            WorkflowStep {
                name: None,
                claude: None,
                shell: Some("git merge --no-ff agent-*".to_string()),
                test: None,
                command: None,
                handler: None,
                capture_output: false,
                timeout: None,
                working_dir: None,
                env: HashMap::new(),
                on_failure: None,
                on_success: None,
                on_exit_code: HashMap::new(),
                commit_required: true,
            },
        ],
    };

    assert_eq!(reduce_phase.commands.len(), 2);
    assert!(reduce_phase.commands[0].claude.is_some());
    assert!(reduce_phase.commands[1].shell.is_some());
}