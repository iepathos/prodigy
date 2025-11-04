//! Commit handling logic for workflow execution
//!
//! This module provides commit verification, auto-commit creation, and commit squashing
//! functionality. It separates commit management concerns from workflow orchestration.

use super::git_support::GitOperationsHelper;
use super::{ExtendedWorkflowConfig, WorkflowContext, WorkflowStep};
use crate::abstractions::git::GitOperations;
use crate::cook::commit_tracker::TrackedCommit;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;

/// Helper for commit-related operations in workflow execution
pub struct CommitHandler {
    git_operations: Arc<dyn GitOperations>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl CommitHandler {
    /// Create a new CommitHandler
    pub fn new(
        git_operations: Arc<dyn GitOperations>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            git_operations,
            user_interaction,
        }
    }

    /// Verify commits were created and handle auto-commit
    /// Returns (commits_created, commits) tuple
    pub async fn verify_and_handle_commits(
        &self,
        working_dir: &std::path::Path,
        head_before: &str,
        head_after: &str,
        step_display: &str,
    ) -> Result<(bool, Vec<TrackedCommit>)> {
        let git_helper = GitOperationsHelper::new(Arc::clone(&self.git_operations));

        if head_after == head_before {
            Ok((false, Vec::new()))
        } else {
            // Track commit metadata if available
            let commits = git_helper
                .get_commits_between(working_dir, head_before, head_after)
                .await?;

            let commit_count = commits.len();
            let files_changed: std::collections::HashSet<_> = commits
                .iter()
                .flat_map(|c| c.files_changed.iter())
                .collect();

            self.user_interaction.display_success(&format!(
                "{step_display} created {} commit{} affecting {} file{}",
                commit_count,
                if commit_count == 1 { "" } else { "s" },
                files_changed.len(),
                if files_changed.len() == 1 { "" } else { "s" }
            ));

            Ok((true, commits))
        }
    }

    /// Check if there are uncommitted changes
    pub async fn has_uncommitted_changes(&self, working_dir: &std::path::Path) -> Result<bool> {
        let git_helper = GitOperationsHelper::new(Arc::clone(&self.git_operations));
        git_helper.check_for_changes(working_dir).await
    }

    /// Create an auto-commit with the given message
    /// If step_display is provided (non-empty), displays a success message
    pub async fn create_auto_commit(
        &self,
        working_dir: &std::path::Path,
        message: &str,
        step_display: &str,
    ) -> Result<()> {
        // Stage all changes
        self.git_operations
            .git_command_in_dir(&["add", "."], "stage changes", working_dir)
            .await
            .context("Failed to stage changes")?;

        // Create commit
        let output = self
            .git_operations
            .git_command_in_dir(&["commit", "-m", message], "create commit", working_dir)
            .await
            .context("Failed to create commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to create commit: {stderr}"));
        }

        // Only display success message if step_display is provided
        if !step_display.is_empty() {
            self.user_interaction
                .display_success(&format!("{step_display} auto-committed changes"));
        }

        Ok(())
    }

    /// Handle commit squashing if enabled in workflow
    pub async fn handle_commit_squashing(
        &self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) {
        // Check if any step has squash enabled in commit_config
        let should_squash = workflow.steps.iter().any(|step| {
            step.commit_config
                .as_ref()
                .map(|config| config.squash)
                .unwrap_or(false)
        });

        if !should_squash {
            return;
        }

        self.user_interaction
            .display_progress("Squashing workflow commits...");

        let git_helper = GitOperationsHelper::new(Arc::clone(&self.git_operations));

        // Try to get all commits created during this workflow
        if let Ok(head_after) = git_helper.get_current_head(&env.working_dir).await {
            // Use a reasonable range for getting commits (last 20 commits should be enough for a workflow)
            if let Ok(commits) = git_helper
                .get_commits_between(&env.working_dir, "HEAD~20", &head_after)
                .await
            {
                if !commits.is_empty() {
                    // Create commit tracker and squash
                    let git_ops = Arc::new(crate::abstractions::git::RealGitOperations::new());
                    let commit_tracker = crate::cook::commit_tracker::CommitTracker::new(
                        git_ops,
                        env.working_dir.to_path_buf(),
                    );

                    // Generate squash message
                    let squash_message = format!(
                        "Squashed {} workflow commits from {}",
                        commits.len(),
                        workflow.name
                    );

                    if let Err(e) = commit_tracker
                        .squash_commits(&commits, &squash_message)
                        .await
                    {
                        tracing::warn!("Failed to squash commits: {}", e);
                    } else {
                        self.user_interaction.display_success(&format!(
                            "Squashed {} commits into one",
                            commits.len()
                        ));
                    }
                }
            }
        }
    }
}

/// Generate a commit message from template or default (pure function)
pub fn generate_commit_message(
    step: &WorkflowStep,
    context: &WorkflowContext,
    step_display_name: &str,
) -> String {
    if let Some(ref config) = step.commit_config {
        if let Some(ref template) = config.message_template {
            // Interpolate variables in template
            let mut message = template.clone();
            message = message.replace("${step.name}", step_display_name);

            // Replace other variables from context
            for (key, value) in &context.variables {
                message = message.replace(&format!("${{{key}}}"), value);
                message = message.replace(&format!("${key}"), value);
            }

            return message;
        }
    }

    format!("Auto-commit: {step_display_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_commit_message_default() {
        let step = WorkflowStep::default();
        let context = WorkflowContext::default();
        let message = generate_commit_message(&step, &context, "test-step");
        assert_eq!(message, "Auto-commit: test-step");
    }

    #[test]
    fn test_generate_commit_message_with_template() {
        let mut step = WorkflowStep::default();
        step.commit_config = Some(crate::cook::commit_tracker::CommitConfig {
            message_template: Some("feat: ${step.name} - ${description}".to_string()),
            message_pattern: None,
            sign: false,
            author: None,
            include_files: None,
            exclude_files: None,
            squash: false,
        });

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("description".to_string(), "test feature".to_string());

        let message = generate_commit_message(&step, &context, "add-feature");
        assert_eq!(message, "feat: add-feature - test feature");
    }

    #[test]
    fn test_generate_commit_message_with_braced_variables() {
        let mut step = WorkflowStep::default();
        step.commit_config = Some(crate::cook::commit_tracker::CommitConfig {
            message_template: Some("chore: ${step.name} for ${project}".to_string()),
            message_pattern: None,
            sign: false,
            author: None,
            include_files: None,
            exclude_files: None,
            squash: false,
        });

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("project".to_string(), "prodigy".to_string());

        let message = generate_commit_message(&step, &context, "update-docs");
        assert_eq!(message, "chore: update-docs for prodigy");
    }
}
