# Implementation Plan: Reduce Complexity in CommitTracker::create_auto_commit

## Problem Summary

**Location**: ./src/cook/commit_tracker.rs:CommitTracker::create_auto_commit:413
**Priority Score**: 28.76
**Debt Type**: ComplexityHotspot (Cyclomatic: 27, Cognitive: 70)
**Current Metrics**:
- Lines of Code: 99
- Cyclomatic Complexity: 27
- Cognitive Complexity: 70
- Nesting Depth: 3

**Issue**: High complexity 27/70 makes function hard to test and maintain. The function mixes multiple responsibilities: file staging decisions, message generation, author/signing configuration, git execution, and commit tracking.

## Target State

**Expected Impact**:
- Complexity Reduction: 13.5 (from 27 to ~10-14)
- Coverage Improvement: 0.0
- Risk Reduction: 7.42

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 27 to ≤ 10
- [ ] Cognitive complexity reduced from 70 to ≤ 30
- [ ] Pure functions extracted for staging logic, message generation, and commit args building
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract File Staging Decision Logic

**Goal**: Separate file staging responsibility into a pure decision function and move I/O to caller context.

**Changes**:
- Extract `determine_staging_strategy()` - pure function that returns staging decision based on commit_config
- Extract `stage_files()` - handles the actual git operations based on staging strategy
- Simplify main function by delegating to these helpers

**New Functions**:
```rust
// Pure decision logic
fn determine_staging_strategy(commit_config: Option<&CommitConfig>) -> StagingStrategy {
    // Returns: StagingStrategy::All or StagingStrategy::Selective(patterns)
}

// I/O wrapper
async fn stage_files(&self, strategy: StagingStrategy) -> Result<()> {
    // Handles actual git add operations
}
```

**Testing**:
```bash
cargo test commit_tracker::
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] `determine_staging_strategy` is a pure function with no side effects
- [ ] `stage_files` handles all git operations
- [ ] Main function delegates to these helpers
- [ ] Complexity reduced by ~5 points
- [ ] All existing tests pass
- [ ] Ready to commit

**Estimated Complexity Reduction**: 5 points (27 → 22)

### Phase 2: Extract Commit Message Generation and Validation

**Goal**: Move message generation and validation into separate, testable functions.

**Changes**:
- Extract `generate_commit_message()` - pure function that generates message from template/step
- Extract `validate_commit_message()` - pure function that validates against pattern
- Combine these into a single `prepare_commit_message()` function

**New Functions**:
```rust
// Pure message generation
fn generate_commit_message(
    step_name: &str,
    template: Option<&str>,
    variables: &HashMap<String, String>
) -> Result<String> {
    // Returns generated message
}

// Pure validation
fn validate_commit_message(message: &str, pattern: Option<&str>) -> Result<()> {
    // Returns Ok(()) or validation error
}

// Combined preparation
fn prepare_commit_message(
    step_name: &str,
    template: Option<&str>,
    variables: &HashMap<String, String>,
    commit_config: Option<&CommitConfig>
) -> Result<String> {
    let message = generate_commit_message(step_name, template, variables)?;
    if let Some(config) = commit_config {
        validate_commit_message(&message, config.message_pattern.as_deref())?;
    }
    Ok(message)
}
```

**Testing**:
```bash
cargo test commit_tracker::
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Message generation logic is extracted and pure
- [ ] Validation logic is extracted and pure
- [ ] Main function uses `prepare_commit_message()`
- [ ] Complexity reduced by ~4 points
- [ ] All tests pass
- [ ] Ready to commit

**Estimated Complexity Reduction**: 4 points (22 → 18)

### Phase 3: Extract Commit Arguments Builder

**Goal**: Separate the complex logic for building commit arguments (author, signing) into a pure function.

**Changes**:
- Extract `build_commit_args()` - pure function that builds git commit arguments
- Move GPG config check into the args builder decision logic
- Simplify main function

**New Functions**:
```rust
// Pure argument building
fn build_commit_args<'a>(
    message: &'a str,
    commit_config: Option<&CommitConfig>,
    gpg_configured: bool
) -> Vec<String> {
    let mut args = vec!["commit".to_string(), "-m".to_string(), message.to_string()];

    if let Some(config) = commit_config {
        if let Some(author) = &config.author {
            args.push(format!("--author={}", author));
        }

        if config.sign && gpg_configured {
            args.push("-S".to_string());
        }
    }

    args
}
```

**Testing**:
```bash
cargo test commit_tracker::
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Commit args building is extracted and pure
- [ ] GPG configuration check happens before calling builder
- [ ] Main function uses `build_commit_args()`
- [ ] Complexity reduced by ~5 points
- [ ] All tests pass
- [ ] Ready to commit

**Estimated Complexity Reduction**: 5 points (18 → 13)

### Phase 4: Extract Commit Tracking Logic

**Goal**: Separate the commit retrieval and tracking logic into a focused helper function.

**Changes**:
- Extract `track_new_commit()` - handles retrieving and tracking the newly created commit
- Simplify the main function's final section

**New Functions**:
```rust
async fn track_new_commit(
    &self,
    step_name: &str,
    new_head: &str
) -> Result<TrackedCommit> {
    let mut commits = self
        .get_commits_between(&format!("{new_head}^"), new_head)
        .await?;

    let mut commit = commits
        .pop()
        .ok_or_else(|| anyhow!("Failed to retrieve created commit"))?;

    commit.step_name = step_name.to_string();

    let mut tracked = self.tracked_commits.write().await;
    tracked.push(commit.clone());

    Ok(commit)
}
```

**Testing**:
```bash
cargo test commit_tracker::
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Commit tracking is extracted into helper
- [ ] Main function delegates final tracking to helper
- [ ] Complexity reduced to target ≤ 10
- [ ] All tests pass
- [ ] Ready to commit

**Estimated Complexity Reduction**: 3 points (13 → 10)

### Phase 5: Final Cleanup and Verification

**Goal**: Ensure the refactored code meets all quality standards and verify improvements.

**Changes**:
- Run full test suite to ensure no regressions
- Run clippy to check for any remaining warnings
- Verify complexity reduction with metrics tools
- Update any documentation if needed

**Testing**:
```bash
# Full test suite
cargo test --all

# Clippy with strict settings
cargo clippy -- -D warnings

# Format check
cargo fmt --check

# Run full CI checks
just ci
```

**Success Criteria**:
- [ ] All tests pass (including integration tests)
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Cyclomatic complexity ≤ 10
- [ ] Cognitive complexity ≤ 30
- [ ] Function length reduced (if possible)
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test commit_tracker::` to verify CommitTracker tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Review the diff to ensure changes are minimal and focused
4. Commit with descriptive message explaining the refactoring

**Final verification**:
1. `just ci` - Full CI checks
2. Manual review of the simplified `create_auto_commit` function
3. Compare before/after complexity metrics

## Rollback Plan

If a phase fails:
1. Review the test failures or clippy warnings
2. Identify the specific issue (logic error, missing edge case, etc.)
3. Options:
   - Fix the issue if it's simple (typo, missing condition)
   - Revert with `git reset --hard HEAD~1` if the approach was wrong
   - Adjust the extraction strategy and retry

## Notes

**Key Principles**:
- Extract pure functions wherever possible (staging decisions, message generation, args building)
- Keep I/O operations (git commands) in the main function or dedicated I/O helpers
- Each extracted function should have a single, clear responsibility
- Maintain backward compatibility - no changes to public API or behavior

**Complexity Sources to Address**:
1. Nested conditionals in file staging (lines 426-448) - **Phase 1**
2. Message generation and validation conditionals (lines 450-462) - **Phase 2**
3. Author and GPG signing nested logic (lines 467-485) - **Phase 3**
4. Commit retrieval pattern matching (lines 495-509) - **Phase 4**

**Expected Final Structure**:
```rust
pub async fn create_auto_commit(...) -> Result<TrackedCommit> {
    // 1. Check for changes (existing)
    if !self.has_changes().await? {
        return Err(anyhow!("No changes to commit"));
    }

    // 2. Stage files (delegated)
    let strategy = determine_staging_strategy(commit_config);
    self.stage_files(strategy).await?;

    // 3. Prepare message (delegated)
    let message = prepare_commit_message(step_name, message_template, variables, commit_config)?;

    // 4. Build commit args (delegated)
    let gpg_configured = if should_check_gpg(commit_config) {
        self.check_gpg_config().await?
    } else {
        false
    };
    let commit_args = build_commit_args(&message, commit_config, gpg_configured);

    // 5. Execute commit (existing I/O)
    self.git_ops.git_command_in_dir(&commit_args, "create commit", &self.working_dir).await?;

    // 6. Track commit (delegated)
    let new_head = self.get_current_head().await?;
    self.track_new_commit(step_name, &new_head).await
}
```

**Estimated Final Complexity**: ~10 cyclomatic, ~25 cognitive
