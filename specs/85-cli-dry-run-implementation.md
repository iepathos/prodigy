---
number: 85
title: CLI Dry-Run Implementation
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 85: CLI Dry-Run Implementation

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The CLI events command includes a `--dry-run` flag intended to preview cleanup operations without actually performing them. However, the implementation in `src/cli/events.rs` (lines 1083, 1110) contains TODO comments indicating that proper dry-run analysis is not implemented. Instead, the code simply prints placeholder messages.

This missing functionality prevents users from:
- Safely previewing what would be deleted before cleanup
- Understanding the impact of cleanup operations
- Validating cleanup criteria before execution
- Building confidence in automated cleanup processes

The dry-run feature is particularly important for operations that delete data, as it allows users to verify the operation's scope before committing to potentially destructive actions.

## Objective

Implement comprehensive dry-run functionality for CLI commands that modify or delete data, providing users with detailed previews of operations before execution.

## Requirements

### Functional Requirements

1. **Event Cleanup Dry-Run**
   - Analyze which events would be deleted
   - Show count and size of files to be removed
   - Display date ranges of affected events
   - List affected job IDs and workflows

2. **Generic Dry-Run Framework**
   - Reusable dry-run infrastructure for all CLI commands
   - Consistent output format across commands
   - Detailed vs summary output modes
   - Machine-readable output option (JSON)

3. **Operation Preview**
   - Show exact files/directories to be modified
   - Display before/after state comparisons
   - Estimate operation duration
   - Show space to be freed

4. **Safety Features**
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

- [ ] Dry-run mode accurately predicts cleanup operations
- [ ] File counts and sizes are correctly calculated
- [ ] Date ranges and job IDs are properly identified
- [ ] JSON output mode provides machine-readable data
- [ ] Confirmation prompts work correctly
- [ ] Large-scale operations show appropriate warnings
- [ ] Dry-run adds <100ms overhead to command execution
- [ ] All existing CLI tests pass
- [ ] New tests validate dry-run accuracy
- [ ] Documentation includes dry-run usage examples

## Technical Details

### Implementation Approach

1. **Replace TODO Implementation**
   ```rust
   // Current (lines 1083, 1110)
   // TODO: Implement proper dry-run analysis

   // New implementation
   fn analyze_cleanup_dry_run(&self, older_than: Duration) -> Result<CleanupAnalysis> {
       let mut analysis = CleanupAnalysis::default();

       let event_dir = self.get_event_directory()?;
       let cutoff_time = SystemTime::now() - older_than;

       for entry in WalkDir::new(event_dir) {
           let entry = entry?;
           let metadata = entry.metadata()?;

           if metadata.modified()? < cutoff_time {
               analysis.files_to_delete.push(entry.path().to_path_buf());
               analysis.total_size += metadata.len();
               analysis.update_date_range(&metadata);
               analysis.extract_job_info(entry.path());
           }
       }

       Ok(analysis)
   }
   ```

2. **Generic Dry-Run Framework**
   ```rust
   pub trait DryRunnable {
       type Operation;
       type Analysis;

       fn analyze(&self, op: &Self::Operation) -> Result<Self::Analysis>;

       fn format_analysis(&self, analysis: &Self::Analysis) -> String;

       fn to_json(&self, analysis: &Self::Analysis) -> Result<Value>;

       fn requires_confirmation(&self, analysis: &Self::Analysis) -> bool;
   }

   pub struct DryRunExecutor<T: DryRunnable> {
       command: T,
       output_format: OutputFormat,
       force: bool,
   }

   impl<T: DryRunnable> DryRunExecutor<T> {
       pub async fn execute(&self, op: T::Operation) -> Result<()> {
           let analysis = self.command.analyze(&op)?;

           match self.output_format {
               OutputFormat::Human => {
                   println!("{}", self.command.format_analysis(&analysis));
               }
               OutputFormat::Json => {
                   println!("{}", serde_json::to_string_pretty(
                       &self.command.to_json(&analysis)?
                   )?);
               }
           }

           if self.command.requires_confirmation(&analysis) && !self.force {
               if !self.prompt_confirmation()? {
                   return Ok(());
               }
           }

           // Proceed with actual operation if not dry-run
           Ok(())
       }
   }
   ```

3. **Detailed Analysis Output**
   ```rust
   pub struct CleanupAnalysis {
       pub files_to_delete: Vec<PathBuf>,
       pub total_size: u64,
       pub oldest_file: Option<SystemTime>,
       pub newest_file: Option<SystemTime>,
       pub affected_jobs: HashSet<String>,
       pub affected_workflows: HashSet<String>,
       pub warnings: Vec<String>,
   }

   impl CleanupAnalysis {
       pub fn format_human(&self) -> String {
           format!(
               r#"
   Cleanup Analysis (DRY RUN)
   ========================
   Files to delete: {}
   Total size: {}
   Date range: {} to {}
   Affected jobs: {}
   Affected workflows: {}

   Warnings:
   {}

   Files:
   {}
               "#,
               self.files_to_delete.len(),
               format_bytes(self.total_size),
               format_time(self.oldest_file),
               format_time(self.newest_file),
               self.affected_jobs.len(),
               self.affected_workflows.len(),
               self.warnings.join("\n"),
               self.format_file_list()
           )
       }
   }
   ```

### Architecture Changes

- Add generic `DryRunnable` trait for commands
- Create `DryRunExecutor` for consistent handling
- Implement analysis types for different operations
- Add formatting utilities for human-readable output

### Data Structures

```rust
pub enum OutputFormat {
    Human,
    Json,
    Yaml,
    Table,
}

pub struct DryRunConfig {
    pub enabled: bool,
    pub verbose: bool,
    pub format: OutputFormat,
    pub show_warnings: bool,
    pub require_confirmation: bool,
}

pub struct OperationImpact {
    pub files_affected: usize,
    pub size_affected: u64,
    pub reversible: bool,
    pub risk_level: RiskLevel,
    pub estimated_duration: Duration,
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

### APIs and Interfaces

```rust
pub trait DryRunCommand {
    fn dry_run(&self, config: &DryRunConfig) -> Result<()>;

    fn analyze_impact(&self) -> Result<OperationImpact>;

    fn format_impact(&self, impact: &OperationImpact) -> String;
}

impl DryRunCommand for EventsCleanupCommand {
    // Implementation
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

- Start with events cleanup command as the prototype
- Design for reusability across all CLI commands
- Consider adding --dry-run to all destructive operations
- Implement progress bars for long analyses
- Add color coding for different risk levels
- Consider implementing a global --dry-run flag
- Plan for future undo/rollback functionality

## Migration and Compatibility

- Existing commands continue to work without dry-run
- Dry-run is opt-in via --dry-run flag
- No breaking changes to command interfaces
- Consider adding dry-run to existing commands gradually