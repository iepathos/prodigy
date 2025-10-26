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

/// Type of file change detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Unknown,
}

/// Classify a git status into a file change type
fn classify_file_status(status: Status) -> FileChangeType {
    if should_track_as_added(status) {
        FileChangeType::Added
    } else if should_track_as_modified(status) {
        FileChangeType::Modified
    } else if should_track_as_deleted(status) {
        FileChangeType::Deleted
    } else {
        FileChangeType::Unknown
    }
}

/// Check if a status should be tracked as an addition
fn should_track_as_added(status: Status) -> bool {
    status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW)
}

/// Check if a status should be tracked as a modification
fn should_track_as_modified(status: Status) -> bool {
    status.contains(Status::WT_MODIFIED) || status.contains(Status::INDEX_MODIFIED)
}

/// Check if a status should be tracked as a deletion
fn should_track_as_deleted(status: Status) -> bool {
    status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED)
}

/// Classify a git delta status into a file change type
fn classify_delta_status(delta: git2::Delta) -> FileChangeType {
    match delta {
        git2::Delta::Added => FileChangeType::Added,
        git2::Delta::Modified => FileChangeType::Modified,
        git2::Delta::Deleted => FileChangeType::Deleted,
        _ => FileChangeType::Unknown,
    }
}

/// Extract file path from a diff delta
fn extract_file_path(delta: &git2::DiffDelta) -> Option<String> {
    delta
        .new_file()
        .path()
        .map(|p| p.to_string_lossy().to_string())
}

/// Check if a path should be added to the list (i.e., not already present)
fn should_add_to_list(list: &[String], path: &str) -> bool {
    !list.contains(&path.to_string())
}

/// Add a file path to the list if it's not already present
fn add_unique_file(list: &mut Vec<String>, path: String) {
    if should_add_to_list(list, &path) {
        list.push(path);
    }
}

/// Sort and deduplicate a file list
fn normalize_file_list(list: &mut Vec<String>) {
    list.sort();
    list.dedup();
}

/// Normalize all file lists in step changes (sort and deduplicate)
fn normalize_file_lists(changes: &mut StepChanges) {
    normalize_file_list(&mut changes.files_added);
    normalize_file_list(&mut changes.files_modified);
    normalize_file_list(&mut changes.files_deleted);
}

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

            match classify_file_status(status) {
                FileChangeType::Added => changes.files_added.push(path.to_string()),
                FileChangeType::Modified => changes.files_modified.push(path.to_string()),
                FileChangeType::Deleted => changes.files_deleted.push(path.to_string()),
                FileChangeType::Unknown => {}
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
                                if let Some(path_str) = extract_file_path(&delta) {
                                    match classify_delta_status(delta.status()) {
                                        FileChangeType::Added => {
                                            add_unique_file(&mut changes.files_added, path_str)
                                        }
                                        FileChangeType::Modified => {
                                            add_unique_file(&mut changes.files_modified, path_str)
                                        }
                                        FileChangeType::Deleted => {
                                            add_unique_file(&mut changes.files_deleted, path_str)
                                        }
                                        FileChangeType::Unknown => {}
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
        normalize_file_lists(&mut changes);

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

    // Phase 1 Tests: Uncommitted Changes Detection

    #[test]
    fn test_calculate_step_changes_with_new_file() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        // Create a new file
        std::fs::write(dir.path().join("new_file.txt"), "content")?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"new_file.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_modified_file() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("existing.txt"), "original content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("existing.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add existing file",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Modify the file
        std::fs::write(dir.path().join("existing.txt"), "modified content")?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_modified.contains(&"existing.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_deleted_file() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to delete",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Delete the file
        std::fs::remove_file(dir.path().join("to_delete.txt"))?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_deleted.contains(&"to_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_new_file() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        // Create and stage a new file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("staged_new.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("staged_new.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"staged_new.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_modification() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "original")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to modify",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Modify and stage the file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "modified")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_modified
            .contains(&"to_modify.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_deletion() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("staged_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("staged_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file for staged deletion",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Delete and stage the deletion
        {
            let repo = Repository::open(dir.path())?;
            std::fs::remove_file(dir.path().join("staged_delete.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("staged_delete.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_deleted
            .contains(&"staged_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_mixed_changes() -> Result<()> {
        let dir = init_test_repo()?;

        // Setup: create and commit initial files
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("existing1.txt"), "content1")?;
            std::fs::write(dir.path().join("existing2.txt"), "content2")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("existing1.txt"))?;
            index.add_path(Path::new("existing2.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add initial files",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Create mixed changes: new file, modified file (unstaged), deleted file (staged)
        {
            let repo = Repository::open(dir.path())?;

            // New file (not staged)
            std::fs::write(dir.path().join("new.txt"), "new content")?;

            // Modified file (not staged)
            std::fs::write(dir.path().join("existing1.txt"), "modified content")?;

            // Deleted file (staged)
            std::fs::remove_file(dir.path().join("existing2.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("existing2.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"new.txt".to_string()));
        assert!(changes
            .files_modified
            .contains(&"existing1.txt".to_string()));
        assert!(changes.files_deleted.contains(&"existing2.txt".to_string()));

        Ok(())
    }

    // Phase 2 Tests: Commit History Walking

    #[test]
    fn test_calculate_step_changes_with_new_commit() -> Result<()> {
        let dir = init_test_repo()?;

        // Create initial tracker to capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a new commit
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("new_commit.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("new_commit.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "New commit for test",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit to simulate previous step
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 1);
        assert!(changes.files_added.contains(&"new_commit.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_multiple_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create multiple commits
        {
            let repo = Repository::open(dir.path())?;
            let sig = git2::Signature::now("Test", "test@example.com")?;

            // First commit
            std::fs::write(dir.path().join("file1.txt"), "content1")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file1.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "First commit", &tree, &[&parent])?;

            // Second commit
            std::fs::write(dir.path().join("file2.txt"), "content2")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file2.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])?;

            // Third commit
            std::fs::write(dir.path().join("file3.txt"), "content3")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file3.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Third commit", &tree, &[&parent])?;
        }

        // Update tracker's last_commit to simulate previous step
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 3);
        assert!(changes.files_added.contains(&"file1.txt".to_string()));
        assert!(changes.files_added.contains(&"file2.txt".to_string()));
        assert!(changes.files_added.contains(&"file3.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_commit_stats() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit with known insertions
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(
                dir.path().join("stats_test.txt"),
                "line1\nline2\nline3\nline4\nline5\n",
            )?;
            let mut index = repo.index()?;
            index.add_path(Path::new("stats_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Commit with stats",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.insertions > 0);
        assert_eq!(changes.deletions, 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_no_new_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Create tracker and immediately check again (no new commits)
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let current_commit = tracker.workflow_start_commit.clone();
        tracker.last_commit = current_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 0);
        assert_eq!(changes.insertions, 0);
        assert_eq!(changes.deletions, 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_commit_shas() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit
        let new_commit_sha = {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("sha_test.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("sha_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            let commit_oid = repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "SHA tracking test",
                &tree,
                &[&parent],
            )?;
            commit_oid.to_string()
        };

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 1);
        assert_eq!(changes.commits[0], new_commit_sha);

        Ok(())
    }

    // Phase 3 Tests: Diff Statistics and File Changes

    #[test]
    fn test_calculate_step_changes_tracks_added_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit that adds files
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("added1.txt"), "content")?;
            std::fs::write(dir.path().join("added2.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("added1.txt"))?;
            index.add_path(Path::new("added2.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Add files", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"added1.txt".to_string()));
        assert!(changes.files_added.contains(&"added2.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_modified_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create and commit a file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "original")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Initial file", &tree, &[&parent])?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now modify and commit the file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "modified content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Modify file", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_modified
            .contains(&"to_modify.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_deleted_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create and commit a file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to delete",
                &tree,
                &[&parent],
            )?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now delete and commit
        {
            let repo = Repository::open(dir.path())?;
            std::fs::remove_file(dir.path().join("to_delete.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Delete file", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_deleted.contains(&"to_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_deduplicates_files() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a file, stage it (appears in index), then commit it
        // This will cause the file to appear in both uncommitted and committed changes
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("dup_test.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("dup_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Commit for dedup test",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        // File should appear only once in files_added (deduplication working)
        let count = changes
            .files_added
            .iter()
            .filter(|f| *f == "dup_test.txt")
            .count();
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_sorts_file_lists() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create files in non-alphabetical order
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("zebra.txt"), "z")?;
            std::fs::write(dir.path().join("apple.txt"), "a")?;
            std::fs::write(dir.path().join("middle.txt"), "m")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("zebra.txt"))?;
            index.add_path(Path::new("apple.txt"))?;
            index.add_path(Path::new("middle.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add unsorted files",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        // Verify files are sorted
        let mut sorted = changes.files_added.clone();
        sorted.sort();
        assert_eq!(changes.files_added, sorted);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_calculates_insertions_deletions() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create a file with some content
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("changes.txt"), "line1\nline2\nline3\n")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("changes.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Initial content",
                &tree,
                &[&parent],
            )?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now modify the file: remove 1 line, add 2 lines
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(
                dir.path().join("changes.txt"),
                "line2\nline3\nnew line 1\nnew line 2\n",
            )?;
            let mut index = repo.index()?;
            index.add_path(Path::new("changes.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Modify content",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.insertions > 0);
        assert!(changes.deletions > 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_handles_mixed_commit_and_uncommitted() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a committed file and an uncommitted file
        {
            let repo = Repository::open(dir.path())?;

            // Committed file
            std::fs::write(dir.path().join("committed.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("committed.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add committed file",
                &tree,
                &[&parent],
            )?;

            // Uncommitted file
            std::fs::write(dir.path().join("uncommitted.txt"), "content")?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"committed.txt".to_string()));
        assert!(changes.files_added.contains(&"uncommitted.txt".to_string()));
        assert_eq!(changes.commits.len(), 1);

        Ok(())
    }

    // Phase 4 Tests: Pure Function Tests for Status Detection

    #[test]
    fn test_should_track_as_added_with_wt_new() {
        assert!(should_track_as_added(Status::WT_NEW));
    }

    #[test]
    fn test_should_track_as_added_with_index_new() {
        assert!(should_track_as_added(Status::INDEX_NEW));
    }

    #[test]
    fn test_should_track_as_added_with_combined_new() {
        let status = Status::WT_NEW | Status::INDEX_NEW;
        assert!(should_track_as_added(status));
    }

    #[test]
    fn test_should_track_as_modified_with_wt_modified() {
        assert!(should_track_as_modified(Status::WT_MODIFIED));
    }

    #[test]
    fn test_should_track_as_modified_with_index_modified() {
        assert!(should_track_as_modified(Status::INDEX_MODIFIED));
    }

    #[test]
    fn test_should_track_as_modified_with_combined_modified() {
        let status = Status::WT_MODIFIED | Status::INDEX_MODIFIED;
        assert!(should_track_as_modified(status));
    }

    #[test]
    fn test_should_track_as_deleted_with_wt_deleted() {
        assert!(should_track_as_deleted(Status::WT_DELETED));
    }

    #[test]
    fn test_should_track_as_deleted_with_index_deleted() {
        assert!(should_track_as_deleted(Status::INDEX_DELETED));
    }

    #[test]
    fn test_should_track_as_deleted_with_combined_deleted() {
        let status = Status::WT_DELETED | Status::INDEX_DELETED;
        assert!(should_track_as_deleted(status));
    }

    #[test]
    fn test_classify_file_status_added() {
        assert_eq!(classify_file_status(Status::WT_NEW), FileChangeType::Added);
        assert_eq!(
            classify_file_status(Status::INDEX_NEW),
            FileChangeType::Added
        );
    }

    #[test]
    fn test_classify_file_status_modified() {
        assert_eq!(
            classify_file_status(Status::WT_MODIFIED),
            FileChangeType::Modified
        );
        assert_eq!(
            classify_file_status(Status::INDEX_MODIFIED),
            FileChangeType::Modified
        );
    }

    #[test]
    fn test_classify_file_status_deleted() {
        assert_eq!(
            classify_file_status(Status::WT_DELETED),
            FileChangeType::Deleted
        );
        assert_eq!(
            classify_file_status(Status::INDEX_DELETED),
            FileChangeType::Deleted
        );
    }

    // Phase 5 Tests: Pure Function Tests for Diff Processing

    #[test]
    fn test_classify_delta_status_added() {
        assert_eq!(
            classify_delta_status(git2::Delta::Added),
            FileChangeType::Added
        );
    }

    #[test]
    fn test_classify_delta_status_modified() {
        assert_eq!(
            classify_delta_status(git2::Delta::Modified),
            FileChangeType::Modified
        );
    }

    #[test]
    fn test_classify_delta_status_deleted() {
        assert_eq!(
            classify_delta_status(git2::Delta::Deleted),
            FileChangeType::Deleted
        );
    }

    #[test]
    fn test_classify_delta_status_unknown() {
        assert_eq!(
            classify_delta_status(git2::Delta::Unmodified),
            FileChangeType::Unknown
        );
        assert_eq!(
            classify_delta_status(git2::Delta::Renamed),
            FileChangeType::Unknown
        );
        assert_eq!(
            classify_delta_status(git2::Delta::Copied),
            FileChangeType::Unknown
        );
    }

    #[test]
    fn test_should_add_to_list_empty() {
        let list: Vec<String> = vec![];
        assert!(should_add_to_list(&list, "test.txt"));
    }

    #[test]
    fn test_should_add_to_list_not_present() {
        let list = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        assert!(should_add_to_list(&list, "file3.txt"));
    }

    #[test]
    fn test_should_add_to_list_already_present() {
        let list = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        assert!(!should_add_to_list(&list, "file1.txt"));
    }

    #[test]
    fn test_add_unique_file_to_empty_list() {
        let mut list = vec![];
        add_unique_file(&mut list, "test.txt".to_string());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], "test.txt");
    }

    #[test]
    fn test_add_unique_file_new_file() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file2.txt".to_string());
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_add_unique_file_duplicate() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file1.txt".to_string());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_add_unique_file_multiple_duplicates() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file1.txt".to_string());
        add_unique_file(&mut list, "file2.txt".to_string());
        add_unique_file(&mut list, "file1.txt".to_string());
        assert_eq!(list.len(), 2);
    }

    // Phase 6 Tests: Pure Function Tests for Normalization

    #[test]
    fn test_normalize_file_list_empty() {
        let mut list: Vec<String> = vec![];
        normalize_file_list(&mut list);
        assert!(list.is_empty());
    }

    #[test]
    fn test_normalize_file_list_sorts() {
        let mut list = vec![
            "zebra.txt".to_string(),
            "apple.txt".to_string(),
            "middle.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list[0], "apple.txt");
        assert_eq!(list[1], "middle.txt");
        assert_eq!(list[2], "zebra.txt");
    }

    #[test]
    fn test_normalize_file_list_deduplicates() {
        let mut list = vec![
            "file1.txt".to_string(),
            "file2.txt".to_string(),
            "file1.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"file1.txt".to_string()));
        assert!(list.contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_normalize_file_list_sorts_and_deduplicates() {
        let mut list = vec![
            "zebra.txt".to_string(),
            "apple.txt".to_string(),
            "zebra.txt".to_string(),
            "middle.txt".to_string(),
            "apple.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "apple.txt");
        assert_eq!(list[1], "middle.txt");
        assert_eq!(list[2], "zebra.txt");
    }

    #[test]
    fn test_normalize_file_lists_normalizes_all() {
        let mut changes = StepChanges {
            files_added: vec![
                "z.txt".to_string(),
                "a.txt".to_string(),
                "a.txt".to_string(),
            ],
            files_modified: vec![
                "y.txt".to_string(),
                "b.txt".to_string(),
                "b.txt".to_string(),
            ],
            files_deleted: vec![
                "x.txt".to_string(),
                "c.txt".to_string(),
                "c.txt".to_string(),
            ],
            ..Default::default()
        };

        normalize_file_lists(&mut changes);

        // Check all lists are sorted
        assert_eq!(changes.files_added[0], "a.txt");
        assert_eq!(changes.files_added[1], "z.txt");
        assert_eq!(changes.files_modified[0], "b.txt");
        assert_eq!(changes.files_modified[1], "y.txt");
        assert_eq!(changes.files_deleted[0], "c.txt");
        assert_eq!(changes.files_deleted[1], "x.txt");

        // Check all lists are deduplicated
        assert_eq!(changes.files_added.len(), 2);
        assert_eq!(changes.files_modified.len(), 2);
        assert_eq!(changes.files_deleted.len(), 2);
    }

    #[test]
    fn test_normalize_file_lists_handles_empty() {
        let mut changes = StepChanges::default();
        normalize_file_lists(&mut changes);
        assert!(changes.files_added.is_empty());
        assert!(changes.files_modified.is_empty());
        assert!(changes.files_deleted.is_empty());
    }
}
