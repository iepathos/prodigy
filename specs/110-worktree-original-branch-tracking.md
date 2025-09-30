---
number: 110
title: Worktree Original Branch Tracking
category: foundation
priority: critical
status: draft
dependencies: [109]
created: 2025-09-29
---

# Specification 110: Worktree Original Branch Tracking

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [109]

## Context

Currently, when a workflow creates a git worktree (via `-w` flag or default behavior per Spec 109), the system tracks the **new worktree branch** (e.g., `prodigy-session-{uuid}`) but does NOT track the **original branch** the user was on when they started the workflow.

This causes a critical issue at workflow completion: the system merges the worktree back to a hardcoded `main` or `master` branch, instead of merging back to the **original branch** the user was working on.

### Current Behavior (Incorrect)

```bash
# User is on feature branch
$ git checkout feature-xyz
$ git branch
  master
* feature-xyz

# Run workflow with worktree
$ prodigy run workflow.yml -w

# Worktree created: prodigy-session-abc123
# Worktree branches from feature-xyz HEAD ✅ (correct)

# At workflow completion:
# ❌ System merges to main/master (WRONG!)
# ✅ Should merge to feature-xyz (the original branch)
```

### Expected Behavior (Correct)

```bash
# User is on feature branch
$ git checkout feature-xyz

# Run workflow
$ prodigy run workflow.yml -w

# Worktree branches from feature-xyz ✅
# At end: merges back to feature-xyz ✅
# Original branch unchanged if user was on master
```

### Impact

This bug affects:
- **All workflows using worktrees** (100% of workflows after Spec 109)
- **Feature branch development**: Changes get merged to wrong branch
- **MapReduce workflows**: Parent worktree merges to wrong target
- **User expectations**: Surprising and potentially destructive behavior

### Root Cause

**Location**: `/Users/glen/memento-mori/prodigy/src/worktree/manager.rs:668-680`

The `determine_default_branch()` function hardcodes the merge target:
```rust
async fn determine_default_branch(&self) -> Result<String> {
    let main_exists = self.check_branch_exists("main").await?;
    Ok(Self::select_default_branch(main_exists))
}

fn select_default_branch(main_exists: bool) -> String {
    if main_exists { "main".to_string() }   // ❌ Hardcoded
    else { "master".to_string() }           // ❌ Hardcoded
}
```

**Data Structure Gap**: `/Users/glen/memento-mori/prodigy/src/worktree/state.rs:8-26`

`WorktreeState` only tracks the new worktree branch, not the original:
```rust
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,              // The NEW worktree branch
    // ❌ MISSING: original_branch field
}
```

## Objective

Track the original branch when creating a worktree and use it as the merge target when the workflow completes, ensuring workflows merge back to the branch the user was working on, not a hardcoded main/master.

## Requirements

### Functional Requirements

1. **Capture Original Branch**
   - Detect current branch when creating worktree
   - Use `git rev-parse --abbrev-ref HEAD` to get branch name
   - Handle special cases: detached HEAD, new repository with no commits

2. **Store Original Branch**
   - Add `original_branch` field to `WorktreeState`
   - Save original branch to session state file
   - Persist across workflow execution and interruptions

3. **Use Original Branch for Merge**
   - Replace `determine_default_branch()` with `get_merge_target()`
   - Read original branch from session state
   - Merge worktree back to original branch, not main/master

4. **Handle Special Cases**
   - **Detached HEAD**: Fall back to main/master (user not on a branch)
   - **New repo with no commits**: Fall back to main/master
   - **Original branch deleted**: Warn user and fall back to main/master
   - **Missing original_branch in old sessions**: Fall back to main/master

5. **User Communication**
   - Display original branch at workflow start
   - Show merge target at workflow completion
   - Provide clear warnings if fallback is used

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing worktree sessions without `original_branch` continue to work
   - Graceful fallback for old session state files
   - No breaking changes to WorktreeSession API

2. **Performance**
   - Branch detection adds negligible overhead (< 100ms)
   - No impact on workflow execution speed

3. **Reliability**
   - Robust error handling for git command failures
   - Safe fallbacks for edge cases
   - No risk of data loss or corruption

## Acceptance Criteria

- [ ] `WorktreeState` has `original_branch: String` field
- [ ] `create_session_with_id()` captures current branch via `git rev-parse --abbrev-ref HEAD`
- [ ] Original branch saved to session state file
- [ ] `merge_session()` reads original branch from state instead of calling `determine_default_branch()`
- [ ] Detached HEAD case falls back to main/master with warning
- [ ] Original branch deleted case falls back to main/master with warning
- [ ] Old sessions without `original_branch` field fall back to main/master
- [ ] Workflow start logs: "Creating worktree from branch: feature-xyz"
- [ ] Workflow end prompts: "Merge prodigy-session-xxx to feature-xyz? [y/N]"
- [ ] `.claude/commands/merge-master.md` renamed to `merge-branch.md` or similar
- [ ] Merge command updated to merge any source branch into current branch (not hardcoded master)
- [ ] All existing tests pass without modification
- [ ] New integration tests verify branch tracking for main, feature branches, and edge cases
- [ ] Documentation updated to explain branch tracking behavior

## Technical Details

### Implementation Approach

**Phase 1: Extend WorktreeState**

1. Add `original_branch` field to `WorktreeState` struct
2. Update serialization/deserialization to handle missing field (backward compat)
3. Update all constructors to include original branch

**Phase 2: Capture Original Branch**

1. Add `get_current_branch()` method to `WorktreeManager`
2. Call during `create_session_with_id()` before creating worktree
3. Store in `WorktreeState` and persist to disk

**Phase 3: Use Original Branch for Merge**

1. Replace `determine_default_branch()` with `get_merge_target()`
2. Read `original_branch` from session state
3. Use as merge target in `merge_session()`

**Phase 4: Handle Edge Cases**

1. Add fallback logic for missing/invalid original branch
2. Add warning messages for special cases
3. Ensure robust error handling

### Architecture Changes

**Before:**
```rust
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,              // Worktree branch
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub status: WorktreeStatus,
}

async fn determine_default_branch(&self) -> Result<String> {
    // Returns "main" or "master" (hardcoded)
}
```

**After:**
```rust
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,              // Worktree branch (prodigy-session-xxx)
    pub original_branch: String,     // ← NEW: Branch we branched from
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub status: WorktreeStatus,
}

async fn get_merge_target(&self, session_name: &str) -> Result<String> {
    // Returns original_branch from session state
    let state = self.get_session_state(session_name)?;

    // Fallback for old sessions or edge cases
    if state.original_branch.is_empty()
        || state.original_branch == "HEAD" {
        return self.determine_default_branch().await;
    }

    // Check if original branch still exists
    if !self.check_branch_exists(&state.original_branch).await? {
        warn!("Original branch {} no longer exists, using default",
              state.original_branch);
        return self.determine_default_branch().await;
    }

    Ok(state.original_branch)
}
```

### Data Structures

**WorktreeState Changes:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,

    // NEW FIELD: Track the branch we branched from
    #[serde(default = "default_original_branch")]
    pub original_branch: String,

    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub status: WorktreeStatus,
}

// Backward compatibility: default to empty string for old sessions
fn default_original_branch() -> String {
    String::new()
}
```

### APIs and Interfaces

**New Methods:**

```rust
impl WorktreeManager {
    /// Get the current branch name from the repository
    async fn get_current_branch(&self) -> Result<String> {
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .build();

        let output = self.subprocess.runner().run(command).await?;

        if !output.status.success() {
            anyhow::bail!("Failed to get current branch");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a branch exists in the repository
    async fn check_branch_exists(&self, branch: &str) -> Result<bool> {
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
            .build();

        let output = self.subprocess.runner().run(command).await?;
        Ok(output.status.success())
    }

    /// Get merge target for a session (original branch with fallbacks)
    async fn get_merge_target(&self, session_name: &str) -> Result<String> {
        let state = self.get_session_state(session_name)?;

        // Handle old sessions or edge cases
        if state.original_branch.is_empty()
            || state.original_branch == "HEAD" {
            warn!("No original branch tracked, using default");
            return self.determine_default_branch().await;
        }

        // Verify original branch still exists
        if !self.check_branch_exists(&state.original_branch).await? {
            warn!(
                "Original branch '{}' no longer exists, using default",
                state.original_branch
            );
            return self.determine_default_branch().await;
        }

        Ok(state.original_branch.clone())
    }
}
```

**Modified Methods:**

```rust
impl WorktreeManager {
    pub async fn create_session_with_id(
        &self,
        session_id: &str
    ) -> Result<WorktreeSession> {
        // Capture current branch BEFORE creating worktree
        let original_branch = self.get_current_branch().await
            .unwrap_or_else(|_| {
                warn!("Failed to detect current branch, will use default for merge");
                String::from("HEAD")
            });

        info!("Creating worktree from branch: {}", original_branch);

        // ... existing worktree creation code ...

        let state = WorktreeState {
            session_id: session_id.to_string(),
            worktree_name: session_name.clone(),
            branch: format!("prodigy-{}", session_id),
            original_branch,  // ← Store original branch
            path: worktree_path.clone(),
            created_at: Utc::now(),
            status: WorktreeStatus::Active,
        };

        // ... save state ...

        Ok(session)
    }

    pub async fn merge_session(&self, name: &str) -> Result<()> {
        let session = self.find_session_by_name(name).await?;
        let worktree_branch = &session.branch;

        // Use original branch instead of hardcoded main/master
        let target_branch = self.get_merge_target(name).await?;

        info!("Merging {} to {}", worktree_branch, target_branch);

        // ... existing merge logic ...
    }
}
```

## Dependencies

- **Prerequisites**: Spec 109 (default worktrees) should be implemented first
- **Affected Components**:
  - `src/worktree/state.rs` - Add `original_branch` field
  - `src/worktree/manager.rs` - Capture, store, and use original branch
  - `src/cook/orchestrator.rs` - Updated merge prompts and logging
  - `.claude/commands/merge-master.md` - Rename and generalize to merge from any branch
- **External Dependencies**: None (uses existing git commands)

## Testing Strategy

### Unit Tests

**Branch Detection:**
```rust
#[tokio::test]
async fn test_get_current_branch() {
    let manager = setup_test_worktree_manager().await;
    let branch = manager.get_current_branch().await.unwrap();
    assert_eq!(branch, "master"); // or "main"
}

#[tokio::test]
async fn test_get_current_branch_detached_head() {
    // Checkout specific commit (detached HEAD)
    // Verify returns "HEAD"
}
```

**Branch Existence Check:**
```rust
#[tokio::test]
async fn test_check_branch_exists() {
    let manager = setup_test_worktree_manager().await;
    assert!(manager.check_branch_exists("master").await.unwrap());
    assert!(!manager.check_branch_exists("nonexistent").await.unwrap());
}
```

**State Serialization:**
```rust
#[test]
fn test_worktree_state_with_original_branch() {
    let state = WorktreeState {
        original_branch: "feature-xyz".to_string(),
        // ... other fields
    };

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: WorktreeState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.original_branch, "feature-xyz");
}

#[test]
fn test_worktree_state_backward_compatibility() {
    // Old state without original_branch field
    let old_json = r#"{"session_id":"123","branch":"prodigy-123"}"#;
    let state: WorktreeState = serde_json::from_str(old_json).unwrap();
    assert_eq!(state.original_branch, ""); // Default value
}
```

### Integration Tests

**Feature Branch Workflow:**
```rust
#[tokio::test]
async fn test_worktree_tracks_feature_branch() {
    // Setup: Create feature branch
    run_git_command(&["checkout", "-b", "feature-xyz"]).await;

    // Create worktree
    let manager = WorktreeManager::new(repo_path, subprocess);
    let session = manager.create_session_with_id("test-session").await.unwrap();

    // Verify original branch tracked
    let state = manager.get_session_state(&session.name).unwrap();
    assert_eq!(state.original_branch, "feature-xyz");

    // Make changes in worktree
    // ...

    // Merge session
    manager.merge_session(&session.name).await.unwrap();

    // Verify merged to feature-xyz, not master
    let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(current, "feature-xyz");

    // Verify changes are in feature-xyz
    let log = run_git_command(&["log", "--oneline", "-1"]).await;
    assert!(log.contains("Merge worktree"));
}
```

**Main Branch Workflow:**
```rust
#[tokio::test]
async fn test_worktree_tracks_main_branch() {
    // On main branch
    run_git_command(&["checkout", "main"]).await;

    // Create worktree
    let session = manager.create_session_with_id("test").await.unwrap();

    // Verify tracks main
    let state = manager.get_session_state(&session.name).unwrap();
    assert_eq!(state.original_branch, "main");

    // Merge back
    manager.merge_session(&session.name).await.unwrap();

    // Verify merged to main
    let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(current, "main");
}
```

**Edge Case: Detached HEAD:**
```rust
#[tokio::test]
async fn test_worktree_from_detached_head() {
    // Checkout specific commit (detached HEAD)
    run_git_command(&["checkout", "HEAD~1"]).await;

    // Create worktree
    let session = manager.create_session_with_id("test").await.unwrap();

    // Verify original_branch is "HEAD"
    let state = manager.get_session_state(&session.name).unwrap();
    assert_eq!(state.original_branch, "HEAD");

    // Merge should fall back to main/master
    manager.merge_session(&session.name).await.unwrap();

    // Verify used fallback (not HEAD)
    let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert!(current == "main" || current == "master");
}
```

**Edge Case: Original Branch Deleted:**
```rust
#[tokio::test]
async fn test_original_branch_deleted() {
    // Create and track feature branch
    run_git_command(&["checkout", "-b", "temp-feature"]).await;
    let session = manager.create_session_with_id("test").await.unwrap();

    // Switch away and delete original branch
    run_git_command(&["checkout", "main"]).await;
    run_git_command(&["branch", "-D", "temp-feature"]).await;

    // Merge should fall back gracefully
    manager.merge_session(&session.name).await.unwrap();

    // Verify used fallback
    let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert!(current == "main" || current == "master");
}
```

**Backward Compatibility:**
```rust
#[tokio::test]
async fn test_old_session_without_original_branch() {
    // Create old-style session state without original_branch field
    let old_state = WorktreeState {
        session_id: "old-session".to_string(),
        branch: "prodigy-old-session".to_string(),
        original_branch: String::new(), // Empty (missing in old format)
        // ... other fields
    };

    manager.save_session_state(&old_state).unwrap();

    // Merge should fall back to main/master
    manager.merge_session("old-session").await.unwrap();

    // Verify used fallback
    let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert!(current == "main" || current == "master");
}
```

### Manual Testing

1. **Feature Branch Test:**
   ```bash
   git checkout -b test-feature
   prodigy run workflow.yml -w
   # Verify: "Creating worktree from branch: test-feature"
   # Verify: Merge prompt shows "Merge to test-feature"
   ```

2. **Main/Master Test:**
   ```bash
   git checkout main
   prodigy run workflow.yml -w
   # Verify: Tracks and merges back to main
   ```

3. **Detached HEAD Test:**
   ```bash
   git checkout HEAD~1
   prodigy run workflow.yml -w
   # Verify: Warning about detached HEAD
   # Verify: Falls back to main/master
   ```

4. **Branch Deleted Test:**
   ```bash
   git checkout -b temp-branch
   prodigy run workflow.yml -w
   # In another terminal: git branch -D temp-branch
   # Complete workflow
   # Verify: Warning about deleted branch
   # Verify: Falls back gracefully
   ```

## Documentation Requirements

### Code Documentation

**Update WorktreeState doc comment:**
```rust
/// Session state for a git worktree
///
/// Tracks both the worktree branch (e.g., `prodigy-session-{uuid}`) and
/// the original branch the worktree was created from. The original branch
/// is used as the merge target when the session completes.
///
/// # Fields
///
/// * `branch` - The worktree's branch name (e.g., `prodigy-session-abc123`)
/// * `original_branch` - The branch we branched from (e.g., `feature-xyz`, `main`)
///   Used as the merge target. Empty for old sessions (pre-spec-110).
```

**Add method documentation:**
```rust
/// Get the current branch name
///
/// Returns the name of the currently checked-out branch, or "HEAD"
/// if in detached HEAD state.
async fn get_current_branch(&self) -> Result<String>

/// Determine the merge target for a worktree session
///
/// Returns the original branch from session state, with fallbacks:
/// 1. If original_branch is empty (old session): use main/master
/// 2. If original_branch is "HEAD" (detached): use main/master
/// 3. If original_branch was deleted: warn and use main/master
/// 4. Otherwise: use original_branch
async fn get_merge_target(&self, session_name: &str) -> Result<String>
```

### Claude Commands

**Rename and update `.claude/commands/merge-master.md`:**

1. Rename file to `.claude/commands/merge-branch.md`
2. Update title: "Merge Master Into Current Branch" → "Merge Source Branch Into Current Branch"
3. Add SOURCE_BRANCH parameter:
   ```markdown
   ## Variables

   SOURCE_BRANCH: $ARGUMENTS (required - the branch to merge from, e.g., "main", "master", "develop")
   ```
4. Replace hardcoded master/main with SOURCE_BRANCH variable
5. Update step 1 to accept branch as parameter instead of determining default
6. Update all git commands: `git merge origin/$DEFAULT_BRANCH` → `git merge origin/$SOURCE_BRANCH`
7. Update commit message template to use SOURCE_BRANCH variable
8. Update examples to show usage: `/merge-branch main`, `/merge-branch develop`

**Resulting command structure:**
```bash
# Merge main into current feature branch
/merge-branch main

# Merge develop into current feature branch
/merge-branch develop

# Merge any branch into current branch
/merge-branch <source-branch>
```

### User Documentation

**Update CLAUDE.md:**
```markdown
## Git Worktree Behavior

All Prodigy workflows run in isolated git worktrees by default (Spec 109).

### Branch Tracking

Prodigy automatically tracks the branch you're on when starting a workflow:

```bash
# On feature branch
$ git branch
  main
* feature-xyz

# Run workflow
$ prodigy run workflow.yml

# At completion, merges back to feature-xyz ✅
# NOT to main/master ❌
```

### Special Cases

**Detached HEAD**: If you start a workflow in detached HEAD state (e.g., checking
out a specific commit), Prodigy will create the worktree but fall back to merging
to `main` or `master` at completion.

**Deleted Branch**: If the original branch is deleted during workflow execution,
Prodigy will warn you and fall back to `main` or `master`.

**Old Sessions**: Worktree sessions created before Spec 110 don't track the
original branch and will merge to `main` or `master`.
```

### Architecture Updates

**Update ARCHITECTURE.md:**
```markdown
### Git Worktree Management

#### Branch Tracking

Every worktree session tracks two branches:

1. **Worktree Branch** (`branch` field): The temporary branch for the worktree
   - Format: `prodigy-session-{uuid}` or `prodigy-mapreduce-agent-{id}`
   - Created when worktree is created
   - Deleted when worktree is cleaned up

2. **Original Branch** (`original_branch` field): The branch the worktree was created from
   - Detected via `git rev-parse --abbrev-ref HEAD`
   - Stored in session state
   - Used as merge target at workflow completion

#### Merge Target Determination

At workflow completion, Prodigy merges the worktree to the **original branch**:

```
User on: feature-xyz
Worktree: prodigy-session-abc123 (branched from feature-xyz)
Changes:  Made in worktree
Merge:    prodigy-session-abc123 → feature-xyz ✅
```

**Fallback Behavior**: If original branch cannot be determined or no longer exists:
1. Check if `main` branch exists → use `main`
2. Otherwise → use `master`
3. Log warning about fallback

This ensures workflows always merge back to the correct branch, not hardcoded `main`/`master`.
```

## Implementation Notes

### Migration Strategy

**No data migration required** - backward compatibility through `#[serde(default)]`:

```rust
#[serde(default = "default_original_branch")]
pub original_branch: String,
```

Old session files without `original_branch` will deserialize with empty string, triggering fallback logic.

### Gotchas

1. **Detached HEAD Detection**: `git rev-parse --abbrev-ref HEAD` returns "HEAD" in detached state
2. **Branch Existence**: Must verify branch still exists before merging
3. **Concurrent Sessions**: Multiple worktrees can track different original branches
4. **Interrupted Sessions**: Original branch persisted to disk survives restarts

### Performance Considerations

- `git rev-parse --abbrev-ref HEAD`: ~10-50ms
- `git rev-parse --verify refs/heads/{branch}`: ~10-50ms
- Total overhead: < 100ms per workflow (negligible)

### Error Handling

All git commands must handle failures gracefully:
- Branch detection fails → fallback to main/master with warning
- Branch verification fails → fallback to main/master with warning
- Never crash or fail workflow due to branch tracking issues

## Migration and Compatibility

### Breaking Changes

**None** - this is a backward-compatible enhancement:
- Old `WorktreeState` files deserialize successfully (empty `original_branch`)
- Empty `original_branch` triggers fallback to main/master (same as old behavior)
- New sessions get improved behavior automatically

### Migration Path

**No action required**:
1. Deploy updated code
2. New worktrees automatically track original branch
3. Old worktrees continue using fallback behavior
4. Old worktrees naturally age out as they complete

### Rollback Plan

If issues arise:
1. Revert commits adding `original_branch` tracking
2. System falls back to old `determine_default_branch()` behavior
3. No data loss (field is optional in serialization)

## Success Metrics

- [ ] 100% of new worktrees track original branch
- [ ] Merge target matches original branch in >99% of cases
- [ ] Zero regressions in existing workflows
- [ ] Fallback behavior works for all edge cases (detached HEAD, deleted branch)
- [ ] User feedback confirms expected merge behavior
- [ ] No performance impact (< 100ms overhead)

## Related Specifications

- **Spec 109**: Default Git Worktree Isolation (prerequisite - makes worktrees mandatory)
- **Spec 101**: Error Handling Guidelines (affects error messages for git operations)
- **MapReduce Architecture**: Ensures agent worktrees merge to correct parent worktree branch