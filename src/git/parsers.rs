//! Git output parsers

use super::types::*;
use anyhow::Result;
use std::path::PathBuf;

/// Parse git status --porcelain=v2 output
pub fn parse_status_output(output: &str) -> Result<GitStatus> {
    let mut status = GitStatus::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        if line.starts_with("# branch.head ") {
            let branch_name = line.strip_prefix("# branch.head ").unwrap_or("");
            if branch_name != "(detached)" {
                status.branch = Some(branch_name.to_string());
            }
        } else if line.starts_with("# branch.upstream ") {
            // Ignore upstream info for now
        } else if line.starts_with("# branch.ab ") {
            // Ignore ahead/behind info for now
        } else if line.starts_with("# merge.in-progress ") {
            status.in_merge = line.contains("true");
        } else if line.starts_with("1 ") {
            // Regular file entry: 1 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <path>
            parse_status_line(&mut status, line)?;
        } else if line.starts_with("2 ") {
            // Renamed/copied file entry: 2 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path><sep><origPath>
            parse_renamed_line(&mut status, line)?;
        } else if line.starts_with("u ") {
            // Unmerged file entry: u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
            parse_unmerged_line(&mut status, line)?;
        } else if line.starts_with("? ") {
            // Untracked file: ? <path>
            let path = line.strip_prefix("? ").unwrap_or("").trim();
            if !path.is_empty() {
                status.untracked.push(PathBuf::from(path));
            }
        } else if line.starts_with("! ") {
            // Ignored file: ! <path>
            // We don't track ignored files in our status
        }
    }

    Ok(status)
}

fn parse_status_line(status: &mut GitStatus, line: &str) -> Result<()> {
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

fn parse_renamed_line(status: &mut GitStatus, line: &str) -> Result<()> {
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

fn parse_unmerged_line(status: &mut GitStatus, line: &str) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 11 {
        return Ok(());
    }

    let path = parts[10..].join(" ");
    status.conflicts.push(PathBuf::from(path));

    Ok(())
}

/// Parse git diff --numstat output
pub fn parse_diff_output(output: &str) -> Result<GitDiff> {
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
pub fn parse_worktree_list(output: &str) -> Result<Vec<WorktreeInfo>> {
    let mut worktrees = Vec::new();
    let mut current_worktree: Option<WorktreeInfo> = None;

    for line in output.lines() {
        if line.is_empty() {
            if let Some(worktree) = current_worktree.take() {
                worktrees.push(worktree);
            }
            continue;
        }

        if line.starts_with("worktree ") {
            // Start of new worktree entry
            if let Some(worktree) = current_worktree.take() {
                worktrees.push(worktree);
            }

            let path = line.strip_prefix("worktree ").unwrap_or("");
            let path_buf = PathBuf::from(path);
            let name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            current_worktree = Some(WorktreeInfo {
                name,
                path: path_buf,
                branch: String::new(),
                commit: CommitId::new(String::new()),
                is_bare: false,
                is_detached: false,
                is_locked: false,
            });
        } else if let Some(ref mut worktree) = current_worktree {
            if line.starts_with("HEAD ") {
                let commit_hash = line.strip_prefix("HEAD ").unwrap_or("");
                worktree.commit = CommitId::new(commit_hash.to_string());
            } else if line.starts_with("branch ") {
                let branch = line.strip_prefix("branch ").unwrap_or("");
                if !branch.is_empty() {
                    worktree.branch = branch.to_string();
                }
            } else if line == "detached" {
                worktree.is_detached = true;
            } else if line == "bare" {
                worktree.is_bare = true;
            } else if line.starts_with("locked") {
                worktree.is_locked = true;
            }
        }
    }

    // Don't forget the last worktree
    if let Some(worktree) = current_worktree {
        worktrees.push(worktree);
    }

    Ok(worktrees)
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
