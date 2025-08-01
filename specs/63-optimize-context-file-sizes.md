# Specification 63: Optimize Context File Sizes

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The `.mmm/context/` directory contains JSON files that store project analysis data for Claude during development iterations. Currently, these files have grown to 17MB total, with `technical_debt.json` at 8.2MB and `analysis.json` at 8.9MB. This excessive size is primarily due to the duplication detection algorithm generating 8,935 duplication entries through an inefficient sliding window approach.

The current implementation creates overlapping windows for every possible window size from 3-20 lines, resulting in:
- Multiple entries for the same duplicate code (e.g., a 5-line duplicate generates entries for 3, 4, and 5 line windows)
- No deduplication or merging of related duplicates
- Massive JSON files that consume significant Claude CLI context space
- Slower analysis and context loading times

## Objective

Optimize the context file generation to reduce file sizes by at least 90% while maintaining or improving the quality of technical debt analysis. Focus on fixing the duplication detection algorithm and implementing smart aggregation strategies to keep context files under 1MB total.

## Requirements

### Functional Requirements

1. **Fix Duplication Detection Algorithm**
   - Replace sliding window approach with maximal duplicate detection
   - Eliminate overlapping duplicate entries
   - Merge related duplicates into single entries
   - Track only the largest contiguous duplicate blocks

2. **Implement Smart Aggregation**
   - Limit number of items per debt category (e.g., top 100 by impact)
   - Aggregate similar issues into summary entries
   - Store detailed data separately from summary context

3. **Add Context Size Management**
   - Monitor context file sizes during generation
   - Implement size-based truncation strategies
   - Provide warnings when context exceeds size thresholds

4. **Maintain Data Quality**
   - Preserve high-impact technical debt items
   - Keep critical architecture violations
   - Maintain actionable improvement suggestions
   - Ensure no loss of essential context for Claude

### Non-Functional Requirements

1. **Performance**
   - Context generation should complete in under 5 seconds for typical projects
   - File loading should be near-instantaneous (< 100ms)
   - Memory usage during analysis should remain under 100MB

2. **Flexibility** 
   - Break compatibility if needed for better design
   - Prioritize clean implementation over migration support
   - Focus on optimal solution without legacy constraints

3. **Configurability**
   - Allow customization of size limits via configuration
   - Support different aggregation strategies per project
   - Enable detailed mode for debugging when needed

## Acceptance Criteria

- [ ] Technical debt JSON file size reduced from 8.2MB to under 500KB
- [ ] Analysis JSON file size reduced from 8.9MB to under 500KB
- [ ] Total context directory size under 1MB for typical projects
- [ ] Duplication detection generates < 500 entries (down from 8,935)
- [ ] No duplicate entries for the same code block
- [ ] High-impact issues are preserved in the context
- [ ] Context loading time improved by at least 80%
- [ ] Context format is optimized for Claude's consumption
- [ ] Unit tests verify deduplication logic
- [ ] Integration tests confirm size limits are respected

## Technical Details

### Implementation Approach

1. **Phase 1: Fix Duplication Detection**
   ```rust
   // Replace sliding window with maximal duplicate detection
   fn detect_duplicates(&self, files: &[(PathBuf, String)]) -> HashMap<String, Vec<CodeBlock>> {
       // 1. Build suffix array or use rolling hash for efficient detection
       // 2. Find maximal duplicates only (not all substrings)
       // 3. Merge overlapping blocks
       // 4. Filter trivial duplicates (whitespace, single braces)
   }
   ```

2. **Phase 2: Implement Aggregation**
   ```rust
   struct ContextAggregator {
       max_items_per_category: usize,
       max_file_size: usize,
       aggregation_strategy: AggregationStrategy,
   }
   
   impl ContextAggregator {
       fn aggregate_debt_items(&self, items: Vec<DebtItem>) -> Vec<DebtItem> {
           // Sort by impact score
           // Take top N items
           // Group similar items
           // Create summary entries for grouped items
       }
   }
   ```

3. **Phase 3: Add Size Management**
   ```rust
   struct ContextSizeManager {
       target_size: usize,
       warning_threshold: usize,
   }
   
   impl ContextSizeManager {
       fn optimize_context(&self, context: &mut Analysis) -> Result<()> {
           // Monitor serialized size
           // Apply progressive reduction strategies
           // Warn if size exceeds limits
       }
   }
   ```

### Architecture Changes

1. **New Components**
   - `DuplicationDetector` trait with efficient implementation
   - `ContextAggregator` for smart summarization
   - `ContextSizeManager` for size monitoring and optimization

2. **Modified Components**
   - `BasicTechnicalDebtMapper::detect_duplicates()` - Complete rewrite
   - `AnalysisResult` - Add size metadata
   - `ContextAnalyzer` - Integrate aggregation pipeline

### Data Structures

```rust
// Efficient duplicate representation
struct DuplicateGroup {
    content_hash: String,
    instances: Vec<CodeLocation>,
    line_count: usize,
    total_occurrences: usize,
    impact_score: f32,
}

// Aggregated debt summary
struct DebtSummary {
    category: DebtType,
    total_count: usize,
    shown_count: usize,
    top_items: Vec<DebtItem>,
    aggregated_impact: f32,
}

// Size tracking
struct ContextSizeMetadata {
    raw_size: usize,
    compressed_size: usize,
    item_counts: HashMap<String, usize>,
    reduction_applied: bool,
}
```

### APIs and Interfaces

No external API changes. Internal changes:
- `TechnicalDebtMapper::map_technical_debt()` returns size-optimized results
- New `ContextOptimizer` trait for pluggable optimization strategies
- Configuration options in `.mmm/config.toml` for size limits

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/context/debt.rs` - Core changes to duplication detection
  - `src/context/analysis.rs` - Integration of aggregation
  - `src/analyze/context.rs` - Size management integration
- **External Dependencies**: None (uses standard library only)

## Testing Strategy

- **Unit Tests**:
  - Test maximal duplicate detection algorithm
  - Verify deduplication logic
  - Test aggregation strategies
  - Validate size calculation and limits

- **Integration Tests**:
  - Test with large codebases to verify size reduction
  - Ensure Claude commands work with optimized context
  - Verify incremental updates maintain size limits

- **Performance Tests**:
  - Benchmark analysis time before/after optimization
  - Measure memory usage during analysis
  - Test context loading performance

- **User Acceptance**:
  - Verify Claude still receives sufficient context
  - Ensure no regression in improvement quality
  - Test with various project sizes

## Documentation Requirements

- **Code Documentation**:
  - Document new algorithms with complexity analysis
  - Explain aggregation strategies and trade-offs
  - Add examples of size optimization

- **User Documentation**:
  - Update CLAUDE.md with new context size information
  - Document configuration options for size limits
  - Add troubleshooting for context size issues

- **Architecture Updates**:
  - Update architecture docs with new components
  - Document data flow for context optimization
  - Add decision records for algorithm choices

## Implementation Notes

1. **Algorithm Selection**:
   - Consider using suffix arrays or rolling hashes for efficient duplicate detection
   - Rabin-Karp algorithm might be suitable for finding duplicates
   - Union-Find data structure for merging overlapping blocks

2. **Clean Design**:
   - Remove legacy code paths during implementation
   - Use this opportunity to redesign context structure
   - Don't worry about migrating old context files

3. **Progressive Enhancement**:
   - Start with fixing duplication detection (biggest win)
   - Add aggregation in second phase
   - Implement configurable limits last

4. **Edge Cases**:
   - Handle projects with legitimate high duplication (generated code)
   - Preserve security-critical debt items regardless of size
   - Don't lose TODO/FIXME comments in aggregation

## Migration and Compatibility

Since we're prototyping, no migration support is needed:

1. **Clean Break**:
   - Old context files will be regenerated automatically
   - No migration paths or compatibility layers
   - Focus on the optimal implementation

2. **Testing Strategy**:
   - Test directly on MMM codebase
   - Iterate quickly based on results
   - Refine approach based on real-world usage