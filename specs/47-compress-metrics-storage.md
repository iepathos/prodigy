---
number: 47
title: Compress Metrics Storage
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-08-03
---

# Specification 47: Compress Metrics Storage

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The `.mmm/metrics/current.json` file has grown to over 100KB due to storing full absolute paths and individual function-level complexity metrics for every function in the codebase. With 597+ function entries, each containing full paths like `/Users/glen/memento-mori/mmm/src/...`, the file exceeds token limits when being read by Claude during `mmm cook` operations, causing the error: "File content (43434 tokens) exceeds maximum allowed tokens (25000)".

The current structure stores both `cyclomatic_complexity` and `cognitive_complexity` as separate HashMaps with identical keys (full path + function name), resulting in significant redundancy. Most functions have trivial complexity scores of 1-2, yet all are stored individually.

## Objective

Reduce the size of `.mmm/metrics/current.json` by 90%+ through intelligent aggregation and filtering, while preserving the most important metric information for analysis and trend tracking.

## Requirements

### Functional Requirements
- Aggregate complexity metrics by file rather than by function
- Filter out trivial functions with low complexity scores
- Store relative paths instead of absolute paths
- Maintain backward compatibility for metric history loading
- Preserve high-value metrics for problematic functions

### Non-Functional Requirements
- Reduce file size from ~109KB to under 10KB
- Maintain metric accuracy for trend analysis
- Ensure fast serialization/deserialization
- Keep the format human-readable (JSON)

## Acceptance Criteria

- [ ] `current.json` file size reduced by at least 90%
- [ ] File can be read by Claude without token limit errors
- [ ] Historical metrics can still be loaded and compared
- [ ] No loss of critical metric information (hotspots, high complexity functions)
- [ ] Metric trends remain accurate after aggregation
- [ ] All existing tests pass with new format
- [ ] New tests added for compression logic

## Technical Details

### Implementation Approach

1. **File-Level Aggregation** (Approach #4)
   - Replace per-function maps with per-file statistics
   - Calculate average, max, and distribution metrics per file
   - Track count of functions above complexity thresholds

2. **Complexity Filtering** (Approach #3)
   - Only store individual function data for complexity > 5
   - Add summary statistics for filtered functions
   - Maintain a "hotspots" section for high-complexity functions

### Data Structures

Replace current structure:
```rust
pub struct ImprovementMetrics {
    pub cyclomatic_complexity: HashMap<String, u32>,
    pub cognitive_complexity: HashMap<String, u32>,
    // ...
}
```

With compressed structure:
```rust
pub struct ImprovementMetrics {
    pub complexity_summary: ComplexitySummary,
    pub complexity_hotspots: Vec<ComplexityHotspot>,
    // ... other fields unchanged
}

pub struct ComplexitySummary {
    pub by_file: HashMap<String, FileComplexityStats>,
    pub total_functions: u32,
    pub filtered_functions: u32,
}

pub struct FileComplexityStats {
    pub avg_cyclomatic: f32,
    pub max_cyclomatic: u32,
    pub avg_cognitive: f32,
    pub max_cognitive: u32,
    pub functions_count: u32,
    pub high_complexity_count: u32, // functions with complexity > 10
}

pub struct ComplexityHotspot {
    pub file: String,           // relative path
    pub function: String,       // function name only
    pub cyclomatic: u32,
    pub cognitive: u32,
}
```

### APIs and Interfaces

The metrics collector will need updates:
- `collect_complexity_metrics()` - Modified to return aggregated data
- `calculate_file_stats()` - New function for file-level aggregation
- `filter_hotspots()` - New function to identify high-complexity functions

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/metrics/collector.rs` - Metric collection logic
  - `src/metrics/complexity.rs` - Complexity calculation
  - `src/metrics/storage.rs` - Serialization format
  - `src/metrics/mod.rs` - ImprovementMetrics struct
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test aggregation produces correct statistics
  - Verify filtering thresholds work correctly
  - Ensure path conversion (absolute to relative)
- **Integration Tests**: 
  - Load and save metrics with new format
  - Verify backward compatibility with history
  - Test metric trend calculations remain accurate
- **Performance Tests**: 
  - Measure serialization speed improvement
  - Verify file size reduction meets targets
- **User Acceptance**: 
  - Claude can read entire file without token errors
  - Metric displays remain informative

## Documentation Requirements

- **Code Documentation**: 
  - Document new data structures and their purpose
  - Explain aggregation algorithm and thresholds
- **User Documentation**: 
  - Update CLAUDE.md to reflect new metric format
  - Note the compression improvements
- **Architecture Updates**: 
  - Update metrics storage documentation

## Implementation Notes

- Use a threshold of 5 for filtering trivial functions (adjustable via constant)
- Store top 20 complexity hotspots to maintain visibility of problematic code
- Consider using `serde` aliases for backward compatibility during transition
- Implement migration logic to convert old format to new on first load
- Use project root-relative paths by stripping common prefix

## Migration and Compatibility

- Existing `current.json` files will be migrated automatically on first load
- History files will remain in old format but can be compared with new format
- A one-time conversion may be offered for history files if needed
- The change is backward compatible - old metrics can be read and converted