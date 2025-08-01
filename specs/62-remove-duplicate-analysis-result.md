# Specification 62: Remove Duplicate AnalysisResult Struct

**Category**: refactor
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The codebase currently contains two different `AnalysisResult` structs that represent the same data:

1. `context::AnalysisResult` in `src/context/mod.rs` - Uses strongly typed fields
2. `cook::analysis::AnalysisResult` in `src/cook/analysis/mod.rs` - Uses generic `serde_json::Value` fields

This duplication violates the DRY (Don't Repeat Yourself) principle and has already caused bugs. The cook module converts from the typed version to the JSON version, but both save to the same file (`.mmm/context/analysis.json`), leading to deserialization errors when one module saves a format the other doesn't expect.

## Objective

Remove the duplicate `AnalysisResult` struct from the cook module and have it use the context module's typed version directly. When JSON serialization is needed, it should be done at the point of use rather than maintaining a parallel data structure.

## Requirements

### Functional Requirements
- Cook module must use `context::AnalysisResult` directly
- JSON serialization must work when needed (e.g., for external tools)
- All existing functionality must continue to work
- No breaking changes to the analysis file format

### Non-Functional Requirements
- Maintain type safety throughout the codebase
- Reduce code duplication and maintenance burden
- Improve code clarity and consistency

## Acceptance Criteria

- [ ] `cook::analysis::AnalysisResult` struct is removed
- [ ] `cook::analysis::AnalysisMetadata` struct is removed (use context version)
- [ ] Cook module imports and uses `context::AnalysisResult`
- [ ] All cook module code that expects JSON values is updated to work with typed fields
- [ ] JSON serialization happens only when needed (e.g., when passing to Claude)
- [ ] All tests pass without modification
- [ ] No regression in functionality
- [ ] Analysis files saved by cook can be loaded by analyze command and vice versa

## Technical Details

### Implementation Approach

1. **Remove Duplicate Types**
   - Delete `cook::analysis::AnalysisResult` struct
   - Delete `cook::analysis::AnalysisMetadata` struct
   - Update `cook::analysis::mod.rs` to re-export from context module

2. **Update Type References**
   - Change all `cook::analysis::AnalysisResult` to `context::AnalysisResult`
   - Update import statements throughout cook module
   - Fix any type mismatches in function signatures

3. **Handle JSON Conversion**
   - Where cook module needs JSON, serialize at point of use
   - Add helper methods if needed for common JSON conversions
   - Ensure backward compatibility with existing analysis files

4. **Update Cook Runner**
   - Modify `cook::analysis::runner::AnalysisRunnerImpl` to return typed result
   - Remove manual JSON conversion in `run_analysis` method
   - Update coverage handling to work with typed fields

### Architecture Changes

The cook module will depend directly on the context module for analysis types, creating a clearer dependency hierarchy:

```
context module (defines types)
    ↑
cook module (uses types)
```

### Data Structures

No new data structures. The cook module will use these existing types from context:
- `context::AnalysisResult`
- `context::AnalysisMetadata`
- All nested types (DependencyGraph, ArchitectureInfo, etc.)

### APIs and Interfaces

The `AnalysisCoordinator` trait will be updated:
```rust
#[async_trait]
pub trait AnalysisCoordinator: Send + Sync {
    async fn analyze_project(&self, project_path: &Path) -> Result<context::AnalysisResult>;
    async fn analyze_incremental(&self, project_path: &Path, changed_files: &[String]) -> Result<context::AnalysisResult>;
    async fn get_cached_analysis(&self, project_path: &Path) -> Result<Option<context::AnalysisResult>>;
    async fn save_analysis(&self, project_path: &Path, analysis: &context::AnalysisResult) -> Result<()>;
    async fn clear_cache(&self, project_path: &Path) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `cook::analysis` module
  - `cook::analysis::runner`
  - `cook::analysis::cache`
  - Any code that uses `AnalysisCoordinator` trait
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Ensure all existing cook analysis tests pass
- **Integration Tests**: Verify cook and analyze commands can read each other's files
- **Regression Tests**: Full cook workflow must work as before
- **Compatibility Tests**: Ensure existing analysis.json files can still be loaded

## Documentation Requirements

- **Code Documentation**: Update module documentation to clarify type usage
- **Architecture Updates**: Update ARCHITECTURE.md to show clear dependency
- **User Documentation**: No changes needed (internal refactor)

## Implementation Notes

1. Be careful with the conversion in `AnalysisRunnerImpl::run_analysis` - it currently converts typed fields to JSON values. This needs to be removed.

2. The cache implementation may need updates if it's storing JSON values.

3. Watch for any code that's accessing fields like `analysis.dependency_graph` and expecting a `serde_json::Value` - these will need to handle the typed version.

4. The test coverage field handling should be simplified since both versions now use `Option<T>`.

## Migration and Compatibility

No migration needed for users. The analysis.json file format remains unchanged since both versions serialize to the same JSON structure. This is purely an internal code refactoring.