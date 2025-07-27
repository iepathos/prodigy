# Specification 27: Enhanced Claude-Assisted Worktree Merge

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: 24, 25

## Context

The current Claude-assisted worktree merge (spec 25) provides basic conflict resolution but lacks robustness, flexibility, and performance optimizations. As teams use parallel MMM sessions more frequently, the merge process needs to handle edge cases better, provide more control over conflict resolution strategies, and integrate more tightly with the MMM quality tracking system.

## Objective

Enhance the Claude-assisted worktree merge functionality with pre-merge validation, multiple resolution strategies, performance optimizations for bulk operations, and better error recovery mechanisms to ensure reliable and efficient parallel workflow completion.

## Requirements

### Functional Requirements

1. **Pre-Merge Validation**
   - Verify target branch exists and is accessible
   - Check for uncommitted changes in both worktree and target
   - Validate worktree health and branch tracking
   - Ensure sufficient disk space for merge operations

2. **Resolution Strategies**
   - Support multiple conflict resolution strategies (aggressive, conservative, balanced, test-driven)
   - Allow dry-run mode to preview conflicts without merging
   - Provide strategy recommendations based on conflict analysis

3. **Enhanced Error Recovery**
   - Implement fallback to show conflict markers when Claude fails
   - Save partial progress for complex merges
   - Categorize errors (network, conflicts, permissions)
   - Support resumable merges

4. **Performance Optimizations**
   - Pre-fetch branches for `--all` flag operations
   - Batch conflict analysis across multiple files
   - Show progress indicators for long operations
   - Optimize git operations for large repositories

5. **Integration Improvements**
   - Auto-detect original branch for merging
   - Suggest merge order based on dependencies
   - Update quality scores after successful merge
   - Run configured test suites post-merge

### Non-Functional Requirements

- Merge operations should complete within reasonable time (< 5 minutes for typical conflicts)
- Clear feedback during all merge stages
- Detailed logging for debugging failed merges
- Backward compatibility with existing merge workflow

## Acceptance Criteria

- [ ] Pre-merge validation prevents common failure scenarios
- [ ] Multiple resolution strategies available via CLI flags
- [ ] Dry-run mode accurately predicts merge conflicts
- [ ] Failed Claude merges fallback gracefully to manual resolution
- [ ] Bulk merges with `--all` show progress and handle errors individually
- [ ] Post-merge tests can be configured and run automatically
- [ ] Edge cases (binary files, submodules, permissions) handled appropriately
- [ ] Performance metrics show 30% improvement for bulk operations
- [ ] Integration with quality tracking updates scores post-merge

## Technical Details

### Implementation Approach

1. **Enhanced CLI Options**
   ```rust
   WorktreeCommands::Merge {
       name: Option<String>,
       #[arg(long)]
       target: Option<String>,
       #[arg(long)]
       all: bool,
       #[arg(long)]
       dry_run: bool,
       #[arg(long, value_enum)]
       strategy: Option<MergeStrategy>,
       #[arg(long)]
       test: bool,
       #[arg(long)]
       progress: bool,
   }
   ```

2. **Merge Strategy Enum**
   ```rust
   enum MergeStrategy {
       Aggressive,    // Prefer incoming changes
       Conservative,  // Prefer current branch
       Balanced,      // Default behavior
       TestDriven,    // Run tests after each resolution
   }
   ```

3. **Enhanced Claude Command Interface**
   ```markdown
   /mmm-merge-worktree <branch> [options]
   
   Options:
   --target <branch>      Target branch (default: current)
   --strategy <strategy>  Resolution strategy
   --dry-run             Preview without merging
   --test-command <cmd>  Run tests after merge
   ```

### Architecture Changes

1. **WorktreeManager Updates**
   - Add `validate_merge_prerequisites()` method
   - Implement `merge_session_with_options()` for enhanced control
   - Add progress reporting callbacks
   - Support merge state persistence

2. **Claude Command Enhancements**
   - Accept strategy parameter for conflict resolution
   - Provide richer context (test coverage, recent commits)
   - Support incremental conflict resolution
   - Generate detailed merge reports

### Data Structures

```rust
struct MergeOptions {
    target_branch: Option<String>,
    strategy: MergeStrategy,
    dry_run: bool,
    test_command: Option<String>,
    show_progress: bool,
}

struct MergeResult {
    success: bool,
    conflicts_resolved: Vec<ConflictInfo>,
    test_results: Option<TestResult>,
    quality_score_delta: Option<f32>,
    merge_commit: Option<String>,
}

struct ConflictInfo {
    file_path: String,
    conflict_type: ConflictType,
    resolution_method: String,
    confidence: f32,
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 24 (Git worktree isolation) - Base worktree functionality
  - Spec 25 (Claude-assisted worktree merge) - Basic merge capability
- **Affected Components**: 
  - `src/worktree/manager.rs` - Major enhancements
  - `src/main.rs` - New CLI options
  - `.claude/commands/mmm-merge-worktree.md` - Strategy support
- **External Dependencies**: 
  - git 2.5+ (existing requirement)
  - Claude CLI (existing requirement)

## Testing Strategy

- **Unit Tests**: 
  - Mock various conflict scenarios with different strategies
  - Test pre-merge validation logic
  - Verify progress reporting accuracy
  - Test error categorization

- **Integration Tests**: 
  - Create real git conflicts and test resolution
  - Test bulk merge operations with mixed success/failure
  - Verify dry-run predictions match actual merges
  - Test merge resumption after failures

- **Performance Tests**: 
  - Benchmark bulk merge operations
  - Measure overhead of pre-merge validation
  - Compare strategy performance

- **Edge Case Tests**:
  - Binary file conflicts
  - Submodule conflicts
  - File permission changes
  - Large file handling
  - Circular dependencies

## Documentation Requirements

- **Code Documentation**: 
  - Document all new merge options and strategies
  - Add examples for each strategy type
  - Document error recovery procedures

- **User Documentation**: 
  - Update README with new merge options
  - Add troubleshooting guide for merge failures
  - Create strategy selection guide
  - Document best practices for parallel workflows

- **Architecture Updates**: 
  - Update ARCHITECTURE.md with enhanced merge flow
  - Document merge state persistence format
  - Add sequence diagrams for complex scenarios

## Implementation Notes

1. **Backward Compatibility**
   - Existing `mmm worktree merge` commands work unchanged
   - New options are additive only
   - Default behavior remains the same

2. **Performance Considerations**
   - Use git plumbing commands for efficiency
   - Cache branch information during bulk operations
   - Stream progress updates instead of buffering

3. **Error Handling Philosophy**
   - Fail fast for configuration errors
   - Attempt recovery for transient failures
   - Always leave repository in clean state

4. **Future Extensibility**
   - Design for pluggable merge strategies
   - Allow custom conflict resolution scripts
   - Support merge hooks for integration

## Migration and Compatibility

- No breaking changes to existing workflow
- Configuration files remain compatible
- Existing Claude commands continue to work
- Optional features require opt-in via flags

## Success Metrics

- 90% of merges complete without manual intervention
- 30% reduction in time for bulk merge operations
- Zero data loss from failed merges
- 95% accuracy in dry-run predictions
- Positive user feedback on merge reliability

## Example Workflows

### Preview Conflicts
```bash
mmm worktree merge mmm-feature-123 --dry-run
# Shows what conflicts would occur without merging
```

### Aggressive Merge with Testing
```bash
mmm worktree merge mmm-perf-456 --strategy aggressive --test
# Prefers incoming changes and runs tests after
```

### Bulk Merge with Progress
```bash
mmm worktree merge --all --progress
# Shows progress bar and handles each worktree
```

### Conservative Merge to Specific Branch
```bash
mmm worktree merge mmm-fix-789 --target release-1.0 --strategy conservative
# Carefully merges bugfix to release branch
```