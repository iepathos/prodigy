# Specification 44: Context-Aware Project Understanding

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [Spec 10: Smart Project Analyzer, Spec 11: Simple State Management]

## Context

Currently, MMM operates with limited understanding of the project it's improving. While it can detect languages and frameworks (Spec 10), it lacks deep understanding of the codebase structure, dependencies, patterns, and technical debt. This limitation prevents truly autonomous operation over many iterations, as Claude lacks the context to make informed decisions about what to improve next, where the critical paths are, and what patterns to follow.

For MMM to achieve truly self-sufficient loops that can run for days without human intervention, it needs to build and maintain a comprehensive understanding of the project - not just surface-level file detection, but deep architectural knowledge, dependency relationships, and project-specific patterns.

## Objective

Implement a context-aware project understanding system that automatically builds and maintains deep knowledge about the codebase, enabling Claude to make intelligent, goal-oriented improvements without human guidance.

## Requirements

### Functional Requirements

1. **Dependency Graph Analysis**
   - Build a complete understanding of module relationships
   - Track imports, exports, and cross-file dependencies
   - Identify circular dependencies and tight coupling
   - Map API boundaries and interfaces
   - Understand build dependencies and external packages

2. **Architecture Extraction**
   - Auto-document the real architecture (not just intended)
   - Identify architectural patterns (MVC, microservices, etc.)
   - Map component boundaries and responsibilities
   - Detect architectural violations and inconsistencies
   - Generate visual architecture diagrams

3. **Convention Detection**
   - Learn coding patterns beyond .editorconfig rules
   - Identify naming conventions for variables, functions, classes
   - Detect common patterns for error handling, logging
   - Learn project-specific idioms and practices
   - Understand test patterns and structure

4. **Technical Debt Mapping**
   - Automatically identify and categorize technical debt
   - Prioritize debt by impact and risk
   - Track TODO/FIXME/HACK comments with context
   - Identify code duplication and complexity hotspots
   - Measure refactoring opportunities

5. **Test Coverage Gap Analysis**
   - Map untested critical paths in the codebase
   - Identify high-risk code without tests
   - Understand existing test patterns and frameworks
   - Prioritize test writing opportunities
   - Track test quality, not just quantity

### Non-Functional Requirements

- **Performance**: Analysis should complete within reasonable time (< 5 min for large projects)
- **Incremental Updates**: Support incremental analysis as code changes
- **Memory Efficiency**: Handle large codebases without excessive memory use
- **Accuracy**: Produce reliable, actionable insights
- **Language Agnostic**: Work with any language Claude can understand

## Acceptance Criteria

- [ ] Dependency graph builder creates accurate module relationship maps
- [ ] Architecture extraction identifies real patterns in the codebase
- [ ] Convention detector learns project-specific patterns accurately
- [ ] Technical debt mapper produces prioritized, actionable debt list
- [ ] Test coverage analyzer identifies critical untested paths
- [ ] Context is persisted and updated incrementally
- [ ] Analysis results are used by improvement commands
- [ ] Performance meets requirements for codebases up to 100k LOC
- [ ] Integration with existing MMM workflow is seamless

## Technical Details

### Implementation Approach

1. **Multi-Pass Analysis System**
   ```rust
   pub struct ProjectAnalyzer {
       dependency_analyzer: DependencyAnalyzer,
       architecture_extractor: ArchitectureExtractor,
       convention_detector: ConventionDetector,
       debt_mapper: TechnicalDebtMapper,
       coverage_analyzer: TestCoverageAnalyzer,
   }
   ```

2. **Persistent Context Storage**
   ```
   .mmm/context/
   ├── dependency_graph.json
   ├── architecture.json
   ├── conventions.json
   ├── technical_debt.json
   ├── test_coverage.json
   └── analysis_metadata.json
   ```

3. **Integration with Claude Commands**
   - Provide rich context to `/mmm-code-review`
   - Guide `/mmm-implement-spec` with conventions
   - Inform focus areas based on debt and coverage

### Architecture Changes

- New `context` module alongside existing `analyzer` module
- Enhanced state management to include analysis results
- Modified Claude command templates to use context
- New CLI commands for manual analysis triggering

### Data Structures

```rust
pub struct DependencyGraph {
    nodes: HashMap<String, ModuleNode>,
    edges: Vec<DependencyEdge>,
    cycles: Vec<Vec<String>>,
    layers: Vec<ArchitecturalLayer>,
}

pub struct ProjectConventions {
    naming_patterns: NamingRules,
    code_patterns: HashMap<String, Pattern>,
    test_patterns: TestingConventions,
    project_idioms: Vec<Idiom>,
}

pub struct TechnicalDebtMap {
    debt_items: Vec<DebtItem>,
    hotspots: Vec<ComplexityHotspot>,
    duplication_map: HashMap<String, Vec<CodeBlock>>,
    priority_queue: BinaryHeap<DebtItem>,
}
```

### APIs and Interfaces

```rust
pub trait ContextAnalyzer {
    async fn analyze(&self, project_path: &Path) -> Result<AnalysisResult>;
    async fn update(&self, changed_files: &[PathBuf]) -> Result<AnalysisResult>;
    fn get_context_for_file(&self, file: &Path) -> FileContext;
    fn get_improvement_suggestions(&self) -> Vec<Suggestion>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 10: Smart Project Analyzer (for basic language detection)
  - Spec 11: Simple State Management (for context persistence)
- **Affected Components**: 
  - Claude command templates will need context integration
  - Cook workflow will use analysis for decisions
  - State management will store analysis results
- **External Dependencies**: 
  - Consider tree-sitter for language-agnostic parsing
  - Potential use of LSP servers for accurate analysis

## Testing Strategy

- **Unit Tests**: Test each analyzer component independently
- **Integration Tests**: Verify full analysis pipeline on sample projects
- **Performance Tests**: Benchmark analysis on various project sizes
- **Accuracy Tests**: Validate analysis results against known projects
- **User Acceptance**: Run autonomous improvements using context

## Documentation Requirements

- **Code Documentation**: Document analysis algorithms and heuristics
- **User Documentation**: Explain how context improves autonomous operation
- **Architecture Updates**: Update ARCHITECTURE.md with context system
- **Claude Context Files**: Generate PROJECT_CONTEXT.md automatically

## Implementation Notes

1. **Incremental Analysis**: Critical for performance - only re-analyze changed parts
2. **Language Agnostic**: Use generic patterns where possible, language-specific where needed
3. **Heuristic Balance**: Some analysis will be heuristic - aim for useful, not perfect
4. **Context Pruning**: Keep context focused and relevant, not exhaustive
5. **Fail Gracefully**: If analysis fails, MMM should still function with reduced context

## Migration and Compatibility

- **Backward Compatible**: MMM continues to work without full analysis
- **Progressive Enhancement**: Context improves over time with use
- **Optional Features**: Users can disable specific analyzers if needed
- **Existing Projects**: First run performs full analysis, subsequent runs are incremental