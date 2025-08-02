---
number: 68
title: Iteration and Workflow Timing Statistics
category: optimization
priority: medium
status: draft
dependencies: [46, 58, 60]
created: 2025-08-02
---

# Specification 68: Iteration and Workflow Timing Statistics

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [46 (Real Metrics Tracking), 58 (Session State Management Refactor), 60 (Metrics Collection Isolation)]

## Context

MMM currently tracks various metrics about code quality and improvements but lacks visibility into the time spent on each iteration and the total workflow duration. Users running long improvement sessions have no insight into:
- How long each iteration takes to complete
- Which commands within an iteration consume the most time
- The total duration of the entire workflow from start to finish
- Performance trends across iterations

This timing information would be valuable for:
- Understanding the cost/benefit of running additional iterations
- Identifying performance bottlenecks in specific commands
- Optimizing workflows by focusing on slow operations
- Providing transparency about the time investment required

## Objective

Add comprehensive timing statistics that display:
1. Individual iteration durations at the end of each iteration
2. Command-level timing breakdown within each iteration
3. Total workflow duration at completion
4. Integration with the existing metrics system for historical tracking

## Requirements

### Functional Requirements
- Track start and end times for each iteration
- Track duration of each command execution within an iteration
- Display iteration timing summary after each iteration completes
- Display total workflow timing summary after all iterations complete
- Store timing data in metrics history for trend analysis
- Support both human-readable and machine-readable output formats

### Non-Functional Requirements
- Minimal performance overhead from timing collection
- Clear, concise timing displays that don't clutter output
- Accurate timing even with interrupted/resumed sessions
- Thread-safe timing collection for parallel operations

## Acceptance Criteria

- [ ] Each iteration displays its duration upon completion (e.g., "Iteration 3 completed in 2m 34s")
- [ ] Command breakdown shows time for each step (review: 45s, implement: 1m 20s, lint: 29s)
- [ ] Total workflow duration displayed at end (e.g., "Total workflow time: 15m 42s across 5 iterations")
- [ ] Timing data persisted in metrics system for historical analysis
- [ ] Timing displays respect verbosity settings (detailed in verbose mode, summary in normal mode)
- [ ] Resumed sessions correctly track cumulative time excluding interruption periods
- [ ] Timing statistics included in generated reports when --metrics flag is used

## Technical Details

### Implementation Approach
1. Extend SessionState to track timing information
2. Add timing fields to IterationState structure
3. Integrate with MetricsCollector for persistence
4. Update display logic to show timing summaries
5. Enhance reports to include timing analytics

### Architecture Changes
- Add timing fields to SessionState and IterationState
- Create TimingTracker component for accurate time measurement
- Extend MetricsEvent to include timing-specific events
- Update SessionManager to emit timing events

### Data Structures
```rust
pub struct IterationTiming {
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub command_timings: HashMap<String, Duration>,
}

pub struct WorkflowTiming {
    pub total_duration: Duration,
    pub iteration_count: usize,
    pub average_iteration_time: Duration,
    pub slowest_iteration: (usize, Duration),
    pub fastest_iteration: (usize, Duration),
}
```

### APIs and Interfaces
- `SessionManager::start_iteration()` - Records iteration start time
- `SessionManager::complete_iteration()` - Records iteration end time and calculates duration
- `SessionManager::record_command_timing()` - Records individual command execution time
- `MetricsCollector::collect_timing()` - Persists timing data to metrics storage

## Dependencies

- **Prerequisites**: 
  - Spec 46 (Real Metrics Tracking) - For metrics storage and reporting
  - Spec 58 (Session State Management) - For session lifecycle tracking
  - Spec 60 (Metrics Collection Isolation) - For clean metrics integration
- **Affected Components**: 
  - cook module - Integration points for timing collection
  - session module - State tracking enhancements
  - metrics module - New timing metrics types
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - TimingTracker accuracy with mock time sources
  - Correct duration calculations including edge cases
  - Proper state transitions with timing data
- **Integration Tests**: 
  - End-to-end timing collection during cook operations
  - Timing persistence and retrieval from metrics storage
  - Report generation with timing statistics
- **Performance Tests**: 
  - Verify minimal overhead from timing collection
  - Benchmark impact on overall execution time
- **User Acceptance**: 
  - Clear, readable timing displays
  - Useful timing breakdowns for optimization decisions

## Documentation Requirements

- **Code Documentation**: 
  - Document timing collection points in cook workflow
  - Explain timing calculation methodology
  - Document timing display formatting choices
- **User Documentation**: 
  - Add timing statistics section to README
  - Include examples of timing output in documentation
  - Explain how to interpret timing breakdowns
- **Architecture Updates**: 
  - Update ARCHITECTURE.md with timing component details
  - Document timing data flow through system

## Implementation Notes

- Use monotonic clocks (Instant) for accurate duration measurement
- Handle clock adjustments gracefully for long-running sessions
- Consider formatting options (human-readable vs ISO 8601 durations)
- Ensure timing collection doesn't interfere with actual command execution
- Support for excluding certain operations from timing (e.g., user prompts)

## Migration and Compatibility

- No breaking changes to existing functionality
- Timing display is additive to current output
- Existing metrics data remains compatible
- Optional timing features can be disabled if needed