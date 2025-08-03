---
number: 46
title: Unified Analysis Function
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-08-03
---

# Specification 46: Unified Analysis Function

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, MMM has two completely separate code paths for running project analysis:

1. **Command-line path (`mmm analyze`)**: Uses `MetricsCollector::new()` and `ProjectAnalyzer::new()` directly with its own subprocess injection and display logic
2. **Workflow/orchestrator path**: Uses `MetricsCoordinator` and `AnalysisCoordinator` traits with different implementations and error handling

This duplication leads to:
- Inconsistent behavior between standalone analysis and workflow analysis
- Maintenance burden of keeping two implementations in sync
- Bugs that only appear in one path (like the recent `futures::executor::block_on` hanging issue)
- Confusion about which implementation to modify when adding features
- Different output formats and error handling between the two paths

## Objective

Create a single, unified analysis function that both the command-line and workflow paths can use, ensuring consistent behavior, reducing code duplication, and simplifying maintenance.

## Requirements

### Functional Requirements

1. **Single Analysis Entry Point**
   - Create a unified `run_analysis()` function that handles both metrics and context analysis
   - Support configuration options for output format, verbosity, and commit behavior
   - Provide consistent progress reporting interface
   - Handle subprocess injection for testing

2. **Consistent Behavior**
   - Ensure identical analysis results regardless of invocation method
   - Use the same subprocess handling and error recovery
   - Apply the same caching and optimization strategies
   - Maintain consistent timing and progress reporting

3. **Flexible Configuration**
   - Support different output formats (json, pretty, summary)
   - Allow selective analysis (metrics-only, context-only, or both)
   - Configure commit behavior (auto-commit, no-commit, or prompt)
   - Support force-refresh and cache management options

4. **Backward Compatibility**
   - Maintain existing command-line interface
   - Preserve workflow analysis behavior
   - Keep existing trait interfaces for extensibility
   - Support existing configuration files

### Non-Functional Requirements

1. **Performance**
   - No performance regression compared to current implementation
   - Maintain existing parallelization strategies
   - Preserve caching effectiveness
   - Optimize for minimal overhead

2. **Testability**
   - Support dependency injection for testing
   - Enable mocking of subprocess operations
   - Allow isolated testing of analysis components
   - Provide test utilities for common scenarios

3. **Maintainability**
   - Clear separation of concerns
   - Well-documented interfaces
   - Consistent error handling patterns
   - Easy to extend with new analysis types

## Acceptance Criteria

- [ ] Single `UnifiedAnalysis` module created with `run_analysis()` function
- [ ] Both `mmm analyze` command and workflow orchestrator use the same function
- [ ] All existing tests pass without modification
- [ ] No duplicate code between command and workflow paths
- [ ] Consistent output format across all invocation methods
- [ ] Performance benchmarks show no regression
- [ ] Documentation updated to reflect unified approach
- [ ] Integration tests verify identical behavior between paths

## Technical Details

### Implementation Approach

1. **Create Unified Analysis Module**
   ```rust
   // src/analysis/unified.rs
   pub struct AnalysisConfig {
       pub output_format: OutputFormat,
       pub save_results: bool,
       pub commit_changes: bool,
       pub force_refresh: bool,
       pub run_metrics: bool,
       pub run_context: bool,
       pub verbose: bool,
   }

   pub async fn run_analysis(
       project_path: &Path,
       config: AnalysisConfig,
       subprocess: SubprocessManager,
       progress: Arc<dyn ProgressReporter>,
   ) -> Result<AnalysisResults> {
       // Unified implementation
   }
   ```

2. **Progress Reporting Interface**
   ```rust
   #[async_trait]
   pub trait ProgressReporter: Send + Sync {
       fn display_progress(&self, message: &str);
       fn display_info(&self, message: &str);
       fn display_warning(&self, message: &str);
       fn display_success(&self, message: &str);
   }
   ```

3. **Results Structure**
   ```rust
   pub struct AnalysisResults {
       pub metrics: Option<ImprovementMetrics>,
       pub context: Option<AnalysisResult>,
       pub suggestions: Vec<ImprovementSuggestion>,
       pub health_score: f64,
       pub timing: AnalysisTiming,
   }
   ```

### Architecture Changes

1. **Module Organization**
   - Move unified analysis to `src/analysis/unified.rs`
   - Keep existing traits for extensibility
   - Adapt command and orchestrator to use unified function
   - Consolidate display logic into formatters

2. **Dependency Flow**
   ```
   mmm analyze command ─┐
                       ├─> UnifiedAnalysis::run_analysis()
   workflow orchestrator ─┘
   ```

3. **Error Handling**
   - Unified error types for analysis failures
   - Consistent retry logic for transient failures
   - Clear error messages with recovery suggestions

### Data Structures

1. **Configuration**
   - Unified configuration structure for all analysis options
   - Builder pattern for easy configuration
   - Sensible defaults for common use cases

2. **Results**
   - Combined results structure with optional components
   - Timing information for performance monitoring
   - Structured suggestions for improvements

### APIs and Interfaces

1. **Public API**
   ```rust
   pub use analysis::unified::{
       run_analysis,
       AnalysisConfig,
       AnalysisResults,
       ProgressReporter,
   };
   ```

2. **Trait Adaptors**
   - Implement existing traits using unified function
   - Maintain backward compatibility
   - Allow gradual migration

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/analyze/command.rs`
  - `src/cook/orchestrator.rs`
  - `src/cook/analysis/`
  - `src/metrics/`
- **External Dependencies**: No new dependencies

## Testing Strategy

- **Unit Tests**: Test configuration parsing and result formatting
- **Integration Tests**: Verify identical behavior between old and new paths
- **Performance Tests**: Benchmark analysis time and memory usage
- **Regression Tests**: Ensure no behavior changes in existing commands

## Documentation Requirements

- **Code Documentation**: Document all public APIs and configuration options
- **User Documentation**: Update command help text and README
- **Architecture Updates**: Document the unified analysis flow
- **Migration Guide**: Steps for extending analysis with new components

## Implementation Notes

1. **Phased Migration**
   - Phase 1: Create unified function with existing implementations
   - Phase 2: Migrate command-line path
   - Phase 3: Migrate workflow path
   - Phase 4: Remove duplicate code

2. **Subprocess Handling**
   - Ensure all async calls properly await results
   - Avoid blocking operations in async context
   - Handle subprocess timeouts gracefully

3. **Caching Strategy**
   - Unified cache key generation
   - Consistent cache invalidation
   - Shared cache between invocation methods

## Migration and Compatibility

- No breaking changes to public APIs
- Existing configuration files remain valid
- Command-line interface unchanged
- Workflow definitions continue to work