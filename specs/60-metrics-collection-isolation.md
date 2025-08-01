# Specification 60: Metrics Collection Isolation

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [56-cook-orchestrator-refactor, 57-subprocess-abstraction-layer]

## Context

Metrics collection is currently embedded within the cook module's execution flow, making it difficult to:

- Test metrics collection independently
- Add new metrics without modifying core logic
- Mock metrics for testing other components
- Support different metrics backends
- Control metrics collection granularity

A properly isolated metrics system would improve testability and enable richer insights into MMM's operation.

## Objective

Extract metrics collection into an isolated, pluggable system with clear interfaces, comprehensive testing support, and the ability to add new metrics without modifying core execution logic.

## Requirements

### Functional Requirements
- Support all existing metrics collection
- Enable addition of new metrics without code changes
- Support multiple metrics backends (file, memory, remote)
- Provide metrics aggregation and querying
- Support both push and pull metrics models
- Enable fine-grained control over what metrics to collect

### Non-Functional Requirements
- Near-zero overhead when metrics are disabled
- Thread-safe metrics collection
- Async-compatible interfaces
- Support for high-frequency metrics
- Minimal memory footprint for long-running sessions

## Acceptance Criteria

- [ ] Metrics collection completely isolated from execution logic
- [ ] Plugin system for adding new metrics collectors
- [ ] 95% test coverage for metrics system
- [ ] Support for at least 3 metrics backends
- [ ] Performance overhead less than 1% when enabled
- [ ] Zero overhead when disabled
- [ ] Rich querying API for metrics data

## Technical Details

### Implementation Approach

1. **Core Metrics Traits**
   ```rust
   // Metrics event
   #[derive(Debug, Clone)]
   pub enum MetricEvent {
       Counter { name: String, value: i64, tags: Tags },
       Gauge { name: String, value: f64, tags: Tags },
       Timer { name: String, duration: Duration, tags: Tags },
       Custom { name: String, data: serde_json::Value, tags: Tags },
   }

   // Core trait for metrics collection
   #[async_trait]
   pub trait MetricsCollector: Send + Sync {
       async fn record(&self, event: MetricEvent) -> Result<()>;
       async fn flush(&self) -> Result<()>;
   }

   // Trait for metrics querying
   #[async_trait]
   pub trait MetricsReader: Send + Sync {
       async fn query(&self, query: MetricsQuery) -> Result<MetricsResult>;
       async fn aggregate(&self, aggregation: Aggregation) -> Result<AggregateResult>;
   }
   ```

2. **Metrics Registry Pattern**
   ```rust
   pub struct MetricsRegistry {
       collectors: Vec<Box<dyn MetricsCollector>>,
       config: MetricsConfig,
   }

   impl MetricsRegistry {
       pub fn new(config: MetricsConfig) -> Self { /* ... */ }
       pub fn register(&mut self, collector: Box<dyn MetricsCollector>) { /* ... */ }
       
       // Convenience methods
       pub async fn increment(&self, name: &str, tags: Tags) { /* ... */ }
       pub async fn gauge(&self, name: &str, value: f64, tags: Tags) { /* ... */ }
       pub async fn time<F, R>(&self, name: &str, tags: Tags, f: F) -> Result<R>
       where
           F: Future<Output = Result<R>>,
       { /* ... */ }
   }
   ```

3. **Structured Metrics**
   ```rust
   // Session metrics
   pub struct SessionMetrics {
       pub session_id: String,
       pub start_time: DateTime<Utc>,
       pub iterations: Vec<IterationMetrics>,
       pub total_duration: Duration,
       pub final_status: SessionStatus,
   }

   // Iteration metrics
   pub struct IterationMetrics {
       pub iteration_number: u32,
       pub duration: Duration,
       pub commands_executed: usize,
       pub files_changed: usize,
       pub lines_added: usize,
       pub lines_removed: usize,
       pub errors: Vec<ErrorMetric>,
   }

   // Code quality metrics
   pub struct QualityMetrics {
       pub test_coverage: f64,
       pub lint_warnings: usize,
       pub complexity_score: f64,
       pub documentation_coverage: f64,
   }
   ```

### Architecture Changes

1. **Metrics Collectors**
   ```rust
   // File-based collector
   pub struct FileMetricsCollector {
       path: PathBuf,
       buffer: Arc<Mutex<Vec<MetricEvent>>>,
       flush_interval: Duration,
   }

   // In-memory collector for testing
   pub struct MemoryMetricsCollector {
       events: Arc<RwLock<Vec<MetricEvent>>>,
   }

   // Composite collector
   pub struct CompositeMetricsCollector {
       collectors: Vec<Box<dyn MetricsCollector>>,
   }
   ```

2. **Integration Points**
   ```rust
   // Metrics context for passing through execution
   pub struct MetricsContext {
       registry: Arc<MetricsRegistry>,
       tags: Tags,
   }

   impl MetricsContext {
       pub fn child(&self, additional_tags: Tags) -> Self { /* ... */ }
       pub async fn record(&self, event: MetricEvent) { /* ... */ }
   }
   ```

3. **Querying System**
   ```rust
   #[derive(Debug, Clone)]
   pub struct MetricsQuery {
       pub metric_names: Vec<String>,
       pub time_range: Option<TimeRange>,
       pub tags: Option<Tags>,
       pub aggregation: Option<Aggregation>,
   }

   #[derive(Debug, Clone)]
   pub enum Aggregation {
       Sum,
       Average,
       Min,
       Max,
       Count,
       Percentile(f64),
   }
   ```

### Data Structures

1. **Configuration**
   ```rust
   pub struct MetricsConfig {
       pub enabled: bool,
       pub collectors: Vec<CollectorConfig>,
       pub flush_interval: Duration,
       pub buffer_size: usize,
       pub sampling_rate: f64,
   }

   pub enum CollectorConfig {
       File { path: PathBuf },
       Memory,
       Remote { endpoint: String, api_key: String },
       Custom { name: String, config: serde_json::Value },
   }
   ```

2. **Testing Support**
   ```rust
   pub struct MetricsAssert {
       collector: Arc<MemoryMetricsCollector>,
   }

   impl MetricsAssert {
       pub fn new() -> (Self, Box<dyn MetricsCollector>) { /* ... */ }
       
       pub fn assert_counter(&self, name: &str, expected: i64) { /* ... */ }
       pub fn assert_gauge(&self, name: &str, expected: f64) { /* ... */ }
       pub fn assert_timer_called(&self, name: &str) { /* ... */ }
       pub fn assert_no_metrics(&self) { /* ... */ }
   }
   ```

## Dependencies

- **Prerequisites**: 
  - [56-cook-orchestrator-refactor]
  - [57-subprocess-abstraction-layer]
- **Affected Components**: 
  - Cook module (primary user)
  - Analysis components
  - Session management
- **External Dependencies**: 
  - serde for serialization
  - Optional: metrics crates for standard formats

## Testing Strategy

- **Unit Tests**: 
  - Test each collector implementation
  - Test metrics aggregation
  - Test query system
  - Test configuration loading
- **Integration Tests**: 
  - End-to-end metrics flow
  - Multiple collectors
  - Performance under load
- **Performance Tests**: 
  - Overhead measurement
  - High-frequency metrics
  - Memory usage over time

## Documentation Requirements

- **Code Documentation**: 
  - Document all metrics events
  - Collector implementation guide
  - Query language reference
- **Metrics Catalog**: 
  - List all collected metrics
  - Describe metric meanings
  - Tagging conventions
- **Operations Guide**: 
  - How to add new metrics
  - Monitoring best practices
  - Performance tuning

## Implementation Notes

1. **Performance Optimizations**
   - Use atomic counters for high-frequency metrics
   - Batch writes to reduce I/O
   - Implement sampling for verbose metrics
   - Lazy initialization when disabled

2. **Extensibility**
   - Plugin system for custom collectors
   - Metrics transformation pipeline
   - Custom aggregation functions
   - Export to standard formats (Prometheus, StatsD)

3. **Error Handling**
   - Metrics failures should not affect operation
   - Log metrics errors separately
   - Graceful degradation
   - Circuit breaker for remote collectors

## Migration and Compatibility

1. **Backward Compatibility**
   - Maintain existing metrics format
   - Support old metrics file locations
   - Gradual migration path

2. **Migration Steps**
   - Phase 1: Implement new metrics system
   - Phase 2: Add adapter for existing metrics
   - Phase 3: Migrate cook module
   - Phase 4: Deprecate old system

3. **Feature Flags**
   - Enable new metrics system optionally
   - A/B testing of implementations
   - Gradual rollout