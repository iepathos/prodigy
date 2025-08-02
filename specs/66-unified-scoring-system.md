---
number: 66
title: Unified Scoring System
category: optimization
priority: high
status: draft
dependencies: [46]
created: 2024-01-15
---

# Specification 66: Unified Scoring System

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [46 - Real metrics tracking]

## Context

The current MMM scoring system is confusing and inconsistent:

1. **Two different "overall quality scores"** with different calculations:
   - Context analysis: Test coverage (30%) + Hybrid coverage (30%) + Tech debt (40%)
   - Metrics analysis: Test coverage (30%) + Code quality (20%) + Docs (15%) + Tech debt (20%) + Type coverage (15%)

2. **Two different "technical debt scores"** that mean opposite things:
   - Context analysis: Higher score = less debt (good) - based on count of TODOs/FIXMEs
   - Metrics analysis: Higher score = more debt (bad) - based on coverage, complexity, warnings

3. **Confusing "hybrid coverage score"** that users don't understand:
   - Attempts to combine test coverage with quality metrics
   - Adds complexity without clear value
   - Shows as 0.0 when no coverage data available

This leads to confusion when running `mmm analyze` or `mmm cook`, where multiple conflicting scores are displayed.

## Objective

Create a single, unified scoring system that:
- Provides one clear "Project Health Score" from 0-100
- Has consistent meaning (higher = better)
- Combines all quality metrics in a transparent way
- Eliminates duplicate and confusing scores

## Requirements

### Functional Requirements
- Single unified health score calculation used everywhere
- Clear breakdown of score components
- Consistent score direction (100 = perfect, 0 = needs work)
- Handle missing data gracefully without showing 0.0
- Display individual metrics clearly without multiple "scores"

### Non-Functional Requirements
- Easy to understand at a glance
- Transparent calculation methodology
- Extensible for future metrics
- Performance: Calculate quickly even for large projects

## Acceptance Criteria

- [ ] Remove hybrid coverage score entirely
- [ ] Unify technical debt calculation into single metric
- [ ] Create single `ProjectHealthScore` struct used by both analysis types
- [ ] Display shows one overall score with clear component breakdown
- [ ] All scores consistently use 0-100 range where higher is better
- [ ] Missing data shows as "N/A" not 0.0
- [ ] Documentation clearly explains scoring methodology
- [ ] Tests verify score calculations are consistent

## Technical Details

### Implementation Approach

1. Create unified `ProjectHealthScore` struct:
```rust
pub struct ProjectHealthScore {
    pub overall: f64,           // 0-100, higher is better
    pub components: ScoreComponents,
    pub timestamp: DateTime<Utc>,
}

pub struct ScoreComponents {
    pub test_coverage: Option<f64>,      // 0-100
    pub code_quality: Option<f64>,       // 0-100  
    pub documentation: Option<f64>,      // 0-100
    pub maintainability: Option<f64>,    // 0-100
    pub type_safety: Option<f64>,        // 0-100
}
```

2. Single calculation method:
```rust
impl ProjectHealthScore {
    pub fn calculate(metrics: &ProjectMetrics) -> Self {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        
        // Test coverage (35% weight)
        if let Some(coverage) = metrics.test_coverage {
            total_score += coverage * 0.35;
            total_weight += 0.35;
        }
        
        // Code quality (25% weight) - based on lint warnings, duplication
        if let Some(quality) = calculate_code_quality(metrics) {
            total_score += quality * 0.25;
            total_weight += 0.25;
        }
        
        // Maintainability (25% weight) - based on complexity, debt items
        if let Some(maint) = calculate_maintainability(metrics) {
            total_score += maint * 0.25;
            total_weight += 0.25;
        }
        
        // Documentation (10% weight)
        if let Some(docs) = metrics.doc_coverage {
            total_score += docs * 0.10;
            total_weight += 0.10;
        }
        
        // Type safety (5% weight)
        if let Some(types) = metrics.type_coverage {
            total_score += types * 0.05;
            total_weight += 0.05;
        }
        
        let overall = if total_weight > 0.0 {
            total_score / total_weight
        } else {
            50.0 // Neutral score when no data
        };
        
        // ... rest of implementation
    }
}
```

3. Maintainability score combines:
   - Technical debt items (TODOs, FIXMEs, etc.)
   - Cyclomatic complexity
   - Code duplication
   - File organization

### Architecture Changes

1. Remove `hybrid_coverage` module entirely
2. Consolidate scoring logic into `scoring` module
3. Update both analysis paths to use unified scoring
4. Modify display functions to show single score breakdown

### Data Structures

Replace current scattered scores with unified structure:
- Remove `tech_debt_score` from `ImprovementMetrics`
- Remove `hybrid_score` from analysis
- Add `health_score: ProjectHealthScore` to both analysis types

### APIs and Interfaces

Update display output to show:
```
📊 Project Health Score: 72.5/100

Components:
  ✓ Test Coverage: 55.6%
  ✓ Code Quality: 85.0% (1 warning, 0% duplication)
  ✓ Maintainability: 78.2% (211 debt items, avg complexity: 2.0)
  ✓ Documentation: 20.2%
  ⚠ Type Safety: N/A

💡 Top improvements:
  1. Increase documentation coverage (current: 20.2%)
  2. Add tests for uncovered modules (current: 55.6%)
  3. Address high-priority technical debt items
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `context/mod.rs` - Remove hybrid coverage integration
  - `metrics/collector.rs` - Update score calculation
  - `metrics/mod.rs` - Update data structures
  - `analyze/command.rs` - Update display logic
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test score calculations with various metric combinations
- **Integration Tests**: Verify consistent scores between analysis types
- **Edge Cases**: Test with missing data, extreme values
- **Regression Tests**: Ensure scores remain stable for same input

## Documentation Requirements

- **Code Documentation**: Document scoring weights and rationale
- **User Documentation**: Update README with new scoring explanation
- **Architecture Updates**: Update ARCHITECTURE.md with scoring module

## Implementation Notes

- Start by creating the unified scoring module
- Gradually migrate both analysis types to use it
- Keep backward compatibility during transition
- Consider making weights configurable in future

## Migration and Compatibility

- No breaking changes to CLI interface
- Internal score storage format will change
- Historical metrics may need score recalculation
- Clear communication about score meaning changes