---
number: 114
title: MapReduce Git Operations Implementation
category: functionality
priority: important
status: draft
dependencies: [109]
created: 2025-09-27
---

# Specification 114: MapReduce Git Operations Implementation

## Context

The current MapReduce implementation includes stubbed git operations `get_worktree_commits()` and `get_worktree_modified_files()` that are not implemented. These operations are essential for tracking changes made by agents, integrating with the merge workflow system, and providing audit trails for MapReduce job results.

Current gaps:
- `get_worktree_commits()` function exists but returns empty results
- `get_worktree_modified_files()` function is stubbed and non-functional
- No git integration for tracking agent changes
- Missing commit metadata collection for map phase results
- No file change tracking for agent outputs
- Incomplete integration with merge workflow variables

Git operations are crucial for:
- Tracking what changes each agent made to the codebase
- Providing detailed merge information for worktree integration
- Creating audit trails for MapReduce job modifications
- Supporting rollback and change analysis capabilities

## Objective

Implement comprehensive git operations for MapReduce workflows that enable tracking of agent changes, provide detailed commit and file modification information, and integrate seamlessly with the merge workflow system.

## Requirements

### Functional Requirements

#### Commit Tracking
- Implement `get_worktree_commits()` to return actual commit information
- Track commits made by individual agents during execution
- Collect commit metadata including author, timestamp, and message
- Support filtering commits by time range and agent
- Provide commit diff information and statistics

#### File Modification Tracking
- Implement `get_worktree_modified_files()` to return actual file changes
- Track files modified, added, and deleted by agents
- Provide file-level diff statistics and content changes
- Support filtering by file type, path patterns, and modification type
- Track file modification timestamps and agent attribution

#### Git State Management
- Maintain git repository state across agent executions
- Handle git conflicts and merge resolution
- Support git hooks and validation during agent execution
- Provide git repository health checks and validation
- Track git repository statistics and metrics

#### Integration with Merge Workflows
- Populate merge workflow variables with git information
- Support `${merge.commits}` and `${merge.modified_files}` variables
- Provide structured git data for merge decision making
- Enable git-based conditional logic in merge workflows
- Support git metadata in checkpoint and resume functionality

### Non-Functional Requirements
- Git operations should complete within 5 seconds for typical repositories
- Support for large repositories with thousands of commits/files
- Efficient caching of git operations to avoid repeated queries
- Thread-safe git operations for concurrent agent execution
- Minimal memory footprint for git data structures

## Acceptance Criteria

- [ ] `get_worktree_commits()` returns actual commit information with metadata
- [ ] `get_worktree_modified_files()` returns comprehensive file change information
- [ ] Git operations work correctly with MapReduce agent worktrees
- [ ] Merge workflow variables are populated with git data
- [ ] Git integration preserves repository integrity during agent execution
- [ ] Performance benchmarks show acceptable git operation latency
- [ ] Git error handling provides clear feedback for repository issues
- [ ] Git operations integrate with existing worktree cleanup mechanisms

## Technical Details

### Implementation Approach

#### 1. Git Operations Service

Create a comprehensive git operations service:

```rust
use git2::{Repository, Commit, Diff, DiffOptions, DiffFormat};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

pub struct GitOperationsService {
    repository_cache: HashMap<PathBuf, Repository>,
    config: GitOperationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationsConfig {
    /// Enable caching of git operations
    pub enable_caching: bool,

    /// Maximum age for cached git data
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub name: String,
    pub email: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModificationType {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
    Copied { from: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_context: usize,
}

impl GitOperationsService {
    pub fn new(config: GitOperationsConfig) -> Self {
        Self {
            repository_cache: HashMap::new(),
            config,
        }
    }

    /// Get commits from a worktree repository
    pub async fn get_worktree_commits(
        &mut self,
        worktree_path: &Path,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
    ) -> Result<Vec<CommitInfo>, GitError> {
        let repo = self.get_repository(worktree_path)?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        let mut count = 0;

        for oid in revwalk {
            if count >= self.config.max_commits {
                break;
            }

            let oid = oid?;
            let commit = repo.find_commit(oid)?;

            let commit_time = DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_else(|| Utc::now());

            // Apply time filters
            if let Some(since) = since {
                if commit_time < since {
                    break; // Commits are sorted by time, so we can stop here
                }
            }

            if let Some(until) = until {
                if commit_time > until {
                    continue;
                }
            }

            let commit_info = self.build_commit_info(&repo, &commit).await?;
            commits.push(commit_info);
            count += 1;
        }

        Ok(commits)
    }

    /// Get modified files from a worktree repository
    pub async fn get_worktree_modified_files(
        &mut self,
        worktree_path: &Path,
        since_commit: Option<&str>,
    ) -> Result<Vec<FileModificationInfo>, GitError> {
        let repo = self.get_repository(worktree_path)?;

        let mut modifications = Vec::new();

        // Get working directory changes
        let workdir_changes = self.get_working_directory_changes(&repo).await?;
        modifications.extend(workdir_changes);

        // Get committed changes since specified commit
        if let Some(since) = since_commit {
            let committed_changes = self.get_committed_changes(&repo, since).await?;
            modifications.extend(committed_changes);
        }

        // Deduplicate and sort
        self.deduplicate_modifications(&mut modifications);

        // Apply file limit
        if modifications.len() > self.config.max_files {
            modifications.truncate(self.config.max_files);
        }

        Ok(modifications)
    }

    async fn build_commit_info(&self, repo: &Repository, commit: &Commit) -> Result<CommitInfo, GitError> {
        let id = commit.id().to_string();
        let short_id = format!("{:.7}", id);

        let author = commit.author();
        let committer = commit.committer();

        let author_info = AuthorInfo {
            name: author.name().unwrap_or("Unknown").to_string(),
            email: author.email().unwrap_or("unknown@example.com").to_string(),
            timestamp: DateTime::from_timestamp(author.when().seconds(), 0)
                .unwrap_or_else(|| Utc::now()),
        };

        let committer_info = AuthorInfo {
            name: committer.name().unwrap_or("Unknown").to_string(),
            email: committer.email().unwrap_or("unknown@example.com").to_string(),
            timestamp: DateTime::from_timestamp(committer.when().seconds(), 0)
                .unwrap_or_else(|| Utc::now()),
        };

        let message = commit.message().unwrap_or("").to_string();
        let tree_id = commit.tree_id().to_string();

        let parent_ids: Vec<String> = commit.parent_ids()
            .map(|id| id.to_string())
            .collect();

        // Calculate commit statistics
        let stats = self.calculate_commit_stats(repo, commit).await?;
        let files_changed = self.get_commit_files(repo, commit).await?;

        Ok(CommitInfo {
            id,
            short_id,
            author: author_info,
            committer: committer_info,
            message,
            timestamp: DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_else(|| Utc::now()),
            parent_ids,
            tree_id,
            stats: Some(stats),
            files_changed,
        })
    }

    async fn calculate_commit_stats(&self, repo: &Repository, commit: &Commit) -> Result<CommitStats, GitError> {
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;

        let stats = diff.stats()?;

        Ok(CommitStats {
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
        })
    }

    async fn get_commit_files(&self, repo: &Repository, commit: &Commit) -> Result<Vec<String>, GitError> {
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;

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
        )?;

        Ok(files)
    }

    async fn get_working_directory_changes(&self, repo: &Repository) -> Result<Vec<FileModificationInfo>, GitError> {
        let mut index = repo.index()?;
        let tree = repo.head()?.peel_to_tree()?;

        let mut diff_opts = DiffOptions::new();
        diff_opts.include_untracked(true);

        let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), Some(&mut diff_opts))?;

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
        )?;

        Ok(modifications)
    }

    async fn get_committed_changes(&self, repo: &Repository, since_commit: &str) -> Result<Vec<FileModificationInfo>, GitError> {
        let since_oid = git2::Oid::from_str(since_commit)?;
        let since_commit = repo.find_commit(since_oid)?;
        let head_commit = repo.head()?.peel_to_commit()?;

        let since_tree = since_commit.tree()?;
        let head_tree = head_commit.tree()?;

        let mut diff_opts = DiffOptions::new();
        let diff = repo.diff_tree_to_tree(Some(&since_tree), Some(&head_tree), Some(&mut diff_opts))?;

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
        )?;

        Ok(modifications)
    }

    fn process_diff_delta(&self, delta: &git2::DiffDelta) -> Option<FileModificationInfo> {
        let new_file = delta.new_file();
        let old_file = delta.old_file();

        let path = new_file.path().or_else(|| old_file.path())?;

        let modification_type = match delta.status() {
            git2::Delta::Added => ModificationType::Added,
            git2::Delta::Deleted => ModificationType::Deleted,
            git2::Delta::Modified => ModificationType::Modified,
            git2::Delta::Renamed => {
                if let Some(old_path) = old_file.path() {
                    ModificationType::Renamed { from: old_path.to_path_buf() }
                } else {
                    ModificationType::Modified
                }
            }
            git2::Delta::Copied => {
                if let Some(old_path) = old_file.path() {
                    ModificationType::Copied { from: old_path.to_path_buf() }
                } else {
                    ModificationType::Modified
                }
            }
            _ => return None,
        };

        Some(FileModificationInfo {
            path: path.to_path_buf(),
            modification_type,
            size_before: if old_file.size() > 0 { Some(old_file.size()) } else { None },
            size_after: if new_file.size() > 0 { Some(new_file.size()) } else { None },
            last_modified: Utc::now(), // Would need filesystem metadata for accurate timestamp
            commit_id: None, // Would need to track this separately
            diff_stats: None, // Could calculate from diff if needed
            content_diff: None, // Would generate if config.include_diffs is true
        })
    }

    fn deduplicate_modifications(&self, modifications: &mut Vec<FileModificationInfo>) {
        // Sort by path and keep the most recent modification for each file
        modifications.sort_by(|a, b| {
            a.path.cmp(&b.path)
                .then_with(|| b.last_modified.cmp(&a.last_modified))
        });

        modifications.dedup_by(|a, b| a.path == b.path);
    }

    fn get_repository(&mut self, path: &Path) -> Result<&Repository, GitError> {
        let canonical_path = path.canonicalize()
            .map_err(|e| GitError::InvalidPath(path.to_path_buf(), e))?;

        if !self.repository_cache.contains_key(&canonical_path) {
            let repo = Repository::open(&canonical_path)
                .map_err(|e| GitError::RepositoryOpen(canonical_path.clone(), e))?;
            self.repository_cache.insert(canonical_path.clone(), repo);
        }

        Ok(self.repository_cache.get(&canonical_path).unwrap())
    }

    /// Get git information for merge workflow variables
    pub async fn get_merge_git_info(
        &mut self,
        worktree_path: &Path,
        target_branch: &str,
    ) -> Result<MergeGitInfo, GitError> {
        let commits = self.get_worktree_commits(worktree_path, None, None).await?;
        let modified_files = self.get_worktree_modified_files(worktree_path, None).await?;

        Ok(MergeGitInfo {
            commits,
            modified_files,
            target_branch: target_branch.to_string(),
            worktree_path: worktree_path.to_path_buf(),
            generated_at: Utc::now(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeGitInfo {
    pub commits: Vec<CommitInfo>,
    pub modified_files: Vec<FileModificationInfo>,
    pub target_branch: String,
    pub worktree_path: PathBuf,
    pub generated_at: DateTime<Utc>,
}
```

#### 2. Error Handling

Comprehensive error handling for git operations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git repository error: {0}")]
    Git(#[from] git2::Error),

    #[error("Invalid repository path {0}: {1}")]
    InvalidPath(PathBuf, #[source] std::io::Error),

    #[error("Failed to open repository at {0}: {1}")]
    RepositoryOpen(PathBuf, #[source] git2::Error),

    #[error("Commit not found: {0}")]
    CommitNotFound(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Git operation timeout after {0:?}")]
    OperationTimeout(Duration),

    #[error("Repository is in invalid state: {0}")]
    InvalidState(String),

    #[error("Permission denied for git operation")]
    PermissionDenied,
}

impl GitError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            GitError::OperationTimeout(_) | GitError::CommitNotFound(_) | GitError::BranchNotFound(_)
        )
    }

    pub fn should_retry(&self) -> bool {
        matches!(self, GitError::OperationTimeout(_))
    }
}
```

#### 3. Integration with MapReduce Phases

Integrate git operations with existing MapReduce infrastructure:

```rust
impl MapPhaseExecutor {
    pub async fn collect_git_results(&self, agent_results: &[AgentResult]) -> Result<GitResults, MapReduceError> {
        let mut git_service = GitOperationsService::new(GitOperationsConfig::default());
        let mut all_commits = Vec::new();
        let mut all_modified_files = Vec::new();

        for agent_result in agent_results {
            if let Some(worktree_path) = &agent_result.worktree_path {
                // Get commits made by this agent
                let agent_commits = git_service
                    .get_worktree_commits(
                        worktree_path,
                        Some(agent_result.started_at),
                        Some(agent_result.completed_at),
                    )
                    .await
                    .unwrap_or_default();

                // Get files modified by this agent
                let agent_files = git_service
                    .get_worktree_modified_files(worktree_path, None)
                    .await
                    .unwrap_or_default();

                all_commits.extend(agent_commits);
                all_modified_files.extend(agent_files);
            }
        }

        Ok(GitResults {
            commits: all_commits,
            modified_files: all_modified_files,
            total_agents: agent_results.len(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitResults {
    pub commits: Vec<CommitInfo>,
    pub modified_files: Vec<FileModificationInfo>,
    pub total_agents: usize,
}
```

#### 4. Merge Workflow Integration

Populate merge workflow variables with git information:

```rust
impl MergeWorkflowExecutor {
    pub async fn prepare_merge_variables(
        &self,
        worktree_path: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<HashMap<String, serde_json::Value>, MergeError> {
        let mut git_service = GitOperationsService::new(GitOperationsConfig::default());

        let merge_git_info = git_service
            .get_merge_git_info(worktree_path, target_branch)
            .await?;

        let mut variables = HashMap::new();

        // Populate standard merge variables
        variables.insert("merge.worktree".to_string(),
            serde_json::Value::String(worktree_path.to_string_lossy().to_string()));
        variables.insert("merge.source_branch".to_string(),
            serde_json::Value::String(source_branch.to_string()));
        variables.insert("merge.target_branch".to_string(),
            serde_json::Value::String(target_branch.to_string()));

        // Add git-specific variables
        variables.insert("merge.commits".to_string(),
            serde_json::to_value(&merge_git_info.commits)?);
        variables.insert("merge.modified_files".to_string(),
            serde_json::to_value(&merge_git_info.modified_files)?);
        variables.insert("merge.commit_count".to_string(),
            serde_json::Value::Number(merge_git_info.commits.len().into()));
        variables.insert("merge.file_count".to_string(),
            serde_json::Value::Number(merge_git_info.modified_files.len().into()));

        // Add summary statistics
        let total_insertions: usize = merge_git_info.commits
            .iter()
            .filter_map(|c| c.stats.as_ref())
            .map(|s| s.insertions)
            .sum();
        let total_deletions: usize = merge_git_info.commits
            .iter()
            .filter_map(|c| c.stats.as_ref())
            .map(|s| s.deletions)
            .sum();

        variables.insert("merge.total_insertions".to_string(),
            serde_json::Value::Number(total_insertions.into()));
        variables.insert("merge.total_deletions".to_string(),
            serde_json::Value::Number(total_deletions.into()));

        Ok(variables)
    }
}
```

### YAML Configuration Example

```yaml
name: git-tracking-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"

  agent_template:
    - claude: "/fix-issue '${item}'"
    - shell: "git add -A"
    - shell: "git commit -m 'Fix ${item.description}'"

reduce:
  - claude: "/summarize-changes ${map.commits} ${map.modified_files}"

merge:
  - shell: "echo 'Merging ${merge.commit_count} commits affecting ${merge.file_count} files'"
  - shell: "echo 'Total changes: +${merge.total_insertions}/-${merge.total_deletions}'"
  - claude: "/review-merge ${merge.commits}"
  - shell: "git merge ${merge.source_branch}"
```

### Performance Optimization

```rust
impl GitOperationsService {
    /// Enable caching for expensive git operations
    pub async fn with_caching(mut self, enable: bool) -> Self {
        self.config.enable_caching = enable;
        self
    }

    /// Batch git operations for efficiency
    pub async fn batch_get_commit_info(
        &mut self,
        worktree_paths: &[PathBuf],
        since: Option<DateTime<Utc>>,
    ) -> Result<HashMap<PathBuf, Vec<CommitInfo>>, GitError> {
        let mut results = HashMap::new();

        // Process repositories in parallel
        let futures: Vec<_> = worktree_paths
            .iter()
            .map(|path| {
                let path = path.clone();
                async move {
                    let mut service = GitOperationsService::new(self.config.clone());
                    let commits = service.get_worktree_commits(&path, since, None).await?;
                    Ok::<_, GitError>((path, commits))
                }
            })
            .collect();

        let batch_results = futures::future::try_join_all(futures).await?;

        for (path, commits) in batch_results {
            results.insert(path, commits);
        }

        Ok(results)
    }
}
```

## Testing Strategy

### Unit Tests
- Test git operations service with various repository states
- Test commit information extraction and formatting
- Test file modification tracking accuracy
- Test error handling for invalid repositories
- Test performance with large repositories

### Integration Tests
- Test git operations integration with MapReduce agents
- Test merge workflow variable population
- Test git operations during checkpoint/resume
- Test git operations with worktree cleanup
- Test concurrent git operations

### Performance Tests
- Benchmark git operations latency vs. repository size
- Test memory usage with large commit histories
- Test concurrent git operation performance
- Test caching effectiveness

### Repository Tests
- Test with various git repository configurations
- Test with repositories having merge conflicts
- Test with repositories having large binary files
- Test with repositories having many branches/tags

## Migration Strategy

### Phase 1: Core Git Operations
1. Implement `GitOperationsService` with basic commit and file tracking
2. Replace stubbed `get_worktree_commits()` and `get_worktree_modified_files()`
3. Add comprehensive error handling and validation

### Phase 2: MapReduce Integration
1. Integrate git operations with map phase result collection
2. Add git data to phase results and context
3. Implement performance optimization and caching

### Phase 3: Merge Workflow Integration
1. Populate merge workflow variables with git information
2. Add git-based conditional logic support
3. Integrate with checkpoint/resume functionality

### Phase 4: Advanced Features
1. Add git repository health monitoring
2. Implement git operation metrics and alerting
3. Add git repository optimization tools

## Documentation Requirements

- Update MapReduce documentation with git integration details
- Document merge workflow variables related to git operations
- Create troubleshooting guide for git operation issues
- Document performance considerations for large repositories
- Add examples demonstrating git-based workflow logic

## Risk Assessment

### High Risk
- **Repository Corruption**: Concurrent git operations might corrupt repository state
- **Performance Impact**: Git operations on large repositories might slow down execution
- **Memory Usage**: Large git datasets might cause memory issues

### Medium Risk
- **Git Conflicts**: Merge conflicts might break automated workflows
- **Repository Access**: Permission issues might prevent git operations
- **Data Accuracy**: Git operation results might not reflect actual changes

### Mitigation Strategies
- Implement repository locking for critical git operations
- Add repository validation and health checks
- Provide configurable limits for git data collection
- Include comprehensive error handling with recovery options
- Add monitoring and alerting for git operation health