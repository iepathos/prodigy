---
number: 49
title: Enhanced Analysis and Criticality Assessment for MMM Context
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-08-04
---

# Specification 49: Enhanced Analysis and Criticality Assessment for MMM Context

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM's context-aware automation relies heavily on the quality of its analysis and criticality assessment algorithms. Current observations show that while the context integration works correctly (environment variables are set, files are loaded), the effectiveness is limited by:

1. **Simplistic Criticality Assessment**: Only 2 functions marked as "High" criticality out of 1,616 untested functions (0.12%)
2. **Basic Pattern Matching**: Criticality determined by simple keyword matching (auth, security, payment, crypto)
3. **Limited Context Utilization**: Claude commands receive context but may not leverage it optimally
4. **Insufficient Signal Quality**: Most functions marked as "Low" criticality, reducing actionable insights

This specification addresses improvements to the analysis pipeline, criticality assessment algorithms, and context utilization patterns to maximize the value of MMM's automation capabilities.

## Objective

Enhance MMM's analysis and criticality assessment to provide more accurate, actionable insights that enable Claude commands to make better-informed decisions about code improvements, test coverage prioritization, and technical debt reduction.

## Requirements

### Functional Requirements

#### 1. Enhanced Criticality Assessment
- **Multi-Factor Scoring**: Replace keyword-based criticality with a comprehensive scoring system
- **Context-Aware Analysis**: Consider function's role in the codebase (API boundaries, data flow, error handling)
- **Dynamic Weighting**: Adjust criticality based on project-specific patterns and historical data
- **Configurable Thresholds**: Allow projects to define criticality rules via configuration

#### 2. Improved Analysis Integration
- **Hybrid Coverage Utilization**: Leverage existing hybrid_coverage.json that combines test coverage with quality metrics
- **Cross-Reference Analysis**: Correlate untested functions with complexity hotspots, change frequency, and bug history
- **Dependency Impact**: Consider function's position in dependency graph for criticality
- **Architecture Awareness**: Use architecture.json to identify critical system boundaries

#### 3. Context Optimization for Claude
- **Focused Context Delivery**: Provide relevant subset of context based on command type
- **Priority-Based Filtering**: Surface high-impact items prominently in context
- **Actionable Summaries**: Include pre-computed recommendations in context files
- **Command-Specific Views**: Tailor context structure for different MMM commands

### Non-Functional Requirements

- **Performance**: Analysis must complete within current time constraints (< 2 seconds for incremental updates)
- **Backward Compatibility**: Maintain existing context file formats while adding new fields
- **Scalability**: Handle large codebases (100K+ LOC) without significant degradation
- **Transparency**: Provide clear explanations for criticality scores and recommendations

## Acceptance Criteria

- [ ] Criticality assessment identifies 5-10% of functions as "High" priority based on comprehensive scoring
- [ ] Coverage improvement suggestions correlate with actual code importance metrics
- [ ] Claude commands demonstrate improved decision-making with enhanced context
- [ ] Analysis provides clear rationale for each criticality assignment
- [ ] Hybrid coverage integration reduces false positives in priority recommendations
- [ ] Configuration allows project-specific criticality rules
- [ ] Performance remains within acceptable bounds for large projects
- [ ] Existing workflows continue to function without modification

## Technical Details

### Implementation Approach

#### Phase 1: Enhanced Criticality Scoring System

1. **Multi-Factor Score Components**:
   ```rust
   pub struct CriticalityScore {
       base_score: f32,          // From keyword/pattern matching
       complexity_factor: f32,    // From cyclomatic/cognitive complexity
       dependency_factor: f32,    // From dependency graph position
       change_frequency: f32,     // From git history analysis
       bug_correlation: f32,      // From historical bug density
       architecture_role: f32,    // From architecture boundaries
       test_gap_impact: f32,      // From coverage analysis
   }
   ```

2. **Scoring Algorithm**:
   - Base score from enhanced pattern matching (0-30 points)
   - Complexity multiplier (1.0-3.0x based on cognitive complexity)
   - Dependency impact (1.0-2.0x for high fan-out functions)
   - Change frequency bonus (0-20 points for frequently modified code)
   - Bug history correlation (0-15 points based on past issues)
   - Architecture boundary bonus (0-15 points for API/interface functions)
   - Test gap severity (0-20 points based on surrounding coverage)

3. **Criticality Thresholds**:
   - High: Score >= 70
   - Medium: Score >= 40
   - Low: Score < 40

#### Phase 2: Context Enhancement

1. **Hybrid Coverage Integration**:
   ```rust
   pub struct EnhancedTestCoverage {
       // Existing fields
       file_coverage: HashMap<PathBuf, FileCoverage>,
       untested_functions: Vec<UntestedFunction>,
       
       // New fields
       priority_gaps: Vec<PriorityGap>,
       quality_correlations: QualityCorrelation,
       actionable_items: Vec<ActionableItem>,
       criticality_rationale: HashMap<String, CriticalityExplanation>,
   }
   ```

2. **Command-Specific Context Views**:
   - `/mmm-coverage`: Priority gaps with test templates
   - `/mmm-code-review`: High-risk code sections with rationale
   - `/mmm-lint`: Convention violations correlated with complexity

3. **Pre-computed Recommendations**:
   ```rust
   pub struct ActionableItem {
       item_type: ItemType,
       priority: Priority,
       estimated_impact: ImpactScore,
       suggested_action: String,
       implementation_hints: Vec<String>,
       related_context: Vec<ContextReference>,
   }
   ```

### Architecture Changes

1. **New Analysis Pipeline Components**:
   - `CriticalityScorer`: Multi-factor scoring engine
   - `ContextOptimizer`: Command-specific context preparation
   - `RecommendationEngine`: Pre-compute actionable suggestions
   - `QualityCorrelator`: Cross-reference quality metrics

2. **Enhanced Data Flow**:
   ```
   Source Code -> Parser -> Analysis Components -> Scoring Engine
                                |                      |
                                v                      v
                         Git History Analysis    Architecture Analysis
                                |                      |
                                v                      v
                         Change Frequency        Boundary Detection
                                |                      |
                                +----------------------+
                                           |
                                           v
                                   Criticality Scorer
                                           |
                                           v
                                   Context Optimizer
                                           |
                                           v
                                   Command-Specific Views
   ```

### Data Structures

1. **Criticality Configuration**:
   ```toml
   [criticality]
   # Project-specific patterns
   high_priority_patterns = ["payment", "auth", "security", "api"]
   critical_paths = ["src/core/engine.rs", "src/api/handlers/"]
   
   # Scoring weights
   [criticality.weights]
   complexity = 2.0
   dependencies = 1.5
   change_frequency = 1.2
   bug_history = 1.8
   architecture = 1.5
   ```

2. **Enhanced Context Metadata**:
   ```json
   {
     "analysis_metadata": {
       "version": "2.0",
       "scoring_algorithm": "multi-factor-v1",
       "criticality_distribution": {
         "high": 127,
         "medium": 431,
         "low": 1058
       },
       "confidence_scores": {
         "coverage_accuracy": 0.95,
         "criticality_confidence": 0.87
       }
     }
   }
   ```

### APIs and Interfaces

1. **Criticality Scorer API**:
   ```rust
   pub trait CriticalityScorer {
       fn score_function(&self, 
           func: &UntestedFunction,
           context: &AnalysisContext
       ) -> CriticalityScore;
       
       fn explain_score(&self, 
           score: &CriticalityScore
       ) -> CriticalityExplanation;
   }
   ```

2. **Context Optimizer API**:
   ```rust
   pub trait ContextOptimizer {
       fn optimize_for_command(&self,
           command: &str,
           full_context: &AnalysisContext
       ) -> OptimizedContext;
       
       fn get_recommendations(&self,
           context: &OptimizedContext
       ) -> Vec<ActionableItem>;
   }
   ```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/context/test_coverage.rs` - Criticality assessment logic
  - `src/context/tarpaulin_coverage.rs` - Coverage analysis
  - `src/context/mod.rs` - Context aggregation
  - `src/cook/orchestrator.rs` - Context delivery to Claude
- **External Dependencies**: None (uses existing Rust toolchain)

## Testing Strategy

- **Unit Tests**: 
  - Criticality scoring algorithm with known inputs/outputs
  - Context optimization for different command types
  - Configuration parsing and validation

- **Integration Tests**:
  - End-to-end analysis pipeline with sample projects
  - Claude command execution with enhanced context
  - Performance benchmarks with large codebases

- **Validation Tests**:
  - Compare criticality assignments with expert reviews
  - Measure improvement in coverage targeting accuracy
  - Track false positive/negative rates

## Documentation Requirements

- **Code Documentation**:
  - Document scoring algorithm and rationale
  - Explain each factor's contribution to final score
  - Provide examples of criticality calculations

- **User Documentation**:
  - Update CLAUDE.md with new context fields
  - Add configuration guide for criticality rules
  - Include troubleshooting for scoring issues

- **Architecture Updates**:
  - Document new analysis pipeline components
  - Update data flow diagrams
  - Add context optimization strategies

## Implementation Notes

1. **Incremental Rollout**: 
   - Phase 1: Implement enhanced scoring without breaking changes
   - Phase 2: Add context optimization layer
   - Phase 3: Integrate command-specific views

2. **Backward Compatibility**:
   - Maintain existing context file structure
   - Add new fields as optional extensions
   - Provide migration path for configurations

3. **Performance Considerations**:
   - Cache git history analysis results
   - Use incremental scoring for file changes
   - Parallelize independent scoring factors

4. **Monitoring and Feedback**:
   - Log criticality score distributions
   - Track command success rates with new context
   - Collect user feedback on recommendation quality

## Migration and Compatibility

- **Existing Projects**: Will automatically benefit from enhanced scoring
- **Configuration Migration**: Provide defaults that match current behavior
- **Context Files**: New fields added without breaking existing consumers
- **Command Updates**: Claude commands will gracefully handle both old and new context formats

## Success Metrics

1. **Criticality Distribution**: 5-10% High, 20-30% Medium, 60-75% Low
2. **Coverage Improvement**: 15% better targeting of critical gaps
3. **Command Effectiveness**: 25% reduction in non-actionable suggestions
4. **User Satisfaction**: Positive feedback on recommendation quality
5. **Performance**: < 5% increase in analysis time