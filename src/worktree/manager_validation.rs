//! Validation functions for worktree merge operations
//!
//! This module contains pure validation functions that check preconditions,
//! verify merge success, and validate command results for worktree operations.
//! All functions in this module are stateless and focus on validation logic
//! without performing I/O operations.
//!
//! # Responsibilities
//!
//! - Merge precondition validation
//! - Claude command result verification
//! - Merge completion checks
//! - Permission denial detection
//! - Branch merge status validation
//!
//! # Design Principles
//!
//! Functions in this module follow functional programming principles:
//! - Pure functions that take inputs and return outputs
//! - No side effects or I/O operations
//! - Stateless validation logic
//! - Clear error messages with context

use anyhow::Result;

/// Determine if merge should proceed based on commit count between branches
///
/// A merge should proceed when there are commits in the worktree branch that
/// are not in the target branch (commit count != "0").
///
/// # Arguments
///
/// * `commit_count` - String representation of the number of commits ahead
///
/// # Returns
///
/// * `true` if merge should proceed (has commits to merge)
/// * `false` if no commits to merge
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_validation::should_proceed_with_merge;
///
/// assert!(should_proceed_with_merge("5"));
/// assert!(!should_proceed_with_merge("0"));
/// ```
pub fn should_proceed_with_merge(commit_count: &str) -> bool {
    commit_count != "0"
}

/// Validate Claude command execution result
///
/// Checks if the Claude command execution was successful and provides
/// detailed error information if it failed.
///
/// # Arguments
///
/// * `result` - The execution result from Claude command
///
/// # Returns
///
/// * `Ok(())` if execution was successful
/// * `Err` with details if execution failed
///
/// # Errors
///
/// Returns error if:
/// - Command execution failed (success = false)
/// - Error output indicates failure
pub fn validate_claude_result(result: &crate::cook::execution::ExecutionResult) -> Result<()> {
    if !result.success {
        eprintln!("❌ Claude merge failed:");
        if !result.stderr.is_empty() {
            eprintln!("Error output: {}", result.stderr);
        }
        if !result.stdout.is_empty() {
            eprintln!("Standard output: {}", result.stdout);
        }
        anyhow::bail!("Claude merge failed");
    }
    Ok(())
}

/// Validate that merge was successful by checking branch merge status
///
/// Verifies that the worktree branch has been successfully merged into the
/// target branch by checking if it appears in the list of merged branches.
/// Provides specific error messages for permission denial cases.
///
/// # Arguments
///
/// * `worktree_branch` - Name of the worktree branch that should be merged
/// * `target_branch` - Name of the target branch (e.g., main or master)
/// * `merged_branches` - Output from git showing merged branches
/// * `merge_output` - Output from the merge command (for error diagnosis)
///
/// # Returns
///
/// * `Ok(())` if branch is successfully merged
/// * `Err` with detailed explanation if merge failed
///
/// # Errors
///
/// Returns error if:
/// - Branch is not found in merged branches list
/// - Permission was denied during merge
/// - Merge was aborted or failed silently
pub fn validate_merge_success(
    worktree_branch: &str,
    target_branch: &str,
    merged_branches: &str,
    merge_output: &str,
) -> Result<()> {
    if !merged_branches.contains(worktree_branch) {
        if is_permission_denied(merge_output) {
            anyhow::bail!(
                "Merge was not completed - Claude requires permission to proceed. \
                Please run the command again and grant permission when prompted."
            );
        }
        anyhow::bail!(
            "Merge verification failed - branch '{}' is not merged into '{}'. \
            The merge may have been aborted or failed silently.",
            worktree_branch,
            target_branch
        );
    }
    Ok(())
}

/// Check if command output indicates permission was denied
///
/// Analyzes command output to detect if the operation failed due to
/// permission denial, which requires user intervention.
///
/// # Arguments
///
/// * `output` - Command output text to analyze
///
/// # Returns
///
/// * `true` if output contains permission denial indicators
/// * `false` otherwise
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_validation::is_permission_denied;
///
/// assert!(is_permission_denied("Error: permission denied"));
/// assert!(is_permission_denied("Please grant permission to proceed"));
/// assert!(!is_permission_denied("Merge completed successfully"));
/// ```
pub fn is_permission_denied(output: &str) -> bool {
    output.contains("permission") || output.contains("grant permission")
}

/// Check if a branch has been merged into a target branch
///
/// Determines if the specified branch appears in the list of branches
/// that have been merged into the target.
///
/// # Arguments
///
/// * `branch` - Branch name to check
/// * `merged_branches_output` - Output from `git branch --merged target`
///
/// # Returns
///
/// * `true` if branch is in the merged list
/// * `false` otherwise
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_validation::check_if_branch_merged;
///
/// let output = "  feature-123\n  bugfix-456\n* main\n";
/// assert!(check_if_branch_merged("feature-123", output));
/// assert!(!check_if_branch_merged("feature-999", output));
/// ```
pub fn check_if_branch_merged(branch: &str, merged_branches_output: &str) -> bool {
    merged_branches_output.contains(branch)
}
