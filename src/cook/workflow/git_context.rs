//! Git context tracking for workflows
//!
//! This module provides automatic tracking of git changes during workflow execution,
//! exposing file change information as interpolatable variables.
//!
//! # Module Structure
//!
//! The git context functionality is organized into focused modules:
//!
//! - [`git_context`](self) - Domain logic for change tracking and variable resolution
//! - `git_context_tests` - Comprehensive test suite (24 tests across 3 phases)
//! - `git_utils` - Pure utility functions for file classification and list operations
//!
//! # Architecture
//!
//! The module follows functional programming principles with:
//! - **Pure helper functions** for parsing and formatting (all under 20 lines)
//! - **Separated I/O and logic** - Git operations isolated from business logic
//! - **Function composition** - Complex operations built from simple, testable units
//! - **Immutable data flow** - Changes are calculated, not mutated
//!
//! # Responsibilities
//!
//! This module handles:
//! - Git repository interaction and change detection (committed and uncommitted)
//! - Step-by-step change tracking during workflow execution
//! - Variable resolution for git context (e.g., `${step.files_added}`, `${workflow.commits:json}`)
//! - Format support: space-separated, newline, JSON array, comma-separated
//! - Glob pattern filtering (e.g., `${step.files_added:*.rs}`)
//! - Aggregation of changes across workflow steps
//!
//! # Variable Resolution
//!
//! Variables support multiple formats and patterns:
//!
//! ```text
//! ${step.files_added}          # Space-separated list
//! ${step.files_added:json}     # JSON array format
//! ${step.files_added:*.rs}     # Filter by glob pattern
//! ${workflow.commit_count}     # Scalar values
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use prodigy::cook::workflow::GitChangeTracker;
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let mut tracker = GitChangeTracker::new(Path::new("."))?;
//!
//! // Track changes for a step
//! tracker.begin_step("step1")?;
//! // ... perform operations ...
//! let changes = tracker.complete_step()?;
//!
//! println!("Files added: {:?}", changes.files_added);
//! println!("Files modified: {:?}", changes.files_modified);
//!
//! // Resolve variables with format and pattern support
//! let json_files = tracker.resolve_variable("step.files_added:json")?;
//! let rust_files = tracker.resolve_variable("step.files_added:*.rs")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Test Organization
//!
//! Tests are organized in `git_context_tests.rs` into three phases:
//! - **Phase 1**: Uncommitted changes detection (8 tests)
//! - **Phase 2**: Commit history walking (6 tests)
//! - **Phase 3**: Diff statistics and file changes (10 tests)

use anyhow::{Context, Result};
use git2::{DiffOptions, Oid, Repository, StatusOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use super::git_utils::{
    add_unique_file, classify_delta_status, classify_file_status, extract_file_path,
    normalize_file_lists, FileChangeType,
};

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
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Parse variable format from modifier string
fn parse_variable_format(modifier: Option<&str>) -> VariableFormat {
    match modifier {
        Some("json") => VariableFormat::JsonArray,
        Some("lines") | Some("newline") => VariableFormat::NewlineSeparated,
        Some("csv") | Some("comma") => VariableFormat::CommaSeparated,
        _ => VariableFormat::SpaceSeparated,
    }
}

/// Extract glob pattern from modifier if it contains glob characters
fn extract_glob_pattern(modifier: Option<&str>) -> Option<&str> {
    modifier.filter(|m| m.contains('*') || m.contains('?'))
}

/// Parse variable path into base path and modifier
fn parse_variable_path(var_path: &str) -> Result<(Vec<&str>, Option<&str>)> {
    let parts: Vec<&str> = var_path.split('.').collect();

    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty variable path"));
    }

    // Parse format and pattern from variable path
    // Format: step.files_added:*.md or step.files_added:json
    if let Some(pos) = parts.last().unwrap().find(':') {
        let last = parts.last().unwrap();
        let base = &last[..pos];
        let modifier = &last[pos + 1..];
        let base_path = parts[..parts.len() - 1]
            .iter()
            .chain(&[base])
            .copied()
            .collect::<Vec<_>>();
        Ok((base_path, Some(modifier)))
    } else {
        Ok((parts, None))
    }
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

    /// Collect uncommitted file changes from git status
    fn collect_uncommitted_changes(repo: &Repository) -> Result<StepChanges> {
        let mut changes = StepChanges::default();
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_opts))?;

        for entry in statuses.iter() {
            let path = match entry.path() {
                Some(p) => p,
                None => continue,
            };

            match classify_file_status(entry.status()) {
                FileChangeType::Added => changes.files_added.push(path.to_string()),
                FileChangeType::Modified => changes.files_modified.push(path.to_string()),
                FileChangeType::Deleted => changes.files_deleted.push(path.to_string()),
                FileChangeType::Unknown => {}
            }
        }

        Ok(changes)
    }

    /// Collect commit SHAs between two OIDs
    fn collect_commits_between(repo: &Repository, from_oid: Oid, to_oid: Oid) -> Result<Vec<String>> {
        let mut commits = Vec::new();
        let mut revwalk = repo.revwalk()?;
        revwalk.push(to_oid)?;
        revwalk.hide(from_oid)?;

        for oid in revwalk {
            commits.push(oid?.to_string());
        }

        Ok(commits)
    }

    /// Calculate changes for the current step
    pub(crate) fn calculate_step_changes(&self) -> Result<StepChanges> {
        let repo = Repository::open(&self.repo_path).context("Failed to open git repository")?;

        let current_commit = Self::get_head_commit(&repo)?;

        // Collect uncommitted changes
        let mut changes = Self::collect_uncommitted_changes(&repo)?;

        // If there's a previous commit, calculate committed changes
        if let (Some(last), Some(current)) = (&self.last_commit, &current_commit) {
            if last != current {
                // New commits were made
                let last_oid = Oid::from_str(last)?;
                let current_oid = Oid::from_str(current)?;

                // Collect commits between last and current
                changes.commits = Self::collect_commits_between(&repo, last_oid, current_oid)?;

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
        let (base_path, modifier) = parse_variable_path(var_path)?;
        let format = parse_variable_format(modifier);
        let pattern = extract_glob_pattern(modifier);

        match base_path[..] {
            ["step", var_name] => {
                let changes = self.get_current_step_changes();
                self.resolve_step_variable(&changes, var_name, format, pattern)
            }
            ["workflow", var_name] => {
                let changes = self.get_workflow_changes();
                self.resolve_step_variable(&changes, var_name, format, pattern)
            }
            _ => Err(anyhow::anyhow!("Unknown git variable path: {}", var_path)),
        }
    }

    /// Get changes for the current step
    fn get_current_step_changes(&self) -> StepChanges {
        if let Some(step_id) = &self.current_step_id {
            self.step_changes.get(step_id).cloned().unwrap_or_default()
        } else {
            StepChanges::default()
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
        match var_name {
            "files_added" => {
                Ok(self.resolve_file_list(&changes.files_added, changes, format, pattern))
            }
            "files_modified" => {
                Ok(self.resolve_file_list(&changes.files_modified, changes, format, pattern))
            }
            "files_deleted" => {
                Ok(self.resolve_file_list(&changes.files_deleted, changes, format, pattern))
            }
            "files_changed" => {
                let all = changes.files_changed();
                Ok(self.resolve_file_list(&all, changes, format, pattern))
            }
            "commits" => Ok(Self::format_file_list(&changes.commits, format)),
            "commit_count" => Ok(changes.commit_count().to_string()),
            "insertions" => Ok(changes.insertions.to_string()),
            "deletions" => Ok(changes.deletions.to_string()),
            _ => Err(anyhow::anyhow!("Unknown step variable: {}", var_name)),
        }
    }

    /// Resolve a file list variable with optional pattern filtering
    fn resolve_file_list(
        &self,
        files: &[String],
        changes: &StepChanges,
        format: VariableFormat,
        pattern: Option<&str>,
    ) -> String {
        let filtered = if let Some(p) = pattern {
            changes.filter_files(files, p)
        } else {
            files.to_vec()
        };
        Self::format_file_list(&filtered, format)
    }

    /// Check if tracker is active (in a git repository)
    pub fn is_active(&self) -> bool {
        self.workflow_start_commit.is_some()
    }
}
