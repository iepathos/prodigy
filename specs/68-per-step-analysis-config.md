---
number: 68
title: Per-Step Analysis Configuration in Workflows
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-08-03
---

# Specification 68: Per-Step Analysis Configuration in Workflows

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: []

## Context

Currently, MMM's cook workflow system runs a single analysis phase at the beginning of workflow execution. While the `--metrics` flag allows enabling metrics collection for the entire workflow, some workflows have mixed requirements where:

1. Certain commands (like `mmm-code-review`, `mmm-cleanup-tech-debt`) need fresh metrics data
2. Other commands only need context analysis (dependencies, architecture, conventions)
3. Some steps might modify code significantly, requiring re-analysis before subsequent steps
4. Running full metrics analysis for every workflow is slow and often unnecessary

The current limitation means workflows cannot optimize their analysis requirements per step, leading to either:
- Running expensive metrics analysis when not needed (with `--metrics`)
- Missing required metrics data for specific commands (without `--metrics`)
- Stale analysis data after significant code modifications mid-workflow

## Objective

Enable workflow configurations to specify per-step analysis requirements, allowing commands to request context analysis, metrics analysis, or both before execution. This provides fine-grained control over when analysis runs, optimizing performance while ensuring commands have the data they need.

## Requirements

### Functional Requirements

1. **Per-Step Analysis Configuration**
   - Allow each workflow step to specify required analysis types
   - Support three analysis modes: `context`, `metrics`, or `all`
   - Analysis should run before the step if requested
   - Analysis results must be saved to `.mmm/` before step execution

2. **Conditional Analysis Execution**
   - Skip analysis if recently run (within configurable time window)
   - Force re-analysis if explicitly requested
   - Detect significant code changes that invalidate cached analysis

3. **Backward Compatibility**
   - Existing workflows without analysis config continue to work
   - Global `--metrics` flag still controls initial analysis
   - Legacy string command format remains supported

4. **Performance Optimization**
   - Incremental analysis when possible
   - Parallel execution of context and metrics analysis
   - Reuse analysis results across steps when appropriate

### Non-Functional Requirements

- Analysis execution must not block user interaction
- Clear progress indicators during analysis phases
- Minimal impact on workflow execution time when analysis not needed
- Consistent analysis data format across all execution modes

## Acceptance Criteria

- [ ] Workflow YAML supports `analysis` field in command metadata
- [ ] Analysis runs before step execution when specified
- [ ] Context-only analysis completes in <5 seconds for typical projects
- [ ] Metrics analysis can be selectively enabled per step
- [ ] Analysis results are committed to git when in worktree mode
- [ ] Existing workflows continue functioning without modification
- [ ] Documentation updated with new configuration options
- [ ] Integration tests cover mixed analysis workflows

## Technical Details

### Implementation Approach

1. Extend `CommandMetadata` struct with analysis configuration:
```rust
pub struct CommandMetadata {
    // ... existing fields ...
    
    /// Analysis requirements for this command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Type of analysis needed: "context", "metrics", or "all"
    pub analysis_type: String,
    
    /// Force fresh analysis even if cached
    #[serde(default)]
    pub force_refresh: bool,
    
    /// Maximum age of cached analysis in seconds
    #[serde(default = "default_cache_duration")]
    pub max_cache_age: u64,
}
```

2. Update workflow executor to check analysis requirements:
```rust
// In WorkflowExecutor::execute_command
if let Some(analysis_config) = &command.metadata.analysis {
    self.run_analysis_if_needed(env, analysis_config).await?;
}
```

3. Implement smart analysis execution:
```rust
async fn run_analysis_if_needed(
    &self,
    env: &ExecutionEnvironment,
    config: &AnalysisConfig,
) -> Result<()> {
    // Check cache age and force_refresh
    // Run appropriate analysis type
    // Save results with commit if in worktree
}
```

### Architecture Changes

1. **Analysis Coordinator Enhancement**
   - Add cache age checking
   - Support partial analysis updates
   - Implement analysis type selection

2. **Workflow Executor Changes**
   - Parse analysis configuration from YAML
   - Inject analysis execution before command runs
   - Track analysis state across workflow execution

3. **Progress Reporting**
   - Add analysis progress to user interaction displays
   - Show which analysis type is running
   - Report cache hits/misses

### Data Structures

```yaml
# Example workflow with per-step analysis
commands:
  - name: mmm-implement-spec
    args: ["$ARG"]
    # No analysis needed, uses initial context
  
  - name: mmm-lint
    commit_required: false
    # No analysis needed for linting
  
  - name: mmm-code-review
    metadata:
      analysis:
        analysis_type: "all"  # Needs both context and metrics
        max_cache_age: 300   # 5 minutes
  
  - name: mmm-cleanup-tech-debt
    metadata:
      analysis:
        analysis_type: "metrics"  # Only needs fresh metrics
        force_refresh: true      # Always run fresh
```

### APIs and Interfaces

No external API changes. Internal interfaces between workflow executor and analysis coordinator will be enhanced to support analysis configuration.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `cook::workflow::executor`
  - `cook::analysis::runner`
  - `config::command`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Command metadata parsing with analysis config
  - Cache age calculation and validation
  - Analysis type selection logic

- **Integration Tests**: 
  - Workflow with mixed analysis requirements
  - Cache invalidation scenarios
  - Performance benchmarks for analysis overhead

- **Performance Tests**: 
  - Measure overhead of analysis checks
  - Validate incremental analysis performance
  - Test parallel analysis execution

- **User Acceptance**: 
  - Run existing workflows to ensure compatibility
  - Test new workflows with per-step analysis
  - Validate analysis data freshness

## Documentation Requirements

- **Code Documentation**: 
  - Document AnalysisConfig struct and fields
  - Add examples in workflow executor
  - Update command metadata docs

- **User Documentation**: 
  - Add "Analysis Configuration" section to cookbook
  - Provide example workflows with analysis config
  - Document performance considerations

- **Architecture Updates**: 
  - Update workflow execution flow diagram
  - Document analysis caching strategy

## Implementation Notes

1. **Cache Key Design**: Use project path + analysis type + file modification times
2. **Incremental Analysis**: Reuse dependency graph when only few files changed
3. **Progress UI**: Show analysis type and elapsed time during execution
4. **Error Handling**: Gracefully degrade if analysis fails (warn but continue)
5. **Git Integration**: Respect `--no-commit` flag for analysis commits

## Migration and Compatibility

- No breaking changes to existing workflows
- Analysis field is optional and defaults to no per-step analysis
- Global `--metrics` flag behavior unchanged
- Gradual adoption path - teams can update workflows incrementally