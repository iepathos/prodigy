---
number: 85
title: CLI Dry-Run Implementation
category: foundation
priority: medium
status: ready
dependencies: []
created: 2025-09-17
updated: 2025-09-18
---

# Specification 85: CLI Dry-Run Implementation

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The CLI events command includes a `--dry-run` flag intended to preview cleanup operations without actually performing them. The implementation in `src/cli/events.rs` (lines 1083, 1110) contains TODO comments indicating that proper dry-run analysis is not implemented. Instead, the code simply prints placeholder messages.

However, the codebase already has strong foundations:
- A comprehensive `RetentionManager` in `src/cook/execution/events/retention.rs` that handles policy-based cleanup
- A `RetentionStats` structure that tracks cleanup operations
- Existing `UserPrompter` trait in `src/cook/interaction/prompts.rs` for user confirmations
- Working retention policy implementation with archiving support

This missing dry-run functionality prevents users from:
- Safely previewing what would be deleted before cleanup
- Understanding the impact of cleanup operations
- Validating cleanup criteria before execution
- Building confidence in automated cleanup processes

The dry-run feature is particularly important for operations that delete data, as it allows users to verify the operation's scope before committing to potentially destructive actions.

## Objective

Implement comprehensive dry-run functionality for CLI commands and workflow execution that modify or delete data, providing users with detailed previews of operations before execution, while properly handling commit requirements in dry-run mode.

## Requirements

### Functional Requirements

1. **Event Cleanup Dry-Run**
   - Analyze which events would be deleted
   - Show count and size of files to be removed
   - Display date ranges of affected events
   - List affected job IDs and workflows

2. **Workflow Execution Dry-Run**
   - Support dry-run mode for workflow commands (claude, shell, git, etc.)
   - Handle `commit_required` validation in dry-run mode
   - Show what commands would execute without actually running them
   - Display assumed outcomes for validation purposes

3. **Commit Requirement Handling**
   - When `commit_required: true` and dry-run is active:
     - Note that commit is required
     - Display: "Would create commit from: [command]"
     - Assume commit would be created for validation
     - Continue workflow execution without failing
   - Track which commands would have created commits
   - Show summary of expected commits at end

4. **Generic Dry-Run Framework**
   - Reusable dry-run infrastructure for all CLI commands
   - Consistent output format across commands
   - Detailed vs summary output modes
   - Machine-readable output option (JSON)

5. **Operation Preview**
   - Show exact files/directories to be modified
   - Display before/after state comparisons
   - Estimate operation duration
   - Show space to be freed

6. **Safety Features**
   - Require confirmation for destructive operations
   - Show warnings for large-scale changes
   - Highlight irreversible operations
   - Provide undo information where possible

### Non-Functional Requirements

1. **Performance**
   - Fast analysis without actual operations
   - Minimal overhead for dry-run mode
   - Efficient file system scanning

2. **Usability**
   - Clear, readable output formatting
   - Color-coded information (if terminal supports)
   - Progress indicators for long analyses

3. **Reliability**
   - Accurate predictions of operations
   - Handle permission issues gracefully
   - Report potential failures

## Acceptance Criteria

### Phase 1 (Required)
- [ ] TODOs in `src/cli/events.rs` are replaced with working code
- [ ] Dry-run mode shows what would be deleted without actually deleting
- [ ] Event counts and file sizes are displayed
- [ ] Dry-run uses existing `RetentionManager` logic
- [ ] Workflow dry-run handles `commit_required` without failing
- [ ] ExecutionContext properly propagates dry-run mode
- [ ] Commands return dry-run results instead of executing
- [ ] All existing CLI tests pass

### Phase 2 (Nice to Have)
- [ ] JSON output mode provides machine-readable data via `--output-format`
- [ ] Analysis includes affected job IDs and date ranges
- [ ] Large-scale operations show appropriate warnings
- [ ] Documentation includes dry-run usage examples

### Phase 3 (Future Enhancement)
- [ ] Confirmation prompts for high-risk operations
- [ ] Risk level assessment and display
- [ ] Generic dry-run framework for other commands
- [ ] Dry-run adds <100ms overhead to command execution

## Technical Details

### Implementation Approach

The implementation should handle both CLI commands and workflow execution dry-run:

1. **Extend RetentionManager with Dry-Run Analysis**
   ```rust
   // Add to src/cook/execution/events/retention.rs
   impl RetentionManager {
       /// Perform dry-run analysis without modifying files
       pub async fn analyze_retention(&self) -> Result<RetentionAnalysis> {
           let mut analysis = RetentionAnalysis::default();

           if !self.events_path.exists() {
               return Ok(analysis);
           }

           // Get file metadata
           let metadata = fs::metadata(&self.events_path)?;
           analysis.original_size_bytes = metadata.len();

           // Analyze what would be cleaned without actually doing it
           self.analyze_cleanup(&mut analysis).await?;

           Ok(analysis)
       }
   }
   ```

2. **Workflow Dry-Run with Commit Handling**
   ```rust
   // In src/cook/workflow/executor.rs
   impl WorkflowExecutor {
       fn execute_command(&mut self, command: &Command, dry_run: bool) -> Result<()> {
           if dry_run {
               // Show what would execute
               println!("[DRY RUN] Would execute: {}", command);

               // Handle commit_required in dry-run
               if command.commit_required {
                   println!("[DRY RUN] Commit required - assuming commit created by: {}",
                           command.display_name());

                   // Set flag to skip actual commit validation
                   self.assumed_commits.push(command.clone());
               }

               // Return success without executing
               return Ok(());
           }

           // Normal execution path
           self.execute_actual_command(command)
       }

       fn validate_commits(&self, step: &WorkflowStep) -> Result<()> {
           // Skip validation if dry-run and commit was assumed
           if self.dry_run && self.assumed_commits.contains(&step.command) {
               println!("[DRY RUN] Skipping commit validation - assumed commit from: {}",
                       step.command.display_name());
               return Ok(());
           }

           // Normal validation
           self.validate_actual_commits(step)
       }
   }
   ```

3. **Replace TODO Implementation in CLI**
   ```rust
   // Replace TODO at lines 1083, 1110 in src/cli/events.rs
   if dry_run {
       let retention = RetentionManager::new(policy.clone(), event_file);
       let analysis = retention.analyze_retention().await?;

       // Display analysis based on output format
       match output_format {
           OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&analysis)?),
           OutputFormat::Human => analysis.display_human(),
       }

       total_cleaned += analysis.events_to_remove;
       if policy.archive_old_events {
           total_archived += analysis.events_to_archive;
       }
   }
   ```

4. **ExecutionContext Dry-Run Propagation**
   ```rust
   // In src/cook/workflow/executor.rs
   let mut exec_context = ExecutionContext::new(env.working_dir.clone());

   // Propagate dry-run mode to context
   if self.dry_run {
       exec_context = exec_context.with_dry_run(true);
   }

   // Commands check context.dry_run to determine behavior
   ```

5. **Command Handler Updates**
   ```rust
   // Example in src/commands/handlers/claude.rs
   impl ClaudeHandler {
       async fn execute(&self, context: &ExecutionContext) -> Result<CommandResult> {
           if context.dry_run {
               // Return what would happen
               return CommandResult::success(json!({
                   "dry_run": true,
                   "command": "claude",
                   "prompt": self.prompt,
                   "message": "Would call Claude API with prompt"
               }));
           }

           // Normal execution
           self.execute_claude_api(context).await
       }
   }
   ```

6. **Generic Dry-Run Framework (Future Enhancement)**

   While not required for the initial implementation, a generic framework could be added later for reusability:

   ```rust
   pub trait DryRunnable {
       type Operation;
       type Analysis;

       fn analyze(&self, op: &Self::Operation) -> Result<Self::Analysis>;
       fn format_analysis(&self, analysis: &Self::Analysis) -> String;
       fn to_json(&self, analysis: &Self::Analysis) -> Result<Value>;
   }
   ```

7. **Analysis Output Structure**
   ```rust
   // Add to src/cook/execution/events/retention.rs
   pub struct RetentionAnalysis {
       pub file_path: PathBuf,
       pub events_total: usize,
       pub events_retained: usize,
       pub events_to_remove: usize,
       pub events_to_archive: usize,
       pub original_size_bytes: u64,
       pub projected_size_bytes: u64,
       pub space_to_save: u64,
       pub warnings: Vec<String>,
   }

   impl RetentionAnalysis {
       pub fn display_human(&self) {
           println!("Cleanup Analysis (DRY RUN)");
           println!("========================");
           println!("File: {}", self.file_path.display());
           println!("Total events: {}", self.events_total);
           println!("Events to retain: {}", self.events_retained);
           println!("Events to remove: {}", self.events_to_remove);
           if self.events_to_archive > 0 {
               println!("Events to archive: {}", self.events_to_archive);
           }
           println!("Space to save: {} bytes", self.space_to_save);

           if !self.warnings.is_empty() {
               println!("\nWarnings:");
               for warning in &self.warnings {
                   println!("  ⚠️  {}", warning);
               }
           }
       }
   }
   ```

### Architecture Changes

- Extend existing `RetentionManager` with `analyze_retention()` method
- Add `RetentionAnalysis` struct to `retention.rs`
- Update CLI to use analysis when `--dry-run` is specified
- Add `--output-format` flag to support JSON output
- Propagate dry-run mode through `ExecutionContext`
- Update workflow executor to handle `commit_required` in dry-run
- Track assumed commits for validation purposes

### Data Structures

The implementation will primarily use existing structures with minimal additions:

```rust
// Existing in retention.rs
pub struct RetentionPolicy { ... }
pub struct RetentionStats { ... }

// New addition for dry-run
pub struct RetentionAnalysis {
    // Fields as shown above
}

// Add to workflow executor
pub struct DryRunState {
    pub assumed_commits: Vec<String>,
    pub would_execute: Vec<String>,
    pub skipped_validations: Vec<String>,
}

// Add to CLI
pub enum OutputFormat {
    Human,
    Json,
}
```

### Dry-Run Output Examples

```text
# Workflow dry-run with commit_required
[DRY RUN] Starting workflow: implement-feature
[DRY RUN] Would execute: claude /implement src/feature.rs
[DRY RUN] Commit required - assuming commit created by: claude /implement
[DRY RUN] Would execute: shell cargo test
[DRY RUN] Would execute: git push origin feature-branch
[DRY RUN] Skipping commit validation - assumed commit from: claude /implement

Summary:
- Commands that would execute: 3
- Assumed commits: 1 (claude /implement)
- Validations skipped: 1
```

### APIs and Interfaces

```rust
// Extension to RetentionManager
impl RetentionManager {
    pub async fn analyze_retention(&self) -> Result<RetentionAnalysis>;
}

// CLI command update
#[derive(Args)]
pub struct CleanCommand {
    #[arg(long)]
    dry_run: bool,

    #[arg(long, default_value = "human")]
    output_format: Option<String>,

    // ... existing fields
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - CLI events command
  - Future CLI commands with destructive operations
  - Terminal output formatting
- **External Dependencies**:
  - `walkdir` for file system traversal
  - `colored` for terminal colors (optional)

## Testing Strategy

- **Unit Tests**:
  - Test analysis accuracy
  - Validate size calculations
  - Test date range detection
  - Verify job ID extraction

- **Integration Tests**:
  - End-to-end dry-run scenarios
  - Comparison with actual operations
  - Various output formats
  - Confirmation prompts

- **User Acceptance**:
  - Manual testing of dry-run output
  - Verify accuracy of predictions
  - Test different output formats
  - Validate confirmation flow

## Documentation Requirements

- **Code Documentation**:
  - Document DryRunnable trait
  - Add examples for implementing dry-run
  - Include formatting guidelines

- **User Documentation**:
  - Dry-run usage guide
  - Output format explanations
  - Examples for each command
  - Best practices

- **Architecture Updates**:
  - Document dry-run framework
  - Include implementation guide
  - Add sequence diagrams

## Implementation Notes

### Minimal Implementation Path

1. **Phase 1a: Workflow Dry-Run (Critical)**
   - Propagate dry-run mode through `ExecutionContext`
   - Update command handlers to check `context.dry_run`
   - Handle `commit_required` by assuming commits in dry-run
   - Skip commit validation when commits are assumed

2. **Phase 1b: Basic Dry-Run for Events**
   - Add `analyze_retention()` method to existing `RetentionManager`
   - Replace TODOs in `src/cli/events.rs` (lines 1083, 1110)
   - Add simple human-readable output
   - Use existing `RetentionPolicy` and file analysis logic

3. **Phase 2: Enhanced Output (Optional)**
   - Add `--output-format` flag with JSON support
   - Include more detailed analysis (affected jobs, date ranges)
   - Add warnings for large-scale operations
   - Show summary of assumed commits

4. **Phase 3: Safety Features (Optional)**
   - Integrate with existing `UserPrompter` for confirmations
   - Add risk level assessment
   - Show preview before actual cleanup in non-dry-run mode

### Key Considerations

- **Critical**: Must handle `commit_required` in dry-run to prevent workflow failures
- When dry-run is active and `commit_required: true`, assume success
- Display clear messaging about what commits would be created
- Leverage existing `RetentionManager` - don't reinvent the wheel
- Keep changes minimal and focused initially
- Generic framework can be extracted later if needed
- Use existing prompting infrastructure from `src/cook/interaction/prompts.rs`

## Migration and Compatibility

- Existing commands continue to work without dry-run
- Dry-run is opt-in via --dry-run flag
- No breaking changes to command interfaces
- Consider adding dry-run to existing commands gradually