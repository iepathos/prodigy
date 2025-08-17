---
number: 50
title: Remove Analyze Functionality for Focused Orchestration
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-08-17
---

# Specification 50: Remove Analyze Functionality for Focused Orchestration

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

MMM currently contains extensive code analysis functionality including dependency analysis, architecture detection, convention extraction, technical debt identification, test coverage analysis, and metrics collection. This functionality represents a significant portion of the codebase across multiple modules:

- `src/analyze/` - Command implementation for analysis
- `src/analysis/` - Core analysis execution and orchestration
- `src/context/` - 14 modules for various analysis aspects (architecture, conventions, debt, dependencies, coverage, etc.)

While valuable, this deep analysis functionality has grown beyond the core mission of MMM as a declarative Claude Code orchestration tool. The analysis features are complex enough to warrant their own dedicated tool, allowing MMM to focus purely on its primary strength: declarative workflow orchestration for Claude Code improvement loops.

The current coupling between analysis and orchestration creates maintenance overhead, increases binary size, adds dependencies, and dilutes the tool's focus. By extracting analysis to a separate tool, both tools can evolve independently and be optimized for their specific purposes.

## Objective

Remove all code analysis functionality from MMM to create a lean, focused tool dedicated exclusively to declarative Claude Code orchestration loops. Extract the analysis capabilities into a separate, dedicated code analysis tool that can evolve independently.

## Requirements

### Functional Requirements
- Remove all analysis-related commands from the CLI interface
- Remove the entire `src/analyze/` module tree
- Remove the entire `src/analysis/` module tree  
- Remove the entire `src/context/` module tree
- Remove analysis-related dependencies from Cargo.toml
- Preserve cook command's ability to work without analysis
- Update cook command to use simplified context if needed
- Remove analysis-specific configuration options
- Clean up analysis-related test fixtures and test code
- Remove `.mmm/context/` directory generation
- Simplify `.mmm/` directory structure to only what's needed for orchestration

### Non-Functional Requirements
- Maintain backward compatibility for cook workflows
- Ensure no runtime errors from missing analysis
- Reduce binary size significantly (target: 30-50% reduction)
- Simplify dependency tree
- Improve compilation time
- Maintain clear separation of concerns
- Document migration path for users needing analysis

## Acceptance Criteria

- [ ] `mmm analyze` command no longer exists
- [ ] All analysis-related source files removed
- [ ] Cook command works without any analysis dependencies
- [ ] Binary size reduced by at least 30%
- [ ] Compilation time improved measurably
- [ ] All tests pass without analysis modules
- [ ] Documentation updated to reflect focused scope
- [ ] Migration guide created for users needing analysis
- [ ] Cook workflows continue to function correctly
- [ ] No references to analysis modules remain in code
- [ ] Cargo.toml dependencies cleaned up
- [ ] `.mmm/` directory only contains orchestration-related files

## Technical Details

### Implementation Approach

1. **Phase 1: Decouple Cook from Analysis**
   - Identify all touchpoints where cook uses analysis
   - Create minimal context provider for cook if needed
   - Update cook to work with optional/missing analysis
   - Add feature flag to disable analysis temporarily

2. **Phase 2: Remove Analysis Modules**
   - Delete `src/analyze/` directory
   - Delete `src/analysis/` directory
   - Delete `src/context/` directory
   - Remove analysis command from main.rs CLI parser
   - Remove analysis-related imports throughout codebase

3. **Phase 3: Clean Dependencies**
   - Remove analysis-specific crates from Cargo.toml
   - Remove analysis-related feature flags
   - Update build configuration
   - Clean up unused imports

4. **Phase 4: Update Tests**
   - Remove analysis-specific test modules
   - Update integration tests to not expect analysis
   - Remove analysis test fixtures
   - Ensure all remaining tests pass

5. **Phase 5: Documentation Updates**
   - Update README to reflect focused scope
   - Remove analysis-related documentation
   - Create migration guide for analysis users
   - Update CLI help text

### Architecture Changes

**Before:**
```
mmm/
├── src/
│   ├── analyze/       # REMOVE
│   ├── analysis/      # REMOVE
│   ├── context/       # REMOVE
│   ├── cook/          # KEEP (simplified)
│   ├── commands/      # KEEP
│   ├── config/        # KEEP
│   └── ...
```

**After:**
```
mmm/
├── src/
│   ├── cook/          # Core orchestration
│   ├── commands/      # Claude command handling
│   ├── config/        # Workflow configuration
│   ├── subprocess/    # Process management
│   ├── worktree/      # Git worktree management
│   └── ...
```

### Data Structures

Remove these context-related structures:
- `AnalysisConfig`
- `AnalysisResult`
- `DependencyGraph`
- `ArchitectureAnalysis`
- `ConventionAnalysis`
- `TechnicalDebt`
- `TestCoverage`
- `ComplexityMetrics`

Keep minimal context if needed:
```rust
pub struct MinimalContext {
    pub project_root: PathBuf,
    pub git_info: Option<GitInfo>,
    pub workflow_config: WorkflowConfig,
}
```

### APIs and Interfaces

**Removed CLI Commands:**
- `mmm analyze context`
- `mmm analyze dependencies`
- `mmm analyze architecture`
- `mmm analyze conventions`
- `mmm analyze debt`
- `mmm analyze coverage`

**Simplified Cook Interface:**
- Remove `--skip-analysis` flag (analysis never runs)
- Remove analysis-related environment variables
- Keep workflow-focused flags only

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/main.rs` - Remove analyze command
  - `src/cook/` - Remove analysis integration
  - `src/lib.rs` - Remove analysis module exports
  - `Cargo.toml` - Remove analysis dependencies
  - All test files referencing analysis
- **External Dependencies to Remove**:
  - Tree-sitter parsers
  - Coverage parsing libraries
  - Complexity calculation libraries
  - Architecture detection libraries

## Testing Strategy

- **Unit Tests**: 
  - Ensure cook tests work without analysis
  - Remove all analysis-specific unit tests
  - Verify minimal context works if needed
- **Integration Tests**: 
  - Test cook workflows without analysis
  - Ensure worktree management still works
  - Verify Claude command execution unchanged
- **Regression Tests**: 
  - Run existing cook workflows
  - Ensure no functionality lost for orchestration
  - Verify performance improvements
- **Migration Tests**:
  - Test migration from analysis-enabled to analysis-free
  - Ensure clean upgrade path

## Documentation Requirements

- **Code Documentation**: 
  - Update module documentation to reflect focus
  - Remove references to analysis features
  - Document simplified architecture
- **User Documentation**: 
  - Rewrite README focusing on orchestration
  - Create "What is MMM" section emphasizing focus
  - Add "MMM vs Analysis Tools" comparison
- **Migration Guide**:
  - Explain why analysis was removed
  - Point to new dedicated analysis tool
  - Provide transition instructions
  - Show how to use external analysis with MMM

## Implementation Notes

- Consider creating `mmm-analyze` as separate tool first
- Ensure smooth migration path for existing users
- Keep git history clean with meaningful commits
- Consider publishing final version with analysis before removal
- Tag last version with analysis for reference
- Use feature flags for gradual rollout if needed
- Ensure cook workflows remain fully functional
- Test thoroughly in real projects before release
- Consider keeping minimal context reading capability
- Preserve ability to read external analysis if provided

## Migration and Compatibility

### For Existing Users
1. **Final Release with Analysis** (v0.2.0)
   - Mark analysis features as deprecated
   - Add deprecation warnings when used
   - Point to new analysis tool

2. **Transition Period**
   - Maintain both tools briefly
   - Allow users to migrate workflows
   - Provide conversion scripts if needed

3. **Clean Break** (v0.3.0)
   - Remove all analysis code
   - Focus purely on orchestration
   - Smaller, faster, more focused tool

### Workflow Compatibility
- Existing cook workflows continue to work
- Analysis steps in workflows skip gracefully
- Optional external analysis integration
- Environment variables for external context

### External Analysis Integration
```yaml
# Future workflow with external analysis
steps:
  - shell: "mmm-analyze --output .analysis.json"
  - claude: "/improve --context .analysis.json"
```

This approach maintains MMM's value while allowing both tools to excel at their specific purposes.