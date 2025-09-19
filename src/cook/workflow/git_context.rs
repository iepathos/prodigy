//! Git context tracking for workflows
//!
//! Provides automatic tracking of git changes during workflow execution,
//! exposing file change information as interpolatable variables.

use anyhow::{Context, Result};
use git2::{DiffOptions, Oid, Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Git change tracker for workflow execution
#[derive(Debug, Clone)]
pub struct GitChangeTracker {
    /// Git repository handle
    repo_path: PathBuf,
    /// Initial commit when workflow started
    pub(crate) workflow_start_commit: Option<String>,
    /// Changes tracked for each step
    pub(crate) step_changes: HashMap<String, StepChanges>,
    /// Current step ID
    pub(crate) current_step_id: Option<String>,
    /// Last known commit before current step
    pub(crate) last_commit: Option<String>,
}

/// Changes tracked for a single step
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepChanges {
    /// Files added in this step
    pub files_added: Vec<String>,
    /// Files modified in this step
    pub files_modified: Vec<String>,
    /// Files deleted in this step
    pub files_deleted: Vec<String>,
    /// Commit SHAs created in this step
    pub commits: Vec<String>,
    /// Lines inserted
    pub insertions: usize,
    /// Lines deleted
    pub deletions: usize,
}

impl StepChanges {
    /// Get all changed files (added + modified + deleted)
    pub fn files_changed(&self) -> Vec<String> {
        let mut all_files = Vec::new();
        all_files.extend(self.files_added.clone());
        all_files.extend(self.files_modified.clone());
        all_files.extend(self.files_deleted.clone());
        all_files.sort();
        all_files.dedup();
        all_files
    }

    /// Get commit count
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Merge changes from another StepChanges
    pub fn merge(&mut self, other: &StepChanges) {
        self.files_added.extend(other.files_added.clone());
        self.files_modified.extend(other.files_modified.clone());
        self.files_deleted.extend(other.files_deleted.clone());
        self.commits.extend(other.commits.clone());
        self.insertions += other.insertions;
        self.deletions += other.deletions;

        // Remove duplicates
        self.files_added.sort();
        self.files_added.dedup();
        self.files_modified.sort();
        self.files_modified.dedup();
        self.files_deleted.sort();
        self.files_deleted.dedup();
        self.commits.sort();
        self.commits.dedup();
    }

    /// Filter files by pattern
    pub fn filter_files(&self, files: &[String], pattern: &str) -> Vec<String> {
        let matcher = match glob::Pattern::new(pattern) {
            Ok(m) => m,
            Err(e) => {
                warn!("Invalid glob pattern '{}': {}", pattern, e);
                return files.to_vec();
            }
        };

        files
            .iter()
            .filter(|f| matcher.matches(f))
            .cloned()
            .collect()
    }
}

/// Format for variable output
#[derive(Debug, Clone, Copy)]
pub enum VariableFormat {
    /// Space-separated (default)
    SpaceSeparated,
    /// Newline-separated
    NewlineSeparated,
    /// JSON array
    JsonArray,
    /// Comma-separated
    CommaSeparated,
}

impl GitChangeTracker {
    /// Create a new git change tracker
    pub fn new(working_dir: impl AsRef<Path>) -> Result<Self> {
        let repo_path = working_dir.as_ref().to_path_buf();

        // Try to open repository to validate it exists
        if let Ok(repo) = Repository::open(&repo_path) {
            let head_commit = Self::get_head_commit(&repo)?;
            debug!("Initialized GitChangeTracker at commit: {:?}", head_commit);

            Ok(Self {
                repo_path,
                workflow_start_commit: head_commit.clone(),
                step_changes: HashMap::new(),
                current_step_id: None,
                last_commit: head_commit,
            })
        } else {
            // Not a git repository, tracker will be inactive
            debug!("Working directory is not a git repository, git tracking disabled");
            Ok(Self {
                repo_path,
                workflow_start_commit: None,
                step_changes: HashMap::new(),
                current_step_id: None,
                last_commit: None,
            })
        }
    }

    /// Get HEAD commit SHA
    fn get_head_commit(repo: &Repository) -> Result<Option<String>> {
        if repo.head_detached()? {
            // Detached HEAD, get commit directly
            let head = repo.head()?;
            if let Some(oid) = head.target() {
                return Ok(Some(oid.to_string()));
            }
        } else {
            // On a branch
            let head = repo.head()?;
            if let Some(oid) = head.target() {
                return Ok(Some(oid.to_string()));
            }
        }
        Ok(None)
    }

    /// Start tracking a new step
    pub fn begin_step(&mut self, step_id: impl Into<String>) -> Result<()> {
        let step_id = step_id.into();
        debug!("Beginning step: {}", step_id);

        // Save current commit as baseline for this step
        if let Ok(repo) = Repository::open(&self.repo_path) {
            self.last_commit = Self::get_head_commit(&repo)?;
        }

        self.current_step_id = Some(step_id.clone());
        self.step_changes.insert(step_id, StepChanges::default());
        Ok(())
    }

    /// Complete tracking for current step and calculate changes
    pub fn complete_step(&mut self) -> Result<StepChanges> {
        let step_id = self
            .current_step_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active step to complete"))?;

        debug!("Completing step: {}", step_id);

        // Calculate changes if we're in a git repo
        if self.workflow_start_commit.is_some() {
            let changes = self.calculate_step_changes()?;
            self.step_changes.insert(step_id.clone(), changes.clone());

            // Update last commit for next step
            if let Ok(repo) = Repository::open(&self.repo_path) {
                self.last_commit = Self::get_head_commit(&repo)?;
            }

            self.current_step_id = None;
            Ok(changes)
        } else {
            // Not in a git repo, return empty changes
            self.current_step_id = None;
            Ok(StepChanges::default())
        }
    }

    /// Calculate changes for the current step
    fn calculate_step_changes(&self) -> Result<StepChanges> {
        let repo = Repository::open(&self.repo_path).context("Failed to open git repository")?;

        let mut changes = StepChanges::default();

        // Get current HEAD
        let current_commit = Self::get_head_commit(&repo)?;

        // Calculate file changes using git status for uncommitted changes
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_opts))?;

        for entry in statuses.iter() {
            let path = match entry.path() {
                Some(p) => p,
                None => continue,
            };
            let status = entry.status();

            if status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW) {
                changes.files_added.push(path.to_string());
            } else if status.contains(Status::WT_MODIFIED)
                || status.contains(Status::INDEX_MODIFIED)
            {
                changes.files_modified.push(path.to_string());
            } else if status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED)
            {
                changes.files_deleted.push(path.to_string());
            }
        }

        // If there's a previous commit, calculate committed changes
        if let (Some(last), Some(current)) = (&self.last_commit, &current_commit) {
            if last != current {
                // New commits were made
                let last_oid = Oid::from_str(last)?;
                let current_oid = Oid::from_str(current)?;

                // Get commits between last and current
                let mut revwalk = repo.revwalk()?;
                revwalk.push(current_oid)?;
                revwalk.hide(last_oid)?;

                for oid in revwalk {
                    let oid = oid?;
                    changes.commits.push(oid.to_string());
                }

                // Calculate diff stats
                if let (Ok(last_commit), Ok(current_commit)) =
                    (repo.find_commit(last_oid), repo.find_commit(current_oid))
                {
                    if let (Some(last_tree), Some(current_tree)) =
                        (last_commit.tree().ok(), current_commit.tree().ok())
                    {
                        let diff = repo.diff_tree_to_tree(
                            Some(&last_tree),
                            Some(&current_tree),
                            Some(&mut DiffOptions::new()),
                        )?;

                        let stats = diff.stats()?;
                        changes.insertions = stats.insertions();
                        changes.deletions = stats.deletions();

                        // Process diff to get file changes from commits
                        diff.foreach(
                            &mut |delta, _progress| {
                                if let Some(path) = delta.new_file().path() {
                                    let path_str = path.to_string_lossy().to_string();
                                    match delta.status() {
                                        git2::Delta::Added => {
                                            if !changes.files_added.contains(&path_str) {
                                                changes.files_added.push(path_str);
                                            }
                                        }
                                        git2::Delta::Modified => {
                                            if !changes.files_modified.contains(&path_str) {
                                                changes.files_modified.push(path_str);
                                            }
                                        }
                                        git2::Delta::Deleted => {
                                            if !changes.files_deleted.contains(&path_str) {
                                                changes.files_deleted.push(path_str);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                true
                            },
                            None,
                            None,
                            None,
                        )?;
                    }
                }
            }
        }

        // Remove duplicates and sort
        changes.files_added.sort();
        changes.files_added.dedup();
        changes.files_modified.sort();
        changes.files_modified.dedup();
        changes.files_deleted.sort();
        changes.files_deleted.dedup();

        debug!(
            "Step changes: {} added, {} modified, {} deleted, {} commits",
            changes.files_added.len(),
            changes.files_modified.len(),
            changes.files_deleted.len(),
            changes.commits.len()
        );

        Ok(changes)
    }

    /// Get changes for a specific step
    pub fn get_step_changes(&self, step_id: &str) -> Option<&StepChanges> {
        self.step_changes.get(step_id)
    }

    /// Get cumulative workflow changes
    pub fn get_workflow_changes(&self) -> StepChanges {
        let mut cumulative = StepChanges::default();
        for changes in self.step_changes.values() {
            cumulative.merge(changes);
        }
        cumulative
    }

    /// Format file list according to format specification
    pub fn format_file_list(files: &[String], format: VariableFormat) -> String {
        match format {
            VariableFormat::SpaceSeparated => files.join(" "),
            VariableFormat::NewlineSeparated => files.join("\n"),
            VariableFormat::JsonArray => {
                serde_json::to_string(files).unwrap_or_else(|_| "[]".to_string())
            }
            VariableFormat::CommaSeparated => files.join(","),
        }
    }

    /// Resolve a git context variable
    pub fn resolve_variable(&self, var_path: &str) -> Result<String> {
        let parts: Vec<&str> = var_path.split('.').collect();

        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty variable path"));
        }

        // Parse format and pattern from variable path
        // Format: step.files_added:*.md or step.files_added:json
        let (base_path, modifier) = if let Some(pos) = parts.last().unwrap().find(':') {
            let last = parts.last().unwrap();
            let base = &last[..pos];
            let modifier = &last[pos + 1..];
            (
                parts[..parts.len() - 1]
                    .iter()
                    .chain(&[base])
                    .copied()
                    .collect::<Vec<_>>(),
                Some(modifier),
            )
        } else {
            (parts, None)
        };

        // Determine format from modifier
        let format = match modifier {
            Some("json") => VariableFormat::JsonArray,
            Some("lines") | Some("newline") => VariableFormat::NewlineSeparated,
            Some("csv") | Some("comma") => VariableFormat::CommaSeparated,
            _ => VariableFormat::SpaceSeparated,
        };

        // Check if modifier is a glob pattern
        let pattern = modifier.filter(|m| m.contains('*') || m.contains('?'));

        match base_path[..] {
            ["step", var_name] => {
                // Get current step changes
                let changes = if let Some(step_id) = &self.current_step_id {
                    self.step_changes.get(step_id).cloned().unwrap_or_default()
                } else {
                    StepChanges::default()
                };

                self.resolve_step_variable(&changes, var_name, format, pattern)
            }
            ["workflow", var_name] => {
                let changes = self.get_workflow_changes();
                self.resolve_step_variable(&changes, var_name, format, pattern)
            }
            _ => Err(anyhow::anyhow!("Unknown git variable path: {}", var_path)),
        }
    }

    /// Resolve a variable for step changes
    fn resolve_step_variable(
        &self,
        changes: &StepChanges,
        var_name: &str,
        format: VariableFormat,
        pattern: Option<&str>,
    ) -> Result<String> {
        let files = match var_name {
            "files_added" => &changes.files_added,
            "files_modified" => &changes.files_modified,
            "files_deleted" => &changes.files_deleted,
            "files_changed" => {
                let all = changes.files_changed();
                return Ok(if let Some(p) = pattern {
                    Self::format_file_list(&changes.filter_files(&all, p), format)
                } else {
                    Self::format_file_list(&all, format)
                });
            }
            "commits" => {
                return Ok(Self::format_file_list(&changes.commits, format));
            }
            "commit_count" => {
                return Ok(changes.commit_count().to_string());
            }
            "insertions" => {
                return Ok(changes.insertions.to_string());
            }
            "deletions" => {
                return Ok(changes.deletions.to_string());
            }
            _ => return Err(anyhow::anyhow!("Unknown step variable: {}", var_name)),
        };

        // Apply pattern filter if specified
        let filtered = if let Some(p) = pattern {
            changes.filter_files(files, p)
        } else {
            files.clone()
        };

        Ok(Self::format_file_list(&filtered, format))
    }

    /// Check if tracker is active (in a git repository)
    pub fn is_active(&self) -> bool {
        self.workflow_start_commit.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_test_repo() -> Result<TempDir> {
        let dir = TempDir::new()?;
        {
            let repo = Repository::init(dir.path())?;

            // Create initial commit
            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = {
                let mut index = repo.index()?;
                index.write_tree()?
            };
            let tree = repo.find_tree(tree_id)?;
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;
        }

        Ok(dir)
    }

    #[test]
    fn test_step_changes() {
        let changes = StepChanges {
            files_added: vec!["a.txt".into(), "b.txt".into()],
            files_modified: vec!["c.txt".into()],
            files_deleted: vec!["d.txt".into()],
            commits: vec!["abc123".into()],
            insertions: 10,
            deletions: 5,
        };

        assert_eq!(changes.files_changed().len(), 4);
        assert_eq!(changes.commit_count(), 1);
    }

    #[test]
    fn test_filter_files() {
        let changes = StepChanges {
            files_added: vec![
                "src/main.rs".into(),
                "src/lib.rs".into(),
                "README.md".into(),
                "docs/guide.md".into(),
            ],
            ..Default::default()
        };

        let filtered = changes.filter_files(&changes.files_added, "*.md");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"README.md".to_string()));
        assert!(filtered.contains(&"docs/guide.md".to_string()));
    }

    #[test]
    fn test_format_file_list() {
        let files = vec!["a.txt".into(), "b.txt".into()];

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::SpaceSeparated),
            "a.txt b.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::NewlineSeparated),
            "a.txt\nb.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::CommaSeparated),
            "a.txt,b.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::JsonArray),
            r#"["a.txt","b.txt"]"#
        );
    }

    #[test]
    fn test_tracker_initialization() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        assert!(tracker.is_active());
        assert!(tracker.workflow_start_commit.is_some());

        Ok(())
    }

    #[test]
    fn test_non_git_directory() -> Result<()> {
        let dir = TempDir::new()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        assert!(!tracker.is_active());
        assert!(tracker.workflow_start_commit.is_none());

        Ok(())
    }
}
