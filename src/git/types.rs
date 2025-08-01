//! Git data structures

use std::path::PathBuf;

/// Git repository status
#[derive(Debug, Clone, PartialEq)]
pub struct GitStatus {
    /// Modified files (staged and unstaged)
    pub modified: Vec<PathBuf>,
    /// Added files (staged)
    pub added: Vec<PathBuf>,
    /// Deleted files (staged and unstaged)
    pub deleted: Vec<PathBuf>,
    /// Untracked files
    pub untracked: Vec<PathBuf>,
    /// Files with conflicts
    pub conflicts: Vec<PathBuf>,
    /// Renamed files (old_path -> new_path)
    pub renamed: Vec<(PathBuf, PathBuf)>,
    /// Current branch name (None if detached HEAD)
    pub branch: Option<String>,
    /// Is repository in merge state
    pub in_merge: bool,
    /// Is repository in rebase state
    pub in_rebase: bool,
}

impl GitStatus {
    /// Create a new empty GitStatus
    pub fn new() -> Self {
        Self {
            modified: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            untracked: Vec::new(),
            conflicts: Vec::new(),
            renamed: Vec::new(),
            branch: None,
            in_merge: false,
            in_rebase: false,
        }
    }

    /// Check if the working directory is clean
    pub fn is_clean(&self) -> bool {
        self.modified.is_empty()
            && self.added.is_empty()
            && self.deleted.is_empty()
            && self.untracked.is_empty()
            && self.conflicts.is_empty()
            && self.renamed.is_empty()
    }

    /// Check if there are staged changes
    pub fn has_staged_changes(&self) -> bool {
        !self.added.is_empty() || !self.renamed.is_empty()
    }

    /// Check if there are unstaged changes
    pub fn has_unstaged_changes(&self) -> bool {
        !self.modified.is_empty() || !self.deleted.is_empty()
    }

    /// Check if there are conflicts
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Get all changed files (staged and unstaged)
    pub fn all_changed_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        files.extend(self.modified.clone());
        files.extend(self.added.clone());
        files.extend(self.deleted.clone());
        files.extend(self.renamed.iter().map(|(_, new)| new.clone()));
        files.sort();
        files.dedup();
        files
    }
}

impl Default for GitStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Git diff information
#[derive(Debug, Clone, PartialEq)]
pub struct GitDiff {
    /// Files changed in the diff
    pub files_changed: Vec<FileDiff>,
    /// Total lines inserted
    pub insertions: usize,
    /// Total lines deleted
    pub deletions: usize,
}

impl GitDiff {
    /// Create a new empty GitDiff
    pub fn new() -> Self {
        Self {
            files_changed: Vec::new(),
            insertions: 0,
            deletions: 0,
        }
    }

    /// Get total number of files changed
    pub fn files_count(&self) -> usize {
        self.files_changed.len()
    }

    /// Check if diff is empty
    pub fn is_empty(&self) -> bool {
        self.files_changed.is_empty()
    }
}

impl Default for GitDiff {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual file diff information
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiff {
    /// File path
    pub path: PathBuf,
    /// Lines added in this file
    pub insertions: usize,
    /// Lines deleted in this file
    pub deletions: usize,
    /// File change type
    pub change_type: FileChangeType,
}

/// Type of file change
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    /// File was added
    Added,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
    /// File was renamed
    Renamed { from: PathBuf },
    /// File was copied
    Copied { from: PathBuf },
}

/// Git worktree information
#[derive(Debug, Clone, PartialEq)]
pub struct WorktreeInfo {
    /// Worktree name/identifier
    pub name: String,
    /// Path to the worktree
    pub path: PathBuf,
    /// Current branch in the worktree
    pub branch: String,
    /// Current commit hash
    pub commit: CommitId,
    /// Whether the worktree is bare
    pub is_bare: bool,
    /// Whether the worktree is detached
    pub is_detached: bool,
    /// Whether the worktree is locked
    pub is_locked: bool,
}

/// Git commit identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitId {
    hash: String,
}

impl CommitId {
    /// Create a new CommitId
    pub fn new(hash: String) -> Self {
        Self { hash }
    }

    /// Get the full commit hash
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Get the short commit hash (first 7 characters)
    pub fn short_hash(&self) -> &str {
        if self.hash.len() >= 7 {
            &self.hash[..7]
        } else {
            &self.hash
        }
    }

    /// Check if this is a valid commit hash
    pub fn is_valid(&self) -> bool {
        !self.hash.is_empty() && self.hash.chars().all(|c| c.is_ascii_hexdigit())
    }
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hash)
    }
}

impl From<String> for CommitId {
    fn from(hash: String) -> Self {
        Self::new(hash)
    }
}

impl From<&str> for CommitId {
    fn from(hash: &str) -> Self {
        Self::new(hash.to_string())
    }
}

/// Git repository state information
#[derive(Debug, Clone, PartialEq)]
pub struct GitRepoState {
    /// Current branch (None if detached HEAD)
    pub current_branch: Option<String>,
    /// Current commit
    pub current_commit: CommitId,
    /// Repository status
    pub status: GitStatus,
    /// List of all branches
    pub branches: Vec<String>,
    /// List of all tags
    pub tags: Vec<String>,
    /// Remote repositories
    pub remotes: Vec<String>,
}
