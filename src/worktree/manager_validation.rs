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
/// Provides specific error messages for different failure scenarios including
/// permission denial and wrong-direction merges.
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
/// - Merge was executed in the wrong direction
/// - Merge was aborted or failed silently
pub fn validate_merge_success(
    worktree_branch: &str,
    target_branch: &str,
    merged_branches: &str,
    merge_output: &str,
) -> Result<()> {
    if !merged_branches.contains(worktree_branch) {
        // Check for specific failure patterns to provide better error messages
        if is_permission_denied(merge_output) {
            anyhow::bail!(
                "Merge was not completed - Claude requires permission to proceed. \
                Please run the command again and grant permission when prompted."
            );
        }

        // Check if this looks like a wrong-direction merge
        if is_wrong_direction_merge(merge_output, target_branch) {
            anyhow::bail!(
                "Merge direction error - Claude merged '{}' into the worktree instead of \
                merging the worktree into '{}'. The output indicates 'Already up to date' \
                which suggests the target branch was already an ancestor of the worktree branch.\n\n\
                To fix this manually:\n\
                  1. git checkout {}\n\
                  2. git merge --no-ff {}\n\n\
                This is a known issue with the merge skill argument parsing.",
                target_branch,
                target_branch,
                target_branch,
                worktree_branch
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

/// Check if merge output indicates a wrong-direction merge
///
/// Detects when Claude merged the target branch into the worktree instead
/// of the worktree into the target. This typically produces "Already up to date"
/// when the target is an ancestor of the worktree.
///
/// # Arguments
///
/// * `merge_output` - Output from the merge command
/// * `target_branch` - Name of the intended target branch
///
/// # Returns
///
/// * `true` if the output indicates a wrong-direction merge
/// * `false` otherwise
///
/// # Examples
///
/// ```
/// use prodigy::worktree::manager_validation::is_wrong_direction_merge;
///
/// // Typical wrong-direction merge output
/// assert!(is_wrong_direction_merge("Already up to date.", "main"));
/// assert!(is_wrong_direction_merge("Already up-to-date.", "main"));
///
/// // Target branch mentioned as being merged (wrong direction)
/// assert!(is_wrong_direction_merge(
///     "Source: main, Target: worktree-branch\nAlready up to date",
///     "main"
/// ));
///
/// // Successful merge should not trigger
/// assert!(!is_wrong_direction_merge("Merge made by the 'ort' strategy.", "main"));
/// ```
pub fn is_wrong_direction_merge(merge_output: &str, target_branch: &str) -> bool {
    let output_lower = merge_output.to_lowercase();

    // Check for "Already up to date" which indicates no merge was needed
    // This happens when the source is already an ancestor of the current branch
    let already_up_to_date =
        output_lower.contains("already up to date") || output_lower.contains("already up-to-date");

    if !already_up_to_date {
        return false;
    }

    // Additional heuristics to confirm wrong direction:
    // 1. The output mentions the target branch as the "source" being merged
    // 2. The merge result mentions the target branch being merged into something else

    // Check if the output explicitly mentions the target as source
    let target_as_source_pattern = format!("source.*{}", target_branch.to_lowercase());
    let mentions_target_as_source = output_lower.contains(&target_as_source_pattern)
        || output_lower.contains(&format!(
            "branch `{}` is already",
            target_branch.to_lowercase()
        ));

    // If we see "already up to date" and no indication of the target being a source,
    // we still report it as wrong direction since this is the most common case
    // when prodigy calls merge with (worktree_branch, target_branch) but Claude
    // interprets it incorrectly
    already_up_to_date && (mentions_target_as_source || !output_lower.contains("merge made"))
}

/// Check if command output indicates permission was denied
///
/// Analyzes command output to detect if the operation failed due to
/// permission denial, which requires user intervention.
///
/// Uses specific phrase patterns to avoid false positives from JSONL
/// metadata fields like "permissionMode" or "permission_denials".
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
/// assert!(is_permission_denied("requires permission to continue"));
/// assert!(!is_permission_denied("Merge completed successfully"));
/// // Should NOT match JSONL metadata fields
/// assert!(!is_permission_denied(r#""permissionMode":"bypassPermissions""#));
/// assert!(!is_permission_denied(r#""permission_denials":[]"#));
/// ```
pub fn is_permission_denied(output: &str) -> bool {
    // Use specific phrases to avoid false positives from JSONL metadata
    // like "permissionMode" or "permission_denials"
    let denial_patterns = [
        "permission denied",
        "permission to proceed",
        "requires permission",
        "grant permission",
        "waiting for permission",
        "need permission",
        "permission required",
    ];

    let output_lower = output.to_lowercase();
    denial_patterns
        .iter()
        .any(|pattern| output_lower.contains(pattern))
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
