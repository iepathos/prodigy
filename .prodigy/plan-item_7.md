# Implementation Plan: Reduce Nesting Complexity in FileTemplateStorage::list

## Problem Summary

**Location**: ./src/cook/workflow/composition/registry.rs:FileTemplateStorage::list:337
**Priority Score**: 31.89
**Debt Type**: ComplexityHotspot (Cognitive: 44, Cyclomatic: 12)
**Current Metrics**:
- Lines of Code: 45
- Cyclomatic Complexity: 12
- Cognitive Complexity: 44
- Nesting Depth: 5 levels

**Issue**: The `list()` function has excessive nesting (5 levels deep) that obscures the core logic. While cyclomatic complexity is manageable at 12, the deep nesting creates cognitive load. The function follows a pattern of nested conditionals: directory exists check → entry iteration → extension check → file stem extraction → metadata file check → metadata parsing attempt.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.0
- Coverage Improvement: 0.0
- Risk Reduction: 11.16

**Success Criteria**:
- [ ] Nesting depth reduced from 5 to 2-3 levels
- [ ] Cognitive complexity reduced by ~6 points
- [ ] All existing tests continue to pass (8 tests in registry.rs)
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Function remains pure and testable

## Implementation Phases

### Phase 1: Extract Template Entry Processing

**Goal**: Extract the nested logic for processing a single directory entry into a separate pure function.

**Changes**:
- Create new function `fn is_template_file(path: &Path) -> bool` that checks for .yml extension
- Create new function `fn extract_template_name(path: &Path) -> Option<String>` that extracts stem and filters out .meta files
- Create new async function `async fn load_template_metadata(&self, name: &str) -> TemplateMetadata` that handles metadata loading with fallback to default
- Use early returns in these helper functions to eliminate nesting

**Testing**:
- Run `cargo test --lib registry` to verify existing tests pass
- Verify function behavior unchanged

**Success Criteria**:
- [ ] Three new helper functions created
- [ ] Functions are simpler and more focused
- [ ] All 8 existing tests pass
- [ ] No clippy warnings

### Phase 2: Refactor list() to Use Helper Functions

**Goal**: Simplify the main `list()` function by using the extracted helpers and early returns.

**Changes**:
- Replace nested extension check with `is_template_file()`
- Replace nested file stem logic with `extract_template_name()`
- Replace nested metadata loading with `load_template_metadata()`
- Use `continue` statements for early loop continuation
- Reduce nesting from 5 levels to 2-3 levels

**Before pattern**:
```rust
while let Some(entry) = entries.next_entry().await? {
    if condition1 {
        if let Some(value) = condition2 {
            if !filter {
                if nested_condition {
                    // 5 levels deep
                }
            }
        }
    }
}
```

**After pattern**:
```rust
while let Some(entry) = entries.next_entry().await? {
    let path = entry.path();
    if !is_template_file(&path) {
        continue;
    }

    let Some(name) = extract_template_name(&path) else {
        continue;
    };

    let metadata = self.load_template_metadata(&name).await;
    // 2-3 levels max
}
```

**Testing**:
- Run `cargo test --lib registry` to verify all tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Nesting depth reduced to 2-3 levels
- [ ] Function is more readable
- [ ] All 8 existing tests pass
- [ ] No clippy warnings

### Phase 3: Optimize Metadata Loading Error Handling

**Goal**: Improve error handling in metadata loading to be more explicit and follow the project's error handling standards (no unwrap_or_default in production code).

**Changes**:
- Update `load_template_metadata()` to return `Result<TemplateMetadata>` instead of just `TemplateMetadata`
- Use proper error context with `.context()` for file read failures
- Use `.unwrap_or_else(|_| TemplateMetadata::default())` instead of `unwrap_or_default()`
- Ensure JSON parse errors are logged but don't fail the entire listing

**Testing**:
- Run `cargo test --lib registry` to verify all tests pass
- Verify error messages are clear and actionable

**Success Criteria**:
- [ ] Metadata loading uses Result type properly
- [ ] Errors have clear context messages
- [ ] Fallback to default metadata is explicit
- [ ] All 8 existing tests pass
- [ ] No use of unwrap() or unwrap_or_default()

### Phase 4: Final Verification and Cleanup

**Goal**: Ensure all quality standards are met and documentation is updated.

**Changes**:
- Add doc comments to new helper functions
- Verify all error messages are descriptive
- Run full test suite and linting
- Check that nesting depth metrics have improved

**Testing**:
- Run `cargo test` (full test suite)
- Run `cargo clippy -- -D warnings` (fail on any warnings)
- Run `cargo fmt --check` (verify formatting)
- Run `just ci` if available

**Success Criteria**:
- [ ] All helper functions have doc comments
- [ ] Full test suite passes
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Nesting depth reduced from 5 to 2-3 levels
- [ ] Ready for commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib registry` to verify the 8 existing tests in registry.rs pass
2. Run `cargo clippy` to check for warnings
3. Commit with clear message if phase succeeds

**Final verification**:
1. `cargo test` - Full test suite (not just registry tests)
2. `cargo clippy -- -D warnings` - Strict warning checks
3. `cargo fmt --check` - Formatting verification
4. `just ci` - Full CI checks if available
5. Verify nesting depth improvement with debtmap re-analysis

**Expected test coverage**:
- Existing tests should all pass unchanged
- No new tests required (function behavior unchanged)
- Tests cover: template registration, retrieval, metadata handling, listing, errors

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the specific failure:
   - Test failures: Check which test failed and why
   - Clippy warnings: Address the specific warning
   - Compilation errors: Fix syntax or type errors
3. Adjust the implementation approach:
   - If helper functions cause issues, inline them temporarily
   - If async changes cause problems, keep synchronous helpers
   - If error handling is too strict, relax with unwrap_or_else
4. Retry the phase with adjusted approach

## Notes

### Key Considerations:

1. **Pure vs I/O**: The `list()` function is marked as `PureLogic` role but contains I/O (file system operations). The refactoring should maintain this boundary - helper functions for logic should be pure where possible.

2. **Async Context**: The function is async and uses `tokio::fs` for file operations. Helper functions that perform I/O must also be async, but pure helpers (like `is_template_file`, `extract_template_name`) can be synchronous.

3. **Error Handling**: The project uses `anyhow::Result` and `.context()` for error messages. Metadata loading failures should not fail the entire listing - use fallback to default metadata.

4. **Existing Tests**: There are 8 tests in the registry module:
   - `test_template_registry`
   - `test_template_metadata`
   - `test_file_template_storage_load_with_metadata`
   - `test_file_template_storage_load_without_metadata`
   - `test_file_template_storage_load_missing_template`
   - `test_file_template_storage_load_invalid_yaml`
   - `test_file_template_storage_load_invalid_metadata_json`
   - `test_file_template_storage_load_corrupted_metadata`

   None of these directly test the `list()` function, so behavior changes won't break tests, but we should maintain exact behavior.

5. **Unwrap Usage**: The current code uses `.unwrap_or_default()` on line 362 for JSON parsing. Per Spec 101, this should be replaced with explicit error handling that logs failures but continues processing.

6. **Upstream Callers**: The function is called by:
   - `TemplateRegistry::list` (line 157)
   - `TemplateRegistry::load_all` (line 216)

   Both expect a `Result<Vec<TemplateInfo>>` and rely on the listing continuing even if individual templates have issues.

### Pattern to Follow:

The refactoring should follow this pattern from the codebase (like `load_metadata_if_exists`):
- Extract small, focused functions
- Use early returns and `?` operator for error propagation
- Provide meaningful error context with `.context()`
- Fall back gracefully for non-critical failures

### Success Indicators:

After implementation, the following should be true:
- Reading the `list()` function should be straightforward
- Each level of nesting should have a clear purpose
- Helper functions should be reusable and testable
- The function should follow the same patterns as other functions in the module
