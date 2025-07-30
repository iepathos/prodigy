# Specification 47: Auto-Commit Analysis Changes

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [Spec 44: Context-Aware Project Understanding, Spec 19: Git-Native Improvement Flow]

## Context

When MMM runs the `analyze` command, it updates multiple files in the `.mmm/` directory containing project analysis data:
- `.mmm/context/analysis.json`
- `.mmm/context/analysis_metadata.json`
- `.mmm/context/architecture.json`
- `.mmm/context/conventions.json`
- `.mmm/context/dependency_graph.json`
- `.mmm/context/technical_debt.json`
- `.mmm/metrics/current.json`

Currently, these changes are left uncommitted in the working directory. This creates several issues:
1. Analysis updates are not tracked in version control
2. Git status becomes cluttered with `.mmm` changes
3. Analysis history is lost between sessions
4. Parallel worktree sessions may have conflicting analysis states

Automatically committing these changes would provide better tracking of project evolution and cleaner git workflow.

## Objective

Implement automatic git commits for analysis changes, creating a proper audit trail of project analysis evolution while maintaining clean git history.

## Requirements

### Functional Requirements

1. **Automatic Commit Creation**
   - Detect when analysis files have been modified
   - Create atomic commits containing only analysis changes
   - Generate descriptive commit messages with analysis metadata
   - Handle both full and incremental analysis updates

2. **Commit Message Format**
   - Use consistent prefix: `analysis: `
   - Include analysis type (full/incremental)
   - Add timestamp and duration information
   - Include file count and key metrics changes
   - Example: `analysis: full project analysis (127 files, 1.5s)`

3. **Git Integration**
   - Check for uncommitted analysis changes before committing
   - Stage only `.mmm/` directory changes
   - Preserve other working directory changes
   - Handle cases where no changes occurred

4. **Error Handling**
   - Gracefully handle git errors (no repo, permissions)
   - Continue analysis even if commit fails
   - Log commit failures for debugging
   - Don't interfere with user's git workflow

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead on analysis operations
   - Efficient git operations (use --quiet flags)
   - No impact on analysis accuracy or speed

2. **Compatibility**
   - Work with all git configurations
   - Support both regular and worktree sessions
   - Compatible with existing MMM workflows
   - Don't break CI/CD pipelines

## Acceptance Criteria

- [ ] Analysis runs automatically commit their changes
- [ ] Commits contain only `.mmm/` directory changes
- [ ] Commit messages follow the specified format
- [ ] No commits created when analysis produces no changes
- [ ] Git errors are handled gracefully without stopping analysis
- [ ] Performance impact is less than 100ms per analysis
- [ ] Works correctly in both main branch and worktree contexts
- [ ] Existing workflows continue to function without modification

## Technical Details

### Implementation Approach

1. **Add Commit Logic to Analyzer**
   - Extend `ProjectAnalyzer::save_analysis()` in `src/context/mod.rs`
   - Check for changes after saving analysis files
   - Create commit if changes detected

2. **Git Operations Module**
   - Add `commit_analysis_changes()` function to `src/cook/git_ops.rs`
   - Use existing git command infrastructure
   - Implement atomic staging and committing

3. **Change Detection**
   - Use `git status --porcelain` to detect `.mmm/` changes
   - Filter for only analysis-related files
   - Skip commit if no relevant changes

4. **Commit Message Generation**
   - Extract metadata from `AnalysisMetadata` struct
   - Format duration in human-readable form
   - Include incremental vs full analysis flag

### Code Structure

```rust
// In src/cook/git_ops.rs
pub async fn commit_analysis_changes(metadata: &AnalysisMetadata) -> Result<bool> {
    // Check for changes in .mmm/
    let changes = get_mmm_changes().await?;
    if changes.is_empty() {
        return Ok(false);
    }
    
    // Stage .mmm/ changes
    stage_mmm_directory().await?;
    
    // Generate commit message
    let message = format_analysis_commit_message(metadata);
    
    // Create commit
    create_commit(&message).await?;
    
    Ok(true)
}

// In src/context/mod.rs
async fn save_analysis(project_path: &Path, result: &AnalysisResult) -> Result<()> {
    // ... existing save logic ...
    
    // Auto-commit if enabled
    if should_auto_commit() {
        if let Err(e) = git_ops::commit_analysis_changes(&result.metadata).await {
            eprintln!("Warning: Failed to commit analysis changes: {}", e);
            // Continue despite commit failure
        }
    }
    
    Ok(())
}
```

## Dependencies

- **Spec 44**: Provides the analysis infrastructure to be committed
- **Spec 19**: Establishes git-native workflow patterns

## Testing Strategy

1. **Unit Tests**
   - Test commit message generation
   - Test change detection logic
   - Test error handling paths

2. **Integration Tests**
   - Test full analysis → commit flow
   - Test with various git states
   - Test in worktree contexts
   - Test with no changes scenario

3. **Manual Testing**
   - Verify commits in real projects
   - Check git history cleanliness
   - Test with concurrent operations

## Documentation Requirements

- Update ARCHITECTURE.md to document auto-commit behavior
- Add configuration option to disable auto-commit
- Document commit message format
- Add troubleshooting guide for commit failures

## Implementation Notes

1. **Configuration Option**
   - Add `--no-auto-commit` flag to disable
   - Environment variable `MMM_NO_AUTO_COMMIT`
   - Respect CI environment detection

2. **Commit Timing**
   - Commit after all analysis files are saved
   - Don't commit partial analysis results
   - Handle interruption gracefully

3. **Git History Considerations**
   - One commit per analysis run
   - No commit amending or rewriting
   - Preserve linear history

## Migration and Compatibility

- Feature is opt-out by default
- Existing projects work without changes
- No migration needed
- Compatible with all MMM versions

## Future Enhancements

1. **Analysis History Viewing**
   - Add `mmm analyze history` command
   - Show analysis evolution over time
   - Compare analysis snapshots

2. **Metric Tracking**
   - Track improvement velocity
   - Visualize code quality trends
   - Generate analysis reports

3. **Smart Commit Grouping**
   - Batch related analysis updates
   - Squash rapid successive analyses
   - Intelligent commit scheduling