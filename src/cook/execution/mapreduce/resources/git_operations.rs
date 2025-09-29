//! Comprehensive Git operations service for MapReduce workflows
//!
//! This module provides detailed git commit and file tracking capabilities
//! for MapReduce agents, including commit metadata extraction, file modification
//! tracking, and integration with merge workflows.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use chrono::{DateTime, Utc};
use git2::{Commit, Delta, DiffOptions, Oid, Repository, Sort, Time};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, trace, warn};

/// Configuration for git operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationsConfig {
    /// Enable caching of git operations
    pub enable_caching: bool,

    /// Maximum age for cached git data in seconds
    pub cache_ttl_secs: u64,

    /// Maximum number of commits to retrieve
    pub max_commits: usize,

    /// Maximum number of files to track
    pub max_files: usize,

    /// Include file content diffs in results
    pub include_diffs: bool,

    /// Git operation timeout in seconds
    pub operation_timeout_secs: u64,
}

impl Default for GitOperationsConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_ttl_secs: 300, // 5 minutes
            max_commits: 1000,
            max_files: 5000,
            include_diffs: false,
            operation_timeout_secs: 30,
        }
    }
}

/// Detailed commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub author: AuthorInfo,
    pub committer: AuthorInfo,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub parent_ids: Vec<String>,
    pub tree_id: String,
    pub stats: Option<CommitStats>,
    pub files_changed: Vec<String>,
}

/// Author/committer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub name: String,
    pub email: String,
    pub timestamp: DateTime<Utc>,
}

/// Commit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// File modification information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileModificationInfo {
    pub path: PathBuf,
    pub modification_type: ModificationType,
    pub size_before: Option<u64>,
    pub size_after: Option<u64>,
    pub last_modified: DateTime<Utc>,
    pub commit_id: Option<String>,
    pub diff_stats: Option<DiffStats>,
    pub content_diff: Option<String>,
}

/// Type of file modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModificationType {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
    Copied { from: PathBuf },
}

/// Diff statistics for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_context: usize,
}

/// Git information for merge workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeGitInfo {
    pub commits: Vec<CommitInfo>,
    pub modified_files: Vec<FileModificationInfo>,
    pub target_branch: String,
    pub worktree_path: PathBuf,
    pub generated_at: DateTime<Utc>,
}

/// Git operations service
pub struct GitOperationsService {
    config: GitOperationsConfig,
}

impl GitOperationsService {
    /// Create a new git operations service
    pub fn new(config: GitOperationsConfig) -> Self {
        Self { config }
    }

    /// Get commits from a worktree repository
    pub async fn get_worktree_commits(
        &mut self,
        worktree_path: &Path,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
    ) -> MapReduceResult<Vec<CommitInfo>> {
        trace!("Getting commits from worktree: {}", worktree_path.display());

        let repo = self.open_repository(worktree_path)?;
        let mut revwalk = repo
            .revwalk()
            .map_err(|e| self.create_git_error("revwalk", &e.to_string()))?;

        // Start from HEAD
        revwalk
            .push_head()
            .map_err(|e| self.create_git_error("push_head", &e.to_string()))?;

        // Sort by time descending (newest first)
        revwalk
            .set_sorting(Sort::TIME)
            .map_err(|e| self.create_git_error("set_sorting", &e.to_string()))?;

        let mut commits = Vec::new();
        let mut count = 0;

        for oid_result in revwalk {
            if count >= self.config.max_commits {
                debug!("Reached max_commits limit: {}", self.config.max_commits);
                break;
            }

            let oid = oid_result.map_err(|e| self.create_git_error("walk_oid", &e.to_string()))?;

            let commit = repo
                .find_commit(oid)
                .map_err(|e| self.create_git_error("find_commit", &e.to_string()))?;

            let commit_time =
                DateTime::from_timestamp(commit.time().seconds(), 0).unwrap_or_else(Utc::now);

            // Apply time filters
            if let Some(since) = since {
                if commit_time < since {
                    trace!(
                        "Stopping walk: commit time {} < since {}",
                        commit_time,
                        since
                    );
                    break; // Commits are sorted by time, so we can stop here
                }
            }

            if let Some(until) = until {
                if commit_time > until {
                    trace!("Skipping commit: time {} > until {}", commit_time, until);
                    continue;
                }
            }

            let commit_info = self.build_commit_info(&repo, &commit)?;
            commits.push(commit_info);
            count += 1;
        }

        info!("Retrieved {} commits from worktree", commits.len());
        Ok(commits)
    }

    /// Get modified files from a worktree repository
    pub async fn get_worktree_modified_files(
        &mut self,
        worktree_path: &Path,
        since_commit: Option<&str>,
    ) -> MapReduceResult<Vec<FileModificationInfo>> {
        trace!(
            "Getting modified files from worktree: {}",
            worktree_path.display()
        );

        let repo = self.open_repository(worktree_path)?;
        let mut modifications = Vec::new();

        // Get working directory changes
        let workdir_changes = self.get_working_directory_changes(&repo)?;
        modifications.extend(workdir_changes);

        // Get committed changes since specified commit
        if let Some(since) = since_commit {
            let committed_changes = self.get_committed_changes(&repo, since)?;
            modifications.extend(committed_changes);
        } else {
            // If no base commit specified, get changes from the last 10 commits
            let recent_changes = self.get_recent_committed_changes(&repo)?;
            modifications.extend(recent_changes);
        }

        // Deduplicate and sort
        self.deduplicate_modifications(&mut modifications);

        // Apply file limit
        if modifications.len() > self.config.max_files {
            warn!(
                "Truncating modifications from {} to max_files: {}",
                modifications.len(),
                self.config.max_files
            );
            modifications.truncate(self.config.max_files);
        }

        info!("Found {} modified files in worktree", modifications.len());
        Ok(modifications)
    }

    /// Build detailed commit information
    fn build_commit_info(&self, repo: &Repository, commit: &Commit) -> MapReduceResult<CommitInfo> {
        let id = commit.id().to_string();
        let short_id = format!("{:.7}", id);

        let author = commit.author();
        let committer = commit.committer();

        let author_info = AuthorInfo {
            name: author.name().unwrap_or("Unknown").to_string(),
            email: author.email().unwrap_or("unknown@example.com").to_string(),
            timestamp: self.time_to_datetime(author.when()),
        };

        let committer_info = AuthorInfo {
            name: committer.name().unwrap_or("Unknown").to_string(),
            email: committer
                .email()
                .unwrap_or("unknown@example.com")
                .to_string(),
            timestamp: self.time_to_datetime(committer.when()),
        };

        let message = commit.message().unwrap_or("").to_string();
        let tree_id = commit.tree_id().to_string();

        let parent_ids: Vec<String> = commit.parent_ids().map(|id| id.to_string()).collect();

        // Calculate commit statistics
        let stats = self.calculate_commit_stats(repo, commit)?;
        let files_changed = self.get_commit_files(repo, commit)?;

        Ok(CommitInfo {
            id,
            short_id,
            author: author_info,
            committer: committer_info,
            message,
            timestamp: self.time_to_datetime(commit.time()),
            parent_ids,
            tree_id,
            stats: Some(stats),
            files_changed,
        })
    }

    /// Calculate commit statistics
    fn calculate_commit_stats(
        &self,
        repo: &Repository,
        commit: &Commit,
    ) -> MapReduceResult<CommitStats> {
        let tree = commit
            .tree()
            .map_err(|e| self.create_git_error("get_tree", &e.to_string()))?;

        let parent_tree = if commit.parent_count() > 0 {
            commit.parent(0).and_then(|p| p.tree()).ok()
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
            .map_err(|e| self.create_git_error("diff_tree", &e.to_string()))?;

        let stats = diff
            .stats()
            .map_err(|e| self.create_git_error("diff_stats", &e.to_string()))?;

        Ok(CommitStats {
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
        })
    }

    /// Get list of files changed in a commit
    fn get_commit_files(&self, repo: &Repository, commit: &Commit) -> MapReduceResult<Vec<String>> {
        let tree = commit
            .tree()
            .map_err(|e| self.create_git_error("get_tree", &e.to_string()))?;

        let parent_tree = if commit.parent_count() > 0 {
            commit.parent(0).and_then(|p| p.tree()).ok()
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
            .map_err(|e| self.create_git_error("diff_tree", &e.to_string()))?;

        let mut files = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    files.push(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| self.create_git_error("diff_foreach", &e.to_string()))?;

        Ok(files)
    }

    /// Get working directory changes
    fn get_working_directory_changes(
        &self,
        repo: &Repository,
    ) -> MapReduceResult<Vec<FileModificationInfo>> {
        let head = repo
            .head()
            .map_err(|e| self.create_git_error("get_head", &e.to_string()))?;

        let tree = head
            .peel_to_tree()
            .map_err(|e| self.create_git_error("peel_to_tree", &e.to_string()))?;

        let mut diff_opts = DiffOptions::new();
        diff_opts.include_untracked(true);

        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut diff_opts))
            .map_err(|e| self.create_git_error("diff_workdir", &e.to_string()))?;

        let mut modifications = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(file_info) = self.process_diff_delta(&delta) {
                    modifications.push(file_info);
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| self.create_git_error("diff_foreach", &e.to_string()))?;

        Ok(modifications)
    }

    /// Get committed changes since a specific commit
    fn get_committed_changes(
        &self,
        repo: &Repository,
        since_commit: &str,
    ) -> MapReduceResult<Vec<FileModificationInfo>> {
        let since_oid = Oid::from_str(since_commit)
            .map_err(|e| self.create_git_error("parse_oid", &e.to_string()))?;

        let since_commit = repo
            .find_commit(since_oid)
            .map_err(|e| self.create_git_error("find_since_commit", &e.to_string()))?;

        let head_commit = repo
            .head()
            .and_then(|h| h.peel_to_commit())
            .map_err(|e| self.create_git_error("get_head_commit", &e.to_string()))?;

        let since_tree = since_commit
            .tree()
            .map_err(|e| self.create_git_error("get_since_tree", &e.to_string()))?;

        let head_tree = head_commit
            .tree()
            .map_err(|e| self.create_git_error("get_head_tree", &e.to_string()))?;

        let mut diff_opts = DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(Some(&since_tree), Some(&head_tree), Some(&mut diff_opts))
            .map_err(|e| self.create_git_error("diff_trees", &e.to_string()))?;

        let mut modifications = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(file_info) = self.process_diff_delta(&delta) {
                    modifications.push(file_info);
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| self.create_git_error("diff_foreach", &e.to_string()))?;

        Ok(modifications)
    }

    /// Get recent committed changes (last 10 commits)
    fn get_recent_committed_changes(
        &self,
        repo: &Repository,
    ) -> MapReduceResult<Vec<FileModificationInfo>> {
        let head_commit = repo
            .head()
            .and_then(|h| h.peel_to_commit())
            .map_err(|e| self.create_git_error("get_head_commit", &e.to_string()))?;

        // Try to get parent commit (10 commits back or as far as possible)
        let mut current = head_commit.clone();
        let mut depth = 0;
        while depth < 10 && current.parent_count() > 0 {
            current = match current.parent(0) {
                Ok(parent) => parent,
                Err(_) => break,
            };
            depth += 1;
        }

        if depth == 0 {
            // No parent commits, get all files in the repository
            return self.get_all_files(repo);
        }

        let base_tree = current
            .tree()
            .map_err(|e| self.create_git_error("get_base_tree", &e.to_string()))?;

        let head_tree = head_commit
            .tree()
            .map_err(|e| self.create_git_error("get_head_tree", &e.to_string()))?;

        let mut diff_opts = DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut diff_opts))
            .map_err(|e| self.create_git_error("diff_trees", &e.to_string()))?;

        let mut modifications = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(file_info) = self.process_diff_delta(&delta) {
                    modifications.push(file_info);
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| self.create_git_error("diff_foreach", &e.to_string()))?;

        Ok(modifications)
    }

    /// Get all files in the repository (for new repos with no history)
    fn get_all_files(&self, repo: &Repository) -> MapReduceResult<Vec<FileModificationInfo>> {
        let head = repo
            .head()
            .map_err(|e| self.create_git_error("get_head", &e.to_string()))?;

        let tree = head
            .peel_to_tree()
            .map_err(|e| self.create_git_error("peel_to_tree", &e.to_string()))?;

        let mut files = Vec::new();

        tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
            if let Some(name) = entry.name() {
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    files.push(FileModificationInfo {
                        path: PathBuf::from(name),
                        modification_type: ModificationType::Added,
                        size_before: None,
                        size_after: None,
                        last_modified: Utc::now(),
                        commit_id: None,
                        diff_stats: None,
                        content_diff: None,
                    });
                }
            }
            git2::TreeWalkResult::Ok
        })
        .map_err(|e| self.create_git_error("tree_walk", &e.to_string()))?;

        Ok(files)
    }

    /// Process a diff delta into file modification info
    fn process_diff_delta(&self, delta: &git2::DiffDelta) -> Option<FileModificationInfo> {
        let new_file = delta.new_file();
        let old_file = delta.old_file();

        let path = new_file.path().or_else(|| old_file.path())?;

        let modification_type = match delta.status() {
            Delta::Added => ModificationType::Added,
            Delta::Deleted => ModificationType::Deleted,
            Delta::Modified => ModificationType::Modified,
            Delta::Renamed => {
                if let Some(old_path) = old_file.path() {
                    ModificationType::Renamed {
                        from: old_path.to_path_buf(),
                    }
                } else {
                    ModificationType::Modified
                }
            }
            Delta::Copied => {
                if let Some(old_path) = old_file.path() {
                    ModificationType::Copied {
                        from: old_path.to_path_buf(),
                    }
                } else {
                    ModificationType::Modified
                }
            }
            _ => return None,
        };

        Some(FileModificationInfo {
            path: path.to_path_buf(),
            modification_type,
            size_before: if old_file.size() > 0 {
                Some(old_file.size())
            } else {
                None
            },
            size_after: if new_file.size() > 0 {
                Some(new_file.size())
            } else {
                None
            },
            last_modified: Utc::now(),
            commit_id: None,
            diff_stats: None,
            content_diff: None,
        })
    }

    /// Deduplicate modifications keeping the most recent for each file
    fn deduplicate_modifications(&self, modifications: &mut Vec<FileModificationInfo>) {
        modifications.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then_with(|| b.last_modified.cmp(&a.last_modified))
        });

        modifications.dedup_by(|a, b| a.path == b.path);
    }

    /// Open repository at given path
    fn open_repository(&self, path: &Path) -> MapReduceResult<Repository> {
        let canonical_path = path.canonicalize().map_err(|e| MapReduceError::General {
            message: format!("Invalid repository path {}: {}", path.display(), e),
            source: None,
        })?;

        debug!("Opening repository at: {}", canonical_path.display());
        Repository::open(&canonical_path).map_err(|e| MapReduceError::General {
            message: format!(
                "Failed to open repository at {}: {}",
                canonical_path.display(),
                e
            ),
            source: None,
        })
    }

    /// Get git information for merge workflow variables
    pub async fn get_merge_git_info(
        &mut self,
        worktree_path: &Path,
        target_branch: &str,
    ) -> MapReduceResult<MergeGitInfo> {
        let commits = self.get_worktree_commits(worktree_path, None, None).await?;
        let modified_files = self
            .get_worktree_modified_files(worktree_path, None)
            .await?;

        Ok(MergeGitInfo {
            commits,
            modified_files,
            target_branch: target_branch.to_string(),
            worktree_path: worktree_path.to_path_buf(),
            generated_at: Utc::now(),
        })
    }

    /// Convert git2 Time to DateTime<Utc>
    fn time_to_datetime(&self, time: Time) -> DateTime<Utc> {
        DateTime::from_timestamp(time.seconds(), 0).unwrap_or_else(Utc::now)
    }

    /// Create a standardized git error
    fn create_git_error(&self, operation: &str, message: &str) -> MapReduceError {
        MapReduceError::General {
            message: format!("Git operation '{}' failed: {}", operation, message),
            source: None,
        }
    }
}

/// Helper trait for converting git results to MapReduce format
pub trait GitResultExt {
    /// Convert to simple string list (for backward compatibility)
    fn to_string_list(&self) -> Vec<String>;
}

impl GitResultExt for Vec<CommitInfo> {
    fn to_string_list(&self) -> Vec<String> {
        self.iter().map(|c| c.id.clone()).collect()
    }
}

impl GitResultExt for Vec<FileModificationInfo> {
    fn to_string_list(&self) -> Vec<String> {
        self.iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect()
    }
}
