# Specification 46: Real Metrics Tracking

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [Spec 10: Smart Project Analyzer, Spec 11: Simple State Management]

## Context

Currently, MMM operates without quantitative feedback about the improvements it makes. While it can analyze code and make changes, it lacks objective metrics to measure whether those changes actually improve code quality, performance, or maintainability. This makes it impossible to determine if iterations are making meaningful progress or to set data-driven improvement goals.

Real metrics tracking would enable MMM to make informed decisions about what to improve next, validate that changes are beneficial, and provide users with concrete evidence of improvement. For Rust projects specifically, tools like rust-code-analysis can provide comprehensive metrics about code complexity, quality, and structure.

## Objective

Implement a comprehensive metrics tracking system for Rust projects that measures code quality, performance, complexity, and progress throughout improvement iterations, enabling data-driven decision making and validation of improvements.

## Requirements

### Functional Requirements

1. **Code Quality Metrics**
   - Test coverage percentage tracking using cargo-tarpaulin or similar
   - Type coverage analysis for generic and concrete types
   - Lint warning counts from clippy and rustc
   - Code duplication detection and measurement
   - Documentation coverage tracking

2. **Performance Metrics**
   - Benchmark results tracking using cargo-bench
   - Memory usage profiling for key operations
   - Compilation time measurements
   - Binary size tracking
   - Runtime performance indicators

3. **Complexity Metrics**
   - Cyclomatic complexity per function/module
   - Cognitive complexity measurements
   - Nesting depth analysis
   - Lines of code metrics (LOC, SLOC)
   - Dependency complexity scores

4. **Progress Tracking**
   - Number of bugs fixed (from TODO/FIXME removal)
   - Features added (new public APIs)
   - Technical debt score calculation
   - Improvement velocity over iterations
   - Regression detection

5. **Integration with rust-code-analysis**
   - Use rust-code-analysis for comprehensive metrics
   - Extract function-level complexity scores
   - Analyze code structure and patterns
   - Generate maintainability indices

### Non-Functional Requirements

- **Performance**: Metrics collection should add < 30 seconds to iteration time
- **Accuracy**: Metrics must be reliable and reproducible
- **Storage**: Efficient storage of historical metrics data
- **Rust-Focused**: Initial implementation targets Rust projects only
- **Extensibility**: Architecture allows adding metrics for other languages later

## Acceptance Criteria

- [ ] Test coverage is accurately measured and tracked over iterations
- [ ] Complexity metrics are calculated for all Rust functions
- [ ] Performance benchmarks are automatically run and recorded
- [ ] Metrics are stored with each iteration for historical analysis
- [ ] Metrics influence improvement decisions in subsequent iterations
- [ ] Dashboard or report shows metrics trends over time
- [ ] Integration with rust-code-analysis provides detailed code analysis
- [ ] Metrics collection adds less than 30 seconds per iteration

## Technical Details

### Implementation Approach

1. **Metrics Collection Pipeline**
   ```rust
   pub struct MetricsCollector {
       quality_analyzer: QualityAnalyzer,
       performance_profiler: PerformanceProfiler,
       complexity_calculator: ComplexityCalculator,
       progress_tracker: ProgressTracker,
   }
   
   impl MetricsCollector {
       pub async fn collect_metrics(&self, project_path: &Path) -> Result<ImprovementMetrics> {
           // Run all analyzers in parallel
           let (quality, performance, complexity, progress) = tokio::join!(
               self.quality_analyzer.analyze(project_path),
               self.performance_profiler.profile(project_path),
               self.complexity_calculator.calculate(project_path),
               self.progress_tracker.track(project_path),
           );
           
           Ok(ImprovementMetrics {
               quality: quality?,
               performance: performance?,
               complexity: complexity?,
               progress: progress?,
               timestamp: Utc::now(),
           })
       }
   }
   ```

2. **Data Structure**
   ```rust
   pub struct ImprovementMetrics {
       // Code quality
       pub test_coverage: f32,
       pub type_coverage: f32,
       pub lint_warnings: u32,
       pub code_duplication: f32,
       pub doc_coverage: f32,
       
       // Performance
       pub benchmark_results: HashMap<String, Duration>,
       pub memory_usage: HashMap<String, Bytes>,
       pub compile_time: Duration,
       pub binary_size: Bytes,
       
       // Complexity
       pub cyclomatic_complexity: HashMap<String, u32>,
       pub cognitive_complexity: HashMap<String, u32>,
       pub max_nesting_depth: u32,
       pub total_lines: u32,
       
       // Progress
       pub bugs_fixed: u32,
       pub features_added: u32,
       pub tech_debt_score: f32,
       pub improvement_velocity: f32,
       
       // Metadata
       pub timestamp: DateTime<Utc>,
       pub iteration_id: String,
   }
   ```

3. **Storage Schema**
   ```
   .mmm/metrics/
   ├── current.json          # Latest metrics snapshot
   ├── history.json          # Time series data
   ├── benchmarks/           # Benchmark result files
   └── reports/              # Generated reports
   ```

### Architecture Changes

- New `metrics` module in the cook workflow
- Integration points in the improvement loop
- Enhanced state management to include metrics history
- Modified decision logic to use metrics data

### Data Structures

```rust
pub struct MetricsHistory {
    pub snapshots: Vec<MetricsSnapshot>,
    pub trends: MetricsTrends,
    pub baselines: MetricsBaselines,
}

pub struct MetricsSnapshot {
    pub metrics: ImprovementMetrics,
    pub iteration: u32,
    pub commit_sha: String,
}

pub struct MetricsTrends {
    pub coverage_trend: Trend,
    pub complexity_trend: Trend,
    pub performance_trend: Trend,
    pub quality_trend: Trend,
}

pub enum Trend {
    Improving(f32),    // Percentage improvement
    Stable,
    Degrading(f32),    // Percentage degradation
}
```

### APIs and Interfaces

```rust
pub trait MetricsAnalyzer {
    async fn analyze(&self, project_path: &Path) -> Result<MetricsData>;
    fn get_baseline(&self) -> Option<MetricsData>;
    fn compare_with_baseline(&self, current: &MetricsData) -> Comparison;
}

pub trait MetricsReporter {
    fn generate_report(&self, history: &MetricsHistory) -> String;
    fn get_summary(&self, current: &ImprovementMetrics) -> String;
    fn export_dashboard(&self, path: &Path) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 10: Smart Project Analyzer (for project detection)
  - Spec 11: Simple State Management (for metrics storage)
- **Affected Components**: 
  - Cook workflow will collect metrics after each iteration
  - Decision logic will use metrics to guide improvements
  - State management will store metrics history
- **External Dependencies**: 
  - rust-code-analysis for code metrics
  - cargo-tarpaulin for test coverage (optional)
  - cargo-bench for performance benchmarks

## Testing Strategy

- **Unit Tests**: Test each metrics analyzer independently
- **Integration Tests**: Verify full metrics collection pipeline
- **Accuracy Tests**: Validate metrics against known values
- **Performance Tests**: Ensure metrics collection stays under 30s
- **Regression Tests**: Detect metrics degradation between iterations

## Documentation Requirements

- **Code Documentation**: Document metrics calculation methods
- **User Documentation**: Explain metrics and how to interpret them
- **Configuration Guide**: Document metrics customization options
- **API Documentation**: Document public metrics interfaces

## Implementation Notes

1. **Rust-First**: Focus exclusively on Rust projects initially
2. **Tool Integration**: Leverage existing Rust ecosystem tools
3. **Incremental Rollout**: Start with basic metrics, add more over time
4. **Caching**: Cache expensive metrics calculations when possible
5. **Graceful Degradation**: If a metric fails, continue with others

## Migration and Compatibility

- **Backward Compatible**: MMM continues to work without metrics
- **Optional Feature**: Metrics collection can be disabled
- **Gradual Adoption**: Projects can opt-in to metrics tracking
- **Data Portability**: Metrics data exports to standard formats