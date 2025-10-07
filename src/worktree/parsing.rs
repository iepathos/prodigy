//! Pure parsing functions for git worktree output
//!
//! This module contains pure functions for parsing the output of
//! `git worktree list --porcelain` into structured data.
//!
//! These functions have no dependencies on WorktreeManager state,
//! making them easy to test and reason about in isolation.

use std::path::PathBuf;

/// Parse git worktree list output into path/branch pairs
///
/// Takes the output from `git worktree list --porcelain` and parses it into
/// a vector of (path, branch) tuples. Each worktree in the output is represented
/// by a block of lines starting with "worktree" followed by metadata lines.
///
/// # Arguments
///
/// * `output` - The raw output from `git worktree list --porcelain`
///
/// # Returns
///
/// A vector of tuples containing (PathBuf, String) for each valid worktree.
/// Only worktrees with both a valid path and branch are included.
///
/// # Examples
///
/// ```
/// use prodigy::worktree::parsing::parse_worktree_output;
/// use std::path::PathBuf;
///
/// let output = r#"worktree /path/to/worktree
/// HEAD abc123
/// branch refs/heads/feature"#;
///
/// let result = parse_worktree_output(output);
/// assert_eq!(result.len(), 1);
/// assert_eq!(result[0].0, PathBuf::from("/path/to/worktree"));
/// assert_eq!(result[0].1, "feature");
/// ```
pub fn parse_worktree_output(output: &str) -> Vec<(PathBuf, String)> {
    // Split output into worktree blocks
    let blocks = split_into_worktree_blocks(output);

    // Parse each block into a path/branch pair
    blocks
        .into_iter()
        .filter_map(parse_worktree_block)
        .collect()
}

/// Split the git worktree output into individual worktree blocks
///
/// Each worktree in the output is represented by a block of lines.
/// Blocks are separated by the start of a new "worktree" line.
///
/// # Arguments
///
/// * `output` - The raw output from `git worktree list --porcelain`
///
/// # Returns
///
/// A vector of blocks, where each block is a vector of lines belonging
/// to a single worktree.
///
/// # Examples
///
/// ```
/// use prodigy::worktree::parsing::split_into_worktree_blocks;
///
/// let output = r#"worktree /path/one
/// HEAD abc123
/// branch refs/heads/feature-one
///
/// worktree /path/two
/// HEAD def456
/// branch refs/heads/feature-two"#;
///
/// let blocks = split_into_worktree_blocks(output);
/// assert_eq!(blocks.len(), 2);
/// assert_eq!(blocks[0][0], "worktree /path/one");
/// ```
pub fn split_into_worktree_blocks(output: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for line in output.lines() {
        if line.starts_with("worktree ") && !current_block.is_empty() {
            // Start of new block, save the current one
            blocks.push(current_block);
            current_block = vec![line];
        } else if !line.is_empty() {
            current_block.push(line);
        }
    }

    // Don't forget the last block
    if !current_block.is_empty() {
        blocks.push(current_block);
    }

    blocks
}

/// Parse a single worktree block into a path/branch pair
///
/// Extracts the worktree path and branch name from a block of lines.
/// Returns None if either the path or branch is missing.
///
/// # Arguments
///
/// * `block` - A vector of lines representing a single worktree's metadata
///
/// # Returns
///
/// * `Some((PathBuf, String))` - If both path and branch are found
/// * `None` - If either path or branch is missing
///
/// # Examples
///
/// ```
/// use prodigy::worktree::parsing::parse_worktree_block;
/// use std::path::PathBuf;
///
/// let block = vec![
///     "worktree /test/path",
///     "HEAD abc123",
///     "branch refs/heads/test-branch",
/// ];
///
/// let result = parse_worktree_block(block);
/// assert!(result.is_some());
///
/// let (path, branch) = result.unwrap();
/// assert_eq!(path, PathBuf::from("/test/path"));
/// assert_eq!(branch, "test-branch");
/// ```
pub fn parse_worktree_block(block: Vec<&str>) -> Option<(PathBuf, String)> {
    let path = block
        .iter()
        .find(|line| line.starts_with("worktree "))
        .map(|line| PathBuf::from(line.trim_start_matches("worktree ")))?;

    let branch = block
        .iter()
        .find(|line| line.starts_with("branch "))
        .map(|line| line.trim_start_matches("branch refs/heads/").to_string())?;

    Some((path, branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_output() {
        // Test parsing of git worktree list --porcelain output
        let output = r#"worktree /home/user/project/.prodigy/worktrees/test-session
HEAD abc123def456
branch refs/heads/test-branch

worktree /home/user/project/.prodigy/worktrees/another-session
HEAD 789012ghi345
branch refs/heads/another-branch

worktree /home/user/project
HEAD xyz789mno123
branch refs/heads/main"#;

        let entries = parse_worktree_output(output);

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].0,
            PathBuf::from("/home/user/project/.prodigy/worktrees/test-session")
        );
        assert_eq!(entries[0].1, "test-branch");
        assert_eq!(
            entries[1].0,
            PathBuf::from("/home/user/project/.prodigy/worktrees/another-session")
        );
        assert_eq!(entries[1].1, "another-branch");
        assert_eq!(entries[2].0, PathBuf::from("/home/user/project"));
        assert_eq!(entries[2].1, "main");
    }

    #[test]
    fn test_parse_worktree_output_empty() {
        // Test with empty output
        let output = "";
        let entries = parse_worktree_output(output);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_worktree_output_single_entry() {
        // Test with single worktree
        let output = r#"worktree /path/to/worktree
HEAD abc123
branch refs/heads/feature"#;

        let entries = parse_worktree_output(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, PathBuf::from("/path/to/worktree"));
        assert_eq!(entries[0].1, "feature");
    }

    #[test]
    fn test_parse_worktree_output_missing_branch() {
        // Test with missing branch info (should not include incomplete entries)
        let output = r#"worktree /path/to/worktree
HEAD abc123"#;

        let entries = parse_worktree_output(output);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_split_into_worktree_blocks() {
        // Test splitting output into individual worktree blocks
        let output = r#"worktree /path/one
HEAD abc123
branch refs/heads/feature-one

worktree /path/two
HEAD def456
branch refs/heads/feature-two"#;

        let blocks = split_into_worktree_blocks(output);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 3);
        assert_eq!(blocks[0][0], "worktree /path/one");
        assert_eq!(blocks[0][1], "HEAD abc123");
        assert_eq!(blocks[0][2], "branch refs/heads/feature-one");

        assert_eq!(blocks[1].len(), 3);
        assert_eq!(blocks[1][0], "worktree /path/two");
    }

    #[test]
    fn test_split_into_worktree_blocks_empty() {
        let output = "";
        let blocks = split_into_worktree_blocks(output);
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_split_into_worktree_blocks_single() {
        let output = r#"worktree /single/path
HEAD xyz789
branch refs/heads/main"#;

        let blocks = split_into_worktree_blocks(output);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].len(), 3);
    }

    #[test]
    fn test_parse_worktree_block_valid() {
        let block = vec![
            "worktree /test/path",
            "HEAD abc123",
            "branch refs/heads/test-branch",
        ];

        let result = parse_worktree_block(block);
        assert!(result.is_some());

        let (path, branch) = result.unwrap();
        assert_eq!(path, PathBuf::from("/test/path"));
        assert_eq!(branch, "test-branch");
    }

    #[test]
    fn test_parse_worktree_block_missing_path() {
        let block = vec!["HEAD abc123", "branch refs/heads/test-branch"];

        let result = parse_worktree_block(block);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_worktree_block_missing_branch() {
        let block = vec!["worktree /test/path", "HEAD abc123"];

        let result = parse_worktree_block(block);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_worktree_block_extra_fields() {
        // Test that extra fields don't break parsing
        let block = vec![
            "worktree /test/path",
            "HEAD abc123",
            "branch refs/heads/test-branch",
            "extra field that should be ignored",
        ];

        let result = parse_worktree_block(block);
        assert!(result.is_some());

        let (path, branch) = result.unwrap();
        assert_eq!(path, PathBuf::from("/test/path"));
        assert_eq!(branch, "test-branch");
    }
}
