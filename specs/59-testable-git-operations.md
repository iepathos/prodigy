# Specification 59: Testable Git Operations Layer

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [57-subprocess-abstraction-layer]

## Context

While MMM has a `GitOperations` trait, the current implementation has several testability issues:

- The trait is too coarse-grained, making it hard to mock specific operations
- Mock implementation requires extensive setup for each test
- No support for simulating complex git scenarios (merge conflicts, rebase issues)
- Direct filesystem operations mixed with git commands
- Difficult to test error conditions and edge cases

A more granular, testable git abstraction would significantly improve test coverage and reliability.

## Objective

Create a fine-grained, highly testable git operations layer that enables comprehensive testing of all git-related functionality with minimal test setup.

## Requirements

### Functional Requirements
- Support all existing git operations
- Enable easy mocking of specific git behaviors
- Support simulation of git error conditions
- Provide high-level operations built on low-level primitives
- Support both porcelain and plumbing commands
- Enable testing of complex git scenarios

### Non-Functional Requirements
- Minimal overhead for production use
- Type-safe git command construction
- Comprehensive error types for all git failures
- Support for concurrent git operations
- Clear separation between read and write operations

## Acceptance Criteria

- [ ] All git operations have dedicated trait methods
- [ ] Mock implementation supports scenario-based testing
- [ ] 100% test coverage for git operations layer
- [ ] Support for simulating all common git errors
- [ ] Type-safe command construction prevents errors
- [ ] Performance overhead less than 1%
- [ ] All existing git functionality preserved

## Technical Details

### Implementation Approach

1. **Granular Git Traits**
   ```rust
   // Read operations
   #[async_trait]
   pub trait GitReader: Send + Sync {
       async fn is_repository(&self, path: &Path) -> Result<bool>;
       async fn get_status(&self, path: &Path) -> Result<GitStatus>;
       async fn get_current_branch(&self, path: &Path) -> Result<String>;
       async fn get_commit_message(&self, path: &Path, ref_: &str) -> Result<String>;
       async fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>>;
       async fn get_diff(&self, path: &Path, from: &str, to: &str) -> Result<GitDiff>;
   }

   // Write operations
   #[async_trait]
   pub trait GitWriter: Send + Sync {
       async fn init_repository(&self, path: &Path) -> Result<()>;
       async fn stage_files(&self, path: &Path, files: &[PathBuf]) -> Result<()>;
       async fn stage_all(&self, path: &Path) -> Result<()>;
       async fn commit(&self, path: &Path, message: &str) -> Result<CommitId>;
       async fn create_branch(&self, path: &Path, name: &str) -> Result<()>;
       async fn switch_branch(&self, path: &Path, name: &str) -> Result<()>;
   }

   // Worktree operations
   #[async_trait]
   pub trait GitWorktree: Send + Sync {
       async fn create_worktree(&self, repo: &Path, name: &str, path: &Path) -> Result<()>;
       async fn remove_worktree(&self, repo: &Path, name: &str) -> Result<()>;
       async fn list_worktrees(&self, repo: &Path) -> Result<Vec<WorktreeInfo>>;
   }

   // Combined trait for convenience
   pub trait GitOperations: GitReader + GitWriter + GitWorktree {}
   ```

2. **Structured Git Data**
   ```rust
   #[derive(Debug, Clone)]
   pub struct GitStatus {
       pub modified: Vec<PathBuf>,
       pub added: Vec<PathBuf>,
       pub deleted: Vec<PathBuf>,
       pub untracked: Vec<PathBuf>,
       pub conflicts: Vec<PathBuf>,
   }

   #[derive(Debug, Clone)]
   pub struct GitDiff {
       pub files_changed: Vec<FileDiff>,
       pub insertions: usize,
       pub deletions: usize,
   }

   #[derive(Debug, Clone)]
   pub struct WorktreeInfo {
       pub name: String,
       pub path: PathBuf,
       pub branch: String,
       pub commit: CommitId,
   }
   ```

3. **Scenario-Based Mock**
   ```rust
   pub struct GitScenarioMock {
       scenarios: HashMap<PathBuf, GitScenario>,
   }

   pub struct GitScenario {
       pub initial_state: GitRepoState,
       pub responses: HashMap<String, ScenarioResponse>,
   }

   impl GitScenarioMock {
       pub fn new() -> Self { /* ... */ }
       
       // Predefined scenarios
       pub fn with_clean_repo(mut self, path: &Path) -> Self { /* ... */ }
       pub fn with_dirty_repo(mut self, path: &Path) -> Self { /* ... */ }
       pub fn with_merge_conflict(mut self, path: &Path) -> Self { /* ... */ }
       pub fn with_detached_head(mut self, path: &Path) -> Self { /* ... */ }
       
       // Custom scenarios
       pub fn when_command(&mut self, cmd: &str) -> &mut ScenarioResponse { /* ... */ }
   }
   ```

### Architecture Changes

1. **Production Implementation**
   ```rust
   pub struct GitCommandRunner {
       process_runner: Box<dyn ProcessRunner>,
   }

   #[async_trait]
   impl GitReader for GitCommandRunner {
       async fn get_status(&self, path: &Path) -> Result<GitStatus> {
           let output = self.process_runner.run(
               ProcessCommandBuilder::new("git")
                   .args(&["status", "--porcelain=v2"])
                   .current_dir(path)
                   .build()
           ).await?;
           
           parse_status_output(&output.stdout)
       }
   }
   ```

2. **Parser Module**
   ```rust
   mod parsers {
       pub fn parse_status_output(output: &str) -> Result<GitStatus> { /* ... */ }
       pub fn parse_diff_output(output: &str) -> Result<GitDiff> { /* ... */ }
       pub fn parse_worktree_list(output: &str) -> Result<Vec<WorktreeInfo>> { /* ... */ }
   }
   ```

3. **Error Handling**
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum GitError {
       #[error("Not a git repository")]
       NotARepository,
       
       #[error("Branch not found: {0}")]
       BranchNotFound(String),
       
       #[error("Merge conflict in files: {files:?}")]
       MergeConflict { files: Vec<PathBuf> },
       
       #[error("Uncommitted changes present")]
       UncommittedChanges,
       
       #[error("Worktree already exists: {0}")]
       WorktreeExists(String),
       
       #[error("Git command failed: {0}")]
       CommandFailed(String),
   }
   ```

### Data Structures

1. **Test Helpers**
   ```rust
   pub struct GitTestBuilder {
       temp_dir: TempDir,
       repo_path: PathBuf,
   }

   impl GitTestBuilder {
       pub fn new() -> Result<Self> { /* ... */ }
       pub fn init_repo(self) -> Result<Self> { /* ... */ }
       pub fn add_file(self, path: &str, content: &str) -> Result<Self> { /* ... */ }
       pub fn commit(self, message: &str) -> Result<Self> { /* ... */ }
       pub fn create_branch(self, name: &str) -> Result<Self> { /* ... */ }
       pub fn build(self) -> Result<GitTestRepo> { /* ... */ }
   }
   ```

2. **Operation Batching**
   ```rust
   pub struct GitBatch {
       operations: Vec<GitOperation>,
   }

   impl GitBatch {
       pub fn new() -> Self { /* ... */ }
       pub fn stage_all(mut self) -> Self { /* ... */ }
       pub fn commit(mut self, message: &str) -> Self { /* ... */ }
       pub async fn execute<G: GitOperations>(&self, git: &G, path: &Path) -> Result<()> { /* ... */ }
   }
   ```

## Dependencies

- **Prerequisites**: [57-subprocess-abstraction-layer]
- **Affected Components**: 
  - All git-dependent operations
  - Worktree management
  - Cook module git interactions
- **External Dependencies**: None new

## Testing Strategy

- **Unit Tests**: 
  - Test all parsers with various outputs
  - Test error conditions
  - Test mock scenarios
  - Test operation batching
- **Integration Tests**: 
  - Test with real git repositories
  - Test error recovery
  - Test concurrent operations
- **Scenario Tests**: 
  - Test complex workflows
  - Test conflict resolution
  - Test worktree operations

## Documentation Requirements

- **Code Documentation**: 
  - Document all trait methods
  - Provide examples for common scenarios
  - Document parser formats
- **Testing Guide**: 
  - How to use scenario mocks
  - Common test patterns
  - Test data builders
- **Migration Guide**: 
  - Updating existing git operations
  - New error handling patterns

## Implementation Notes

1. **Parser Robustness**
   - Handle different git versions
   - Support various output formats
   - Graceful handling of malformed output
   - Comprehensive error messages

2. **Performance Optimization**
   - Batch operations where possible
   - Cache repository state when safe
   - Minimize subprocess calls
   - Use git plumbing commands

3. **Safety Considerations**
   - Validate all inputs
   - Prevent destructive operations in tests
   - Clear error messages for failures
   - Support dry-run mode

## Migration and Compatibility

1. **Incremental Migration**
   - Implement new traits alongside existing
   - Gradually migrate operations
   - Maintain backward compatibility
   - Support both old and new APIs temporarily

2. **Testing Strategy During Migration**
   - Run tests with both implementations
   - Compare outputs for consistency
   - Performance benchmarks
   - Gradual rollout

3. **Deprecation Plan**
   - Mark old APIs as deprecated
   - Provide migration guide
   - Set removal timeline
   - Support transition period