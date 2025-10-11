---
number: 127
title: Fix MapReduce Setup Phase Worktree Execution
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-10-11
---

# Specification 127: Fix MapReduce Setup Phase Worktree Execution

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

MapReduce workflows in Prodigy are designed to execute in isolated git worktrees to prevent conflicts and enable parallel processing. However, there is a critical bug where the **setup phase executes commands in the main repository** instead of the isolated worktree.

### Current Behavior (Broken)

```
2025-10-11T15:09:22.957908Z  INFO Setup phase executing in directory:
    /Users/glen/.prodigy/worktrees/prodigy/session-7e1af6a9-6d4b-4494-b2b5-102423b5cc39
2025-10-11T15:09:22.967991Z  INFO Working directory overridden to:
    /Users/glen/memento-mori/prodigy  ← BUG: Switched back to main repo!
```

This causes:
1. **File modifications in the main repo** instead of the worktree
2. **Git commits in the wrong location** (main repo, not worktree)
3. **Workflow failures** when `commit_required=true` expects commits in the worktree
4. **Dirty working directory** in the main repo after workflow execution

### Evidence

Running `git status` in main repo after failed workflow shows:
```
M .prodigy/book-analysis/features.json
```

The file was modified in the **main repo** instead of the worktree, and the commit attempt failed because Prodigy was looking for commits in the worktree.

### Root Cause

The setup phase is being executed with an incorrect working directory override. The code likely has logic that switches to the original repo directory instead of maintaining the worktree context.

## Objective

Ensure all MapReduce workflow phases (setup, map, reduce, merge) execute commands in the appropriate worktree directory, never modifying the main repository.

## Requirements

### Functional Requirements

1. **Setup Phase Execution**
   - Setup phase commands MUST execute in the worktree directory
   - Working directory MUST NOT be overridden to main repo
   - File modifications MUST occur in worktree
   - Git commits MUST be created in worktree

2. **Map Phase Execution**
   - Each map agent MUST execute in its own isolated worktree
   - No cross-contamination between agents
   - No modifications to main repo

3. **Reduce Phase Execution**
   - Reduce phase MUST execute in the main worktree (parent of map agents)
   - Results aggregation happens in worktree
   - No modifications to main repo

4. **Merge Phase Execution**
   - Merge phase MUST execute in the worktree
   - Cleanup operations happen in worktree
   - Final merge to main repo is explicit and controlled

5. **Working Directory Management**
   - Clear logging of which directory commands execute in
   - No implicit directory changes
   - Explicit validation that we're in the expected directory

### Non-Functional Requirements

- **Isolation**: Main repository MUST remain untouched during workflow execution
- **Traceability**: Logs MUST clearly show which directory each command executes in
- **Reliability**: Working directory state MUST be predictable and consistent
- **Debugging**: Easy to identify if working directory is incorrect

## Acceptance Criteria

- [ ] Setup phase commands execute in worktree directory (not main repo)
- [ ] Setup phase git commits are created in worktree (not main repo)
- [ ] Main repository remains clean (no modified files) during setup phase
- [ ] Map phase agents execute in their isolated worktrees
- [ ] Reduce phase executes in parent worktree
- [ ] Merge phase executes in worktree until final merge
- [ ] Logs clearly show: "Executing in worktree: /path/to/worktree"
- [ ] Integration test verifies main repo stays clean during MapReduce
- [ ] Error if setup phase attempts to modify main repo
- [ ] Documentation explains worktree isolation guarantees

## Technical Details

### Implementation Approach

**1. Identify the Bug Location**

Search for code that overrides working directory:
```rust
// Likely in src/cook/mapreduce/ or src/cook/workflow/
// Look for working_dir overrides in setup phase execution
```

**2. Fix Working Directory Logic**

```rust
// BEFORE (broken):
pub async fn execute_setup_phase(
    &self,
    config: &MapReduceConfig,
    worktree_path: &Path,
) -> Result<()> {
    // BUG: This might be switching back to original repo
    let working_dir = self.original_repo_path.clone(); // ← BUG

    for command in &config.setup.commands {
        execute_command(command, &working_dir).await?; // Wrong directory!
    }
}

// AFTER (fixed):
pub async fn execute_setup_phase(
    &self,
    config: &MapReduceConfig,
    worktree_path: &Path,
) -> Result<()> {
    // FIXED: Execute in worktree
    let working_dir = worktree_path.to_path_buf();

    log::info!("Setup phase executing in worktree: {}", working_dir.display());

    for command in &config.setup.commands {
        execute_command(command, &working_dir).await?;
    }
}
```

**3. Add Directory Validation**

```rust
/// Verify we're executing in the expected worktree
fn validate_execution_context(
    expected_worktree: &Path,
    command_name: &str,
) -> Result<()> {
    let current_dir = env::current_dir()?;

    if !current_dir.starts_with(expected_worktree) {
        return Err(anyhow!(
            "Security violation: Command '{}' attempted to execute in '{}' \
             but should be in worktree '{}'",
            command_name,
            current_dir.display(),
            expected_worktree.display()
        ));
    }

    Ok(())
}
```

**4. Enhanced Logging**

```rust
log::info!(
    "Executing command in worktree: {}\n  Command: {}\n  Worktree: {}",
    command_name,
    command_str,
    working_dir.display()
);
```

### Architecture Changes

**Current (Broken) Flow**:
```
Setup Phase Starts
    ↓
Worktree Created ✓
    ↓
Working Dir Override → Main Repo ✗
    ↓
Commands Execute in Main Repo ✗
    ↓
Commits Created in Main Repo ✗
    ↓
Prodigy Checks Worktree for Commits ✗
    ↓
No Commits Found → Failure ✗
```

**Fixed Flow**:
```
Setup Phase Starts
    ↓
Worktree Created ✓
    ↓
Working Dir = Worktree ✓
    ↓
Commands Execute in Worktree ✓
    ↓
Commits Created in Worktree ✓
    ↓
Prodigy Checks Worktree for Commits ✓
    ↓
Commits Found → Success ✓
```

### Root Cause Analysis

Likely causes:
1. **Global working directory state** being restored to main repo
2. **Relative path resolution** defaulting to original repo
3. **Environment variable** (like `PWD` or `CARGO_MANIFEST_DIR`) pointing to main repo
4. **Command execution context** not being passed correctly

### Data Structures

```rust
/// Execution context for workflow commands
pub struct ExecutionContext {
    /// The worktree directory where commands should execute
    pub worktree_path: PathBuf,

    /// The original repository path (for reference only, not execution)
    pub original_repo_path: PathBuf,

    /// Session ID for correlation
    pub session_id: String,

    /// Phase being executed
    pub phase: WorkflowPhase,
}

impl ExecutionContext {
    /// Get the directory where commands should execute
    pub fn execution_directory(&self) -> &Path {
        // ALWAYS return worktree_path, never original_repo_path
        &self.worktree_path
    }

    /// Validate we're in the correct directory
    pub fn validate(&self) -> Result<()> {
        let current = env::current_dir()?;
        if !current.starts_with(&self.worktree_path) {
            bail!(
                "Execution context violation: currently in {} but should be in {}",
                current.display(),
                self.worktree_path.display()
            );
        }
        Ok(())
    }
}
```

### APIs and Interfaces

No public API changes - this is an internal bug fix.

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - MapReduce orchestrator
  - Setup phase executor
  - Working directory management
  - Command execution logic
  - Git worktree integration
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_execution_context_always_returns_worktree() {
    let ctx = ExecutionContext {
        worktree_path: PathBuf::from("/worktrees/session-123"),
        original_repo_path: PathBuf::from("/original/repo"),
        session_id: "session-123".to_string(),
        phase: WorkflowPhase::Setup,
    };

    let exec_dir = ctx.execution_directory();
    assert_eq!(exec_dir, Path::new("/worktrees/session-123"));
    assert_ne!(exec_dir, Path::new("/original/repo"));
}

#[test]
fn test_validate_execution_context() {
    let ctx = create_test_context();

    // Should fail if current_dir != worktree_path
    env::set_current_dir("/wrong/directory").unwrap();
    assert!(ctx.validate().is_err());

    // Should succeed if current_dir == worktree_path
    env::set_current_dir(&ctx.worktree_path).unwrap();
    assert!(ctx.validate().is_ok());
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_setup_phase_modifies_worktree_not_main_repo() {
    let main_repo = create_test_repo();
    let workflow = r#"
        name: test-setup-isolation
        mode: mapreduce
        setup:
          - shell: "echo 'test' > test-file.txt"
          - shell: "git add test-file.txt && git commit -m 'test'"
    "#;

    // Execute workflow
    let result = run_workflow(&main_repo, workflow).await;
    assert!(result.is_ok());

    // VERIFY: Main repo should be clean
    let main_status = git_status(&main_repo)?;
    assert_eq!(main_status.modified_files.len(), 0);
    assert_eq!(main_status.untracked_files.len(), 0);

    // VERIFY: Worktree should have the changes
    let worktree_path = result.worktree_path;
    let worktree_status = git_status(&worktree_path)?;
    assert!(worktree_path.join("test-file.txt").exists());
}

#[tokio::test]
async fn test_setup_phase_commits_in_worktree() {
    let main_repo = create_test_repo();
    let workflow = create_test_workflow_with_commit_required();

    let result = run_workflow(&main_repo, workflow).await;

    // Should succeed (no "no commits created" error)
    assert!(result.is_ok());

    // Verify commit exists in worktree
    let worktree_path = result.worktree_path;
    let log = git_log(&worktree_path, 1)?;
    assert!(log.contains("test commit"));

    // Verify main repo has no new commits
    let main_log_before = git_log(&main_repo, 1)?;
    let main_log_after = git_log(&main_repo, 1)?;
    assert_eq!(main_log_before, main_log_after);
}
```

### Manual Testing

```bash
# 1. Create a clean test repo
mkdir /tmp/test-prodigy && cd /tmp/test-prodigy
git init
echo "# Test" > README.md
git add . && git commit -m "initial"

# 2. Run MapReduce workflow with setup phase
prodigy run workflows/book-docs-drift.yml -vv

# 3. Check main repo status (should be clean)
git status
# Expected: nothing to commit, working tree clean

# 4. Check worktree (should have changes)
cd ~/.prodigy/worktrees/prodigy/session-*/
git status
git log
# Expected: see the setup phase changes and commits
```

## Documentation Requirements

### Code Documentation

```rust
/// Execute the setup phase of a MapReduce workflow.
///
/// **IMPORTANT**: All setup commands execute in the isolated worktree,
/// NOT in the main repository. This ensures:
/// - Setup modifications are isolated
/// - Setup commits don't pollute main repo
/// - Multiple MapReduce workflows can run concurrently
///
/// The main repository remains untouched until the final merge phase.
///
/// # Worktree Isolation Guarantee
///
/// This function enforces strict worktree isolation:
/// 1. All commands execute with `cwd = worktree_path`
/// 2. Git operations occur in worktree context
/// 3. File modifications affect only worktree
/// 4. Validation checks prevent main repo access
///
/// # Example
///
/// ```rust
/// let result = execute_setup_phase(&config, &worktree_path).await?;
/// // Main repo is unchanged
/// // Worktree contains setup results
/// ```
pub async fn execute_setup_phase(
    config: &MapReduceConfig,
    worktree_path: &Path,
) -> Result<SetupResult>
```

### User Documentation

Update CLAUDE.md with corrected workflow behavior:

```markdown
## MapReduce Workflow Execution

### Worktree Isolation

**All MapReduce phases execute in isolated git worktrees**, never in your main repository:

```
Main Repository (untouched during execution)
    ↓
Worktree Created: ~/.prodigy/worktrees/{project}/session-{id}
    ↓
Setup Phase → Executes in worktree
    ↓
Map Phase → Each agent in separate worktree
    ↓
Reduce Phase → Executes in parent worktree
    ↓
Merge Phase → Merges worktree to main repo
```

Your main repository remains clean until you explicitly merge the worktree changes.

### Setup Phase Behavior

The setup phase:
- Executes in the worktree directory
- Creates files in the worktree
- Makes commits in the worktree
- Does NOT modify your main repository

Example:
```yaml
setup:
  - shell: "mkdir -p .prodigy/analysis"        # Creates in worktree
  - claude: "/analyze-features"                # Writes to worktree
  - shell: "git add . && git commit -m 'analysis'"  # Commits in worktree
```

After setup completes, your main repo is unchanged.
```

## Implementation Notes

### Debugging Tips

1. **Add verbose logging** to track working directory:
```rust
log::debug!("Current working directory: {:?}", env::current_dir());
log::debug!("Expected worktree: {:?}", worktree_path);
log::debug!("Original repo: {:?}", original_repo_path);
```

2. **Validate before each command**:
```rust
validate_execution_context(&worktree_path, command_name)?;
```

3. **Check git context**:
```rust
let git_dir = Command::new("git")
    .arg("rev-parse")
    .arg("--git-dir")
    .output()?;
log::debug!("Git directory: {}", String::from_utf8_lossy(&git_dir.stdout));
```

### Common Pitfalls

- **Environment variables**: `PWD`, `OLDPWD`, `GIT_DIR` may point to main repo
- **Relative paths**: Always use absolute paths for worktrees
- **Current directory caching**: Ensure current_dir is re-read after changes
- **Symlinks**: Worktrees may contain symlinks, resolve them properly

## Migration and Compatibility

### Breaking Changes

None - this is a bug fix that restores intended behavior.

### Behavior Changes

**Before (buggy)**:
- Setup phase modified main repo
- Commits created in main repo
- Workflows failed with "no commits created"

**After (fixed)**:
- Setup phase modifies worktree
- Commits created in worktree
- Workflows succeed as designed

### Migration Steps

1. Deploy fix
2. Test with existing workflows
3. Clean up any stuck worktrees: `prodigy worktree clean -f`
4. Re-run failed workflows: `prodigy resume <session-id>`

### Verification

After deploying fix:
```bash
# Run a workflow
prodigy run workflows/book-docs-drift.yml

# Verify main repo is clean
git status  # Should be clean

# Verify worktree has changes
ls ~/.prodigy/worktrees/prodigy/session-*/
# Should see workflow modifications
```
