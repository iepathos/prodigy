//! Tests for cook orchestrator core functionality
//!
//! Separated from core.rs to keep the main module under 500 LOC.

use super::core::{CookConfig, DefaultCookOrchestrator};
use super::workflow_classifier::WorkflowType;
use crate::config::WorkflowConfig;
use crate::cook::command::CookCommand;
use std::path::PathBuf;
use std::sync::Arc;

fn create_test_cook_command() -> CookCommand {
    CookCommand {
        playbook: PathBuf::from("test.yaml"),
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
        params: std::collections::HashMap::new(),
    }
}

#[test]
fn test_classify_workflow_type_standard() {
    let config = CookConfig {
        command: create_test_cook_command(),
        project_path: Arc::new(PathBuf::from("/test")),
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
    };

    assert_eq!(
        DefaultCookOrchestrator::classify_workflow_type(&config),
        WorkflowType::Standard
    );
}

#[test]
fn test_classify_workflow_type_with_arguments() {
    let mut command = create_test_cook_command();
    command.args = vec!["arg1".to_string()];

    let config = CookConfig {
        command,
        project_path: Arc::new(PathBuf::from("/test")),
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
    };

    assert_eq!(
        DefaultCookOrchestrator::classify_workflow_type(&config),
        WorkflowType::WithArguments
    );
}

#[test]
fn test_classify_workflow_type_with_map_patterns() {
    let mut command = create_test_cook_command();
    command.map = vec!["*.rs".to_string()];

    let config = CookConfig {
        command,
        project_path: Arc::new(PathBuf::from("/test")),
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
    };

    assert_eq!(
        DefaultCookOrchestrator::classify_workflow_type(&config),
        WorkflowType::WithArguments
    );
}
