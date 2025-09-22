//! Git output parsers

use super::types::*;
use crate::LibResult;
use std::path::{Path, PathBuf};

/// Parse git status --porcelain=v2 output
pub fn parse_status_output(output: &str) -> LibResult<GitStatus> {
    let mut status = GitStatus::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        parse_status_line_entry(&mut status, line)?;
    }

    Ok(status)
}

fn parse_status_line_entry(status: &mut GitStatus, line: &str) -> LibResult<()> {
    match line.chars().next() {
        Some('#') => parse_header_line(status, line),
        Some('1') => parse_status_line(status, line),
        Some('2') => parse_renamed_line(status, line),
        Some('u') => parse_unmerged_line(status, line),
        Some('?') => parse_untracked_line(status, line),
        Some('!') => Ok(()), // Ignored files - we don't track these
        _ => Ok(()),
    }
}

fn parse_header_line(status: &mut GitStatus, line: &str) -> LibResult<()> {
    if let Some(branch_name) = line.strip_prefix("# branch.head ") {
        if branch_name != "(detached)" {
            status.branch = Some(branch_name.to_string());
        }
    } else if line.starts_with("# merge.in-progress ") {
        status.in_merge = line.contains("true");
    }
    // Ignore other header lines (upstream, ahead/behind info)
    Ok(())
}

fn parse_untracked_line(status: &mut GitStatus, line: &str) -> LibResult<()> {
    if let Some(path) = line.strip_prefix("? ") {
        let path = path.trim();
        if !path.is_empty() {
            status.untracked.push(PathBuf::from(path));
        }
    }
    Ok(())
}

fn parse_status_line(status: &mut GitStatus, line: &str) -> LibResult<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        return Ok(());
    }

    let xy = parts[1];
    let path = parts[8..].join(" ");
    let path_buf = PathBuf::from(path);

    // Parse status codes (X = staged, Y = unstaged)
    let x = xy.chars().next().unwrap_or('.');
    let y = xy.chars().nth(1).unwrap_or('.');

    // Handle staged changes (X = first character)
    match x {
        'A' => status.added.push(path_buf.clone()),
        'M' => {
            // Staged modified files should go in modified list
            if !status.modified.contains(&path_buf) {
                status.modified.push(path_buf.clone());
            }
        }
        'D' => status.deleted.push(path_buf.clone()),
        _ => {}
    }

    // Handle unstaged changes (Y = second character)
    match y {
        'M' => {
            if !status.modified.contains(&path_buf) {
                status.modified.push(path_buf);
            }
        }
        'D' => {
            if !status.deleted.contains(&path_buf) {
                status.deleted.push(path_buf);
            }
        }
        _ => {}
    }

    Ok(())
}

fn parse_renamed_line(status: &mut GitStatus, line: &str) -> LibResult<()> {
    let parts: Vec<&str> = line.splitn(10, ' ').collect();
    if parts.len() < 10 {
        return Ok(());
    }

    let path_part = parts[9];
    if let Some(sep_pos) = path_part.find('\t') {
        let new_path = &path_part[..sep_pos];
        let old_path = &path_part[sep_pos + 1..];
        status
            .renamed
            .push((PathBuf::from(old_path), PathBuf::from(new_path)));
    }

    Ok(())
}

fn parse_unmerged_line(status: &mut GitStatus, line: &str) -> LibResult<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 11 {
        return Ok(());
    }

    let path = parts[10..].join(" ");
    status.conflicts.push(PathBuf::from(path));

    Ok(())
}

/// Parse git diff --numstat output
pub fn parse_diff_output(output: &str) -> LibResult<GitDiff> {
    let mut diff = GitDiff::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 3 {
            continue;
        }

        let insertions = parts[0].parse::<usize>().unwrap_or(0);
        let deletions = parts[1].parse::<usize>().unwrap_or(0);
        let path = PathBuf::from(parts[2]);

        diff.insertions += insertions;
        diff.deletions += deletions;

        let change_type = if insertions > 0 && deletions == 0 {
            FileChangeType::Added
        } else if insertions == 0 && deletions > 0 {
            FileChangeType::Deleted
        } else {
            FileChangeType::Modified
        };

        diff.files_changed.push(FileDiff {
            path,
            insertions,
            deletions,
            change_type,
        });
    }

    Ok(diff)
}

/// Parse git worktree list --porcelain output
pub fn parse_worktree_list(output: &str) -> LibResult<Vec<WorktreeInfo>> {
    // Split output into blocks separated by empty lines
    let blocks = split_into_worktree_blocks(output);

    // Parse each block into a WorktreeInfo using functional transformation
    let worktrees: Vec<WorktreeInfo> = blocks
        .into_iter()
        .filter_map(|block| parse_single_worktree_block(block))
        .collect();

    Ok(worktrees)
}

/// Split the output into individual worktree blocks
fn split_into_worktree_blocks(output: &str) -> Vec<Vec<&str>> {
    let (mut blocks, current) = output.lines().fold(
        (Vec::new(), Vec::new()),
        |(mut blocks, mut current), line| {
            if line.is_empty() {
                if !current.is_empty() {
                    blocks.push(current);
                    (blocks, Vec::new())
                } else {
                    (blocks, current)
                }
            } else {
                current.push(line);
                (blocks, current)
            }
        },
    );

    // Handle the last block if it exists
    if !current.is_empty() {
        blocks.push(current);
    }

    blocks
}

/// Parse a single worktree block into WorktreeInfo
fn parse_single_worktree_block(lines: Vec<&str>) -> Option<WorktreeInfo> {
    // Find the worktree line which starts the block
    let worktree_line = lines.iter().find(|line| line.starts_with("worktree "))?;

    // Extract path from the worktree line
    let path = worktree_line.strip_prefix("worktree ").unwrap_or("");
    let path_buf = PathBuf::from(path);

    // Build initial WorktreeInfo
    let mut info = WorktreeInfo {
        name: extract_worktree_name(&path_buf),
        path: path_buf,
        branch: String::new(),
        commit: CommitId::new(String::new()),
        is_bare: false,
        is_detached: false,
        is_locked: false,
    };

    // Apply all property updates from the remaining lines
    for line in lines.iter() {
        apply_worktree_property(&mut info, line);
    }

    Some(info)
}

/// Extract the worktree name from its path
fn extract_worktree_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Represents a parsed worktree property
enum WorktreeProperty {
    Head(String),
    Branch(String),
    Detached,
    Bare,
    Locked,
    Unknown,
}

/// Parse a line into a WorktreeProperty
fn parse_worktree_property(line: &str) -> WorktreeProperty {
    if let Some(commit) = line.strip_prefix("HEAD ") {
        WorktreeProperty::Head(commit.to_string())
    } else if let Some(branch) = line.strip_prefix("branch ") {
        if !branch.is_empty() {
            WorktreeProperty::Branch(branch.to_string())
        } else {
            WorktreeProperty::Unknown
        }
    } else {
        match line {
            "detached" => WorktreeProperty::Detached,
            "bare" => WorktreeProperty::Bare,
            line if line.starts_with("locked") => WorktreeProperty::Locked,
            _ => WorktreeProperty::Unknown,
        }
    }
}

/// Apply a single property line to the WorktreeInfo
fn apply_worktree_property(info: &mut WorktreeInfo, line: &str) {
    match parse_worktree_property(line) {
        WorktreeProperty::Head(commit) => info.commit = CommitId::new(commit),
        WorktreeProperty::Branch(branch) => info.branch = branch,
        WorktreeProperty::Detached => info.is_detached = true,
        WorktreeProperty::Bare => info.is_bare = true,
        WorktreeProperty::Locked => info.is_locked = true,
        WorktreeProperty::Unknown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_output_clean() {
        let output = "# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n";
        let status = parse_status_output(output).unwrap();

        assert_eq!(status.branch, Some("main".to_string()));
        assert!(status.is_clean());
        assert!(status.modified.is_empty());
        assert!(status.added.is_empty());
        assert!(status.deleted.is_empty());
        assert!(status.untracked.is_empty());
    }

    #[test]
    fn test_parse_status_output_with_changes() {
        let output = concat!(
            "# branch.head main\n",
            "1 M. N... 100644 100644 100644 abc123 def456 src/main.rs\n",
            "1 A. N... 000000 100644 100644 000000 abc123 src/new.rs\n",
            "? untracked.txt\n"
        );

        let status = parse_status_output(output).unwrap();

        assert_eq!(status.branch, Some("main".to_string()));
        assert!(!status.is_clean());
        assert_eq!(status.modified.len(), 1);
        assert_eq!(status.modified[0], PathBuf::from("src/main.rs"));
        assert_eq!(status.added.len(), 1);
        assert_eq!(status.added[0], PathBuf::from("src/new.rs"));
        assert_eq!(status.untracked.len(), 1);
        assert_eq!(status.untracked[0], PathBuf::from("untracked.txt"));
    }

    #[test]
    fn test_parse_status_output_renamed() {
        let output = "2 R. N... 100644 100644 100644 abc123 def456 R100 new_name.rs\told_name.rs\n";

        let status = parse_status_output(output).unwrap();

        assert_eq!(status.renamed.len(), 1);
        assert_eq!(status.renamed[0].0, PathBuf::from("old_name.rs"));
        assert_eq!(status.renamed[0].1, PathBuf::from("new_name.rs"));
    }

    #[test]
    fn test_parse_status_output_conflict() {
        let output = "u UU N... 100644 100644 100644 100644 abc123 def456 ghi789 conflicted.rs\n";

        let status = parse_status_output(output).unwrap();

        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0], PathBuf::from("conflicted.rs"));
        assert!(status.has_conflicts());
    }

    #[test]
    fn test_parse_diff_output() {
        let output = concat!(
            "10\t5\tsrc/main.rs\n",
            "0\t8\tsrc/deleted.rs\n",
            "15\t0\tsrc/new.rs\n"
        );

        let diff = parse_diff_output(output).unwrap();

        assert_eq!(diff.files_count(), 3);
        assert_eq!(diff.insertions, 25);
        assert_eq!(diff.deletions, 13);

        assert_eq!(diff.files_changed[0].path, PathBuf::from("src/main.rs"));
        assert_eq!(diff.files_changed[0].insertions, 10);
        assert_eq!(diff.files_changed[0].deletions, 5);
        assert_eq!(diff.files_changed[0].change_type, FileChangeType::Modified);

        assert_eq!(diff.files_changed[1].change_type, FileChangeType::Deleted);
        assert_eq!(diff.files_changed[2].change_type, FileChangeType::Added);
    }

    #[test]
    fn test_parse_worktree_list() {
        let output = concat!(
            "worktree /path/to/main\n",
            "HEAD abc1234567890\n",
            "branch main\n",
            "\n",
            "worktree /path/to/feature\n",
            "HEAD def4567890123\n",
            "branch feature/new\n",
            "\n",
            "worktree /path/to/detached\n",
            "HEAD ghi7890123456\n",
            "detached\n",
            "\n"
        );

        let worktrees = parse_worktree_list(output).unwrap();

        assert_eq!(worktrees.len(), 3);

        assert_eq!(worktrees[0].name, "main");
        assert_eq!(worktrees[0].branch, "main");
        assert_eq!(worktrees[0].commit.hash(), "abc1234567890");
        assert!(!worktrees[0].is_detached);

        assert_eq!(worktrees[1].name, "feature");
        assert_eq!(worktrees[1].branch, "feature/new");

        assert_eq!(worktrees[2].name, "detached");
        assert!(worktrees[2].is_detached);
        assert_eq!(worktrees[2].branch, "");
    }

    #[test]
    fn test_parse_worktree_list_with_bare_and_locked() {
        let output = concat!(
            "worktree /path/to/bare\n",
            "HEAD abc1234567890\n",
            "bare\n",
            "\n",
            "worktree /path/to/locked\n",
            "HEAD def4567890123\n",
            "branch feature\n",
            "locked reason for lock\n",
            "\n"
        );

        let worktrees = parse_worktree_list(output).unwrap();

        assert_eq!(worktrees.len(), 2);

        assert_eq!(worktrees[0].name, "bare");
        assert!(worktrees[0].is_bare);
        assert!(!worktrees[0].is_locked);

        assert_eq!(worktrees[1].name, "locked");
        assert!(worktrees[1].is_locked);
        assert!(!worktrees[1].is_bare);
        assert_eq!(worktrees[1].branch, "feature");
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        let output = "";
        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 0);
    }

    #[test]
    fn test_parse_worktree_list_no_trailing_newline() {
        let output = concat!(
            "worktree /path/to/main\n",
            "HEAD abc1234567890\n",
            "branch main"
        );

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].name, "main");
        assert_eq!(worktrees[0].branch, "main");
    }

    #[test]
    fn test_parse_worktree_list_with_empty_branch() {
        let output = concat!(
            "worktree /path/to/main\n",
            "HEAD abc1234567890\n",
            "branch \n",
            "\n"
        );

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch, "");
    }

    #[test]
    fn test_parse_worktree_property() {
        assert!(matches!(
            parse_worktree_property("HEAD abc123"),
            WorktreeProperty::Head(_)
        ));
        assert!(matches!(
            parse_worktree_property("branch main"),
            WorktreeProperty::Branch(_)
        ));
        assert!(matches!(
            parse_worktree_property("detached"),
            WorktreeProperty::Detached
        ));
        assert!(matches!(
            parse_worktree_property("bare"),
            WorktreeProperty::Bare
        ));
        assert!(matches!(
            parse_worktree_property("locked"),
            WorktreeProperty::Locked
        ));
        assert!(matches!(
            parse_worktree_property("locked with reason"),
            WorktreeProperty::Locked
        ));
        assert!(matches!(
            parse_worktree_property("unknown"),
            WorktreeProperty::Unknown
        ));
    }

    #[test]
    fn test_split_into_worktree_blocks() {
        let output = concat!(
            "worktree /path1\n",
            "HEAD abc\n",
            "\n",
            "worktree /path2\n",
            "HEAD def\n"
        );

        let blocks = split_into_worktree_blocks(output);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 2);
        assert_eq!(blocks[1].len(), 2);
    }

    #[test]
    fn test_parse_single_worktree_block() {
        let block = vec![
            "worktree /path/to/repo",
            "HEAD abc1234567890",
            "branch main",
        ];

        let info = parse_single_worktree_block(block).unwrap();
        assert_eq!(info.name, "repo");
        assert_eq!(info.branch, "main");
        assert_eq!(info.commit.hash(), "abc1234567890");
        assert!(!info.is_detached);
        assert!(!info.is_bare);
        assert!(!info.is_locked);
    }

    #[test]
    fn test_parse_single_worktree_block_no_worktree_line() {
        let block = vec!["HEAD abc1234567890", "branch main"];

        let info = parse_single_worktree_block(block);
        assert!(info.is_none());
    }

    #[test]
    fn test_git_status_helper_methods() {
        let mut status = GitStatus::new();
        status.modified.push(PathBuf::from("modified.rs"));
        status.added.push(PathBuf::from("added.rs"));
        status.untracked.push(PathBuf::from("untracked.rs"));

        assert!(!status.is_clean());
        assert!(status.has_staged_changes());
        assert!(status.has_unstaged_changes());
        assert!(!status.has_conflicts());

        let all_files = status.all_changed_files();
        assert_eq!(all_files.len(), 2); // modified and added (not untracked)
    }

    #[test]
    fn test_commit_id() {
        let commit = CommitId::new("abc1234567890def".to_string());

        assert_eq!(commit.hash(), "abc1234567890def");
        assert_eq!(commit.short_hash(), "abc1234");
        assert!(commit.is_valid());

        let invalid_commit = CommitId::new("invalid_hash!".to_string());
        assert!(!invalid_commit.is_valid());
    }
}
