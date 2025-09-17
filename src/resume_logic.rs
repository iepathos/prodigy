//! Pure functions for resume workflow logic
//! Following functional programming principles with no side effects

use crate::cook::workflow::checkpoint::{WorkflowCheckpoint, WorkflowStatus};
use std::path::{Path, PathBuf};

/// Result of finding workflow file
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowFileResult {
    Found(PathBuf),
    NotFound(Vec<PathBuf>),
    Multiple(Vec<PathBuf>),
}

/// Pure function to determine possible workflow file paths
pub fn possible_workflow_paths(working_dir: &Path, workflow_name: Option<&str>) -> Vec<PathBuf> {
    if let Some(name) = workflow_name {
        vec![
            working_dir.join(format!("{}.yml", name)),
            working_dir.join(format!("{}.yaml", name)),
            working_dir.join("workflow.yml"),
            working_dir.join("workflow.yaml"),
            working_dir.join("playbook.yml"),
            working_dir.join("playbook.yaml"),
            working_dir.join("test_complete_resume.yml"),
            working_dir.join("test_checkpoint.yml"),
        ]
    } else {
        vec![
            working_dir.join("workflow.yml"),
            working_dir.join("workflow.yaml"),
            working_dir.join("playbook.yml"),
            working_dir.join("playbook.yaml"),
        ]
    }
}

/// Pure function to find workflow file
pub fn find_workflow_file(
    paths: &[PathBuf],
    file_exists: impl Fn(&Path) -> bool,
) -> WorkflowFileResult {
    let existing: Vec<PathBuf> = paths.iter().filter(|p| file_exists(p)).cloned().collect();

    match existing.len() {
        0 => WorkflowFileResult::NotFound(paths.to_vec()),
        1 => WorkflowFileResult::Found(existing.into_iter().next().unwrap()),
        _ => WorkflowFileResult::Multiple(existing),
    }
}

/// Generate checkpoint status message
pub fn format_checkpoint_status(checkpoint: &WorkflowCheckpoint) -> Vec<String> {
    vec![
        format!(
            "✅ Found checkpoint for workflow: {}",
            checkpoint.workflow_id
        ),
        format!(
            "   Step progress: {}/{}",
            checkpoint.execution_state.current_step_index, checkpoint.execution_state.total_steps
        ),
        format!("   Status: {:?}", checkpoint.execution_state.status),
    ]
}

/// Generate resume action message based on force flag and status
pub fn format_resume_action(force: bool, status: &WorkflowStatus) -> String {
    match (force, status) {
        (true, _) => "📂 Force restarting workflow from beginning".to_string(),
        (false, WorkflowStatus::Interrupted) => {
            "📂 Resuming workflow from checkpoint...".to_string()
        }
        (false, WorkflowStatus::Failed) => "📂 Resuming workflow after failure...".to_string(),
        (false, _) => "📂 Resuming workflow...".to_string(),
    }
}

/// Determine if resume should proceed
pub fn should_resume(status: &WorkflowStatus, force: bool) -> (bool, Option<String>) {
    match status {
        WorkflowStatus::Completed if !force => (
            false,
            Some("Workflow already completed - nothing to resume".to_string()),
        ),
        WorkflowStatus::Running if !force => {
            (false, Some("Workflow is already running".to_string()))
        }
        _ => (true, None),
    }
}

/// Calculate steps to skip
pub fn calculate_skip_count(checkpoint: &WorkflowCheckpoint, force: bool) -> usize {
    if force {
        0
    } else {
        checkpoint.execution_state.current_step_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::checkpoint::ExecutionState;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_possible_workflow_paths_with_name() {
        let dir = Path::new("/test");
        let paths = possible_workflow_paths(dir, Some("my-workflow"));

        assert_eq!(paths.len(), 8);
        assert!(paths.contains(&PathBuf::from("/test/my-workflow.yml")));
        assert!(paths.contains(&PathBuf::from("/test/my-workflow.yaml")));
        assert!(paths.contains(&PathBuf::from("/test/workflow.yml")));
    }

    #[test]
    fn test_possible_workflow_paths_without_name() {
        let dir = Path::new("/test");
        let paths = possible_workflow_paths(dir, None);

        assert_eq!(paths.len(), 4);
        assert!(paths.contains(&PathBuf::from("/test/workflow.yml")));
        assert!(!paths.contains(&PathBuf::from("/test/my-workflow.yml")));
    }

    #[test]
    fn test_find_workflow_file_single() {
        let paths = vec![
            PathBuf::from("/test/workflow.yml"),
            PathBuf::from("/test/playbook.yml"),
        ];

        let result = find_workflow_file(&paths, |p| p == Path::new("/test/workflow.yml"));

        assert_eq!(
            result,
            WorkflowFileResult::Found(PathBuf::from("/test/workflow.yml"))
        );
    }

    #[test]
    fn test_find_workflow_file_none() {
        let paths = vec![
            PathBuf::from("/test/workflow.yml"),
            PathBuf::from("/test/playbook.yml"),
        ];

        let result = find_workflow_file(&paths, |_| false);

        match result {
            WorkflowFileResult::NotFound(p) => assert_eq!(p.len(), 2),
            _ => panic!("Expected NotFound"),
        }
    }

    #[test]
    fn test_find_workflow_file_multiple() {
        let paths = vec![
            PathBuf::from("/test/workflow.yml"),
            PathBuf::from("/test/playbook.yml"),
        ];

        let result = find_workflow_file(&paths, |_| true);

        match result {
            WorkflowFileResult::Multiple(p) => assert_eq!(p.len(), 2),
            _ => panic!("Expected Multiple"),
        }
    }

    #[test]
    fn test_format_checkpoint_status() {
        let checkpoint = WorkflowCheckpoint {
            workflow_id: "test-123".to_string(),
            execution_state: ExecutionState {
                current_step_index: 3,
                total_steps: 10,
                status: WorkflowStatus::Interrupted,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: vec![],
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: Utc::now(),
            version: 1,
            workflow_hash: "hash".to_string(),
            total_steps: 10,
            workflow_name: None,
            workflow_path: None,
        };

        let messages = format_checkpoint_status(&checkpoint);

        assert_eq!(messages.len(), 3);
        assert!(messages[0].contains("test-123"));
        assert!(messages[1].contains("3/10"));
        assert!(messages[2].contains("Interrupted"));
    }

    #[test]
    fn test_format_resume_action_force() {
        let msg = format_resume_action(true, &WorkflowStatus::Completed);
        assert!(msg.contains("Force restarting"));
    }

    #[test]
    fn test_format_resume_action_interrupted() {
        let msg = format_resume_action(false, &WorkflowStatus::Interrupted);
        assert!(msg.contains("Resuming workflow from checkpoint"));
    }

    #[test]
    fn test_should_resume_completed_without_force() {
        let (should, msg) = should_resume(&WorkflowStatus::Completed, false);
        assert!(!should);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("already completed"));
    }

    #[test]
    fn test_should_resume_completed_with_force() {
        let (should, msg) = should_resume(&WorkflowStatus::Completed, true);
        assert!(should);
        assert!(msg.is_none());
    }

    #[test]
    fn test_calculate_skip_count_normal() {
        let checkpoint = WorkflowCheckpoint {
            workflow_id: "test".to_string(),
            execution_state: ExecutionState {
                current_step_index: 5,
                total_steps: 10,
                status: WorkflowStatus::Interrupted,
                start_time: Utc::now(),
                last_checkpoint: Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: vec![],
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: Utc::now(),
            version: 1,
            workflow_hash: "hash".to_string(),
            total_steps: 10,
            workflow_name: None,
            workflow_path: None,
        };

        assert_eq!(calculate_skip_count(&checkpoint, false), 5);
        assert_eq!(calculate_skip_count(&checkpoint, true), 0);
    }
}
