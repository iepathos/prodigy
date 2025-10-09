# Implementation Plan: Reduce Cognitive Complexity in FileTemplateStorage::load

## Problem Summary

**Location**: ./src/cook/workflow/composition/registry.rs:FileTemplateStorage::load:299
**Priority Score**: 39.99
**Debt Type**: ComplexityHotspot (Cognitive: 21, Cyclomatic: 7)

**Current Metrics**:
- Lines of Code: 29
- Cyclomatic Complexity: 7
- Cognitive Complexity: 21
- Function Length: 29 lines
- Function Role: PureLogic (80% purity confidence)
- Coverage: Strong test coverage (5 test cases covering various error scenarios)

**Issue**: The function has high cognitive complexity (21) due to nested async I/O operations, complex error context wrapping, and conditional metadata loading. While cyclomatic complexity (7) is manageable, the cognitive load makes the function harder to understand and maintain. The recommendation suggests maintaining simplicity and prioritizing test coverage, but we can reduce complexity while preserving all existing tests.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.5
- Coverage Improvement: 0.0 (maintain existing coverage)
- Risk Reduction: 13.996304951684996

**Success Criteria**:
- [ ] Reduce cognitive complexity from 21 to ~10-15
- [ ] Reduce cyclomatic complexity from 7 to ~5
- [ ] Extract at least 2 pure helper functions
- [ ] All 5 existing test cases continue to pass without modification
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Function length reduced to ~15-20 lines

## Implementation Phases

### Phase 1: Extract Metadata Loading Logic

**Goal**: Separate metadata loading into a dedicated helper function to reduce nesting and improve readability.

**Changes**:
1. Create new private function `load_metadata_if_exists` that:
   - Takes `&self` and `name: &str` as parameters
   - Returns `Result<TemplateMetadata>`
   - Handles the conditional metadata file reading
   - Provides clear error context for metadata parsing failures

2. Update `load` function to call the new helper:
   - Replace lines 310-320 with a single call to `load_metadata_if_exists`
   - Simplifies the main function's control flow
   - Reduces nesting depth from 2 to 1

**Testing**:
- Run `cargo test registry::tests::test_file_template_storage_load_with_metadata` - should pass
- Run `cargo test registry::tests::test_file_template_storage_load_without_metadata` - should pass
- Run `cargo test registry::tests::test_file_template_storage_load_invalid_metadata_json` - should pass
- Run `cargo test registry::tests::test_file_template_storage_load_corrupted_metadata` - should pass

**Success Criteria**:
- [ ] `load_metadata_if_exists` function created and working
- [ ] Main `load` function simplified
- [ ] All 5 existing tests pass
- [ ] Cognitive complexity reduced by ~5-7 points
- [ ] Code compiles without warnings
- [ ] Ready to commit

### Phase 2: Extract Template Loading Logic

**Goal**: Separate template YAML loading into a dedicated helper function to further reduce complexity.

**Changes**:
1. Create new private function `load_template_yaml` that:
   - Takes `&self` and `name: &str` as parameters
   - Returns `Result<ComposableWorkflow>`
   - Handles reading and parsing the template YAML file
   - Provides clear error context for file and parsing failures

2. Update `load` function to call the new helper:
   - Replace lines 301-307 with a single call to `load_template_yaml`
   - Further simplifies the main function
   - Creates clear separation between template and metadata loading

**Testing**:
- Run full test suite: `cargo test registry::tests` - all 5 storage tests should pass
- Verify error messages are preserved:
  - `test_file_template_storage_load_missing_template` - should still show "Failed to read template file"
  - `test_file_template_storage_load_invalid_yaml` - should still show "Failed to parse template YAML"

**Success Criteria**:
- [ ] `load_template_yaml` function created and working
- [ ] Main `load` function now consists of 3 clear steps: load template, load metadata, construct entry
- [ ] All 5 existing tests pass
- [ ] Error messages preserved for debugging
- [ ] Cognitive complexity reduced by another ~3-5 points
- [ ] Code compiles without warnings
- [ ] Ready to commit

### Phase 3: Simplify Main Function and Final Validation

**Goal**: Ensure the refactored `load` function is clear, concise, and maintainable.

**Changes**:
1. Review the refactored `load` function structure:
   - Should now be ~12-15 lines
   - Three main steps: load template, load metadata, construct result
   - Minimal nesting (0-1 levels)
   - Clear error propagation with `?` operator

2. Add inline documentation if needed:
   - Brief doc comment for `load_metadata_if_exists` explaining conditional loading
   - Brief doc comment for `load_template_yaml` explaining template parsing

3. Run full validation suite:
   - `cargo fmt` - ensure consistent formatting
   - `cargo clippy` - check for any warnings
   - `cargo test --lib` - verify all tests pass
   - Review diff to ensure changes are minimal and focused

**Testing**:
- Full test suite: `cargo test registry::tests`
- Clippy check: `cargo clippy --tests`
- Format check: `cargo fmt --check`
- Manual review of test output to ensure all assertions pass

**Success Criteria**:
- [ ] Main `load` function is ~12-15 lines
- [ ] Nesting depth reduced to 0-1 levels
- [ ] All 5 existing tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Cognitive complexity target achieved (~10-15)
- [ ] Cyclomatic complexity target achieved (~5)
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run targeted tests first:
   ```bash
   cargo test registry::tests::test_file_template_storage_load
   ```
2. Run clippy to check for warnings:
   ```bash
   cargo clippy --tests -- -D warnings
   ```
3. Format code:
   ```bash
   cargo fmt
   ```

**Final verification**:
1. Full test suite:
   ```bash
   cargo test --lib
   ```
2. Full clippy check:
   ```bash
   cargo clippy -- -D warnings
   ```
3. Verify all 5 template storage tests pass:
   - `test_file_template_storage_load_with_metadata`
   - `test_file_template_storage_load_without_metadata`
   - `test_file_template_storage_load_missing_template`
   - `test_file_template_storage_load_invalid_yaml`
   - `test_file_template_storage_load_invalid_metadata_json`
   - `test_file_template_storage_load_corrupted_metadata`

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure output
3. Check if error messages need adjustment
4. Verify helper function signatures match usage
5. Retry with corrections

## Notes

**Key Considerations**:
1. **Preserve Error Messages**: The existing tests check for specific error message patterns (e.g., "Failed to read template file", "Failed to parse template YAML"). The helper functions must use identical error context strings.

2. **Async Operations**: All file I/O operations are async. The helper functions must be `async fn` and properly awaited.

3. **Test Coverage**: The function already has excellent test coverage (5 test cases covering happy path and 4 error scenarios). Our goal is to reduce complexity WITHOUT changing test behavior.

4. **Functional Programming**: The helper functions will be pure transformations (file path → Result<T>), making them easier to test and reason about.

5. **Incremental Approach**: Each phase reduces complexity incrementally while maintaining a working, testable state. This aligns with the "incremental progress over big bangs" philosophy.

**Root Causes of High Cognitive Complexity**:
1. Mixed I/O operations and error handling in a single function
2. Nested error context wrapping creates deep nesting
3. Conditional metadata loading adds branching complexity
4. No separation between file reading logic and business logic

**Expected Outcome**:
After all phases, the `load` function will be a clear, high-level orchestrator:
```rust
async fn load(&self, name: &str) -> Result<TemplateEntry> {
    let template = self.load_template_yaml(name).await?;
    let metadata = self.load_metadata_if_exists(name).await?;

    Ok(TemplateEntry {
        name: name.to_string(),
        template,
        metadata,
    })
}
```

This structure has:
- Cognitive complexity: ~10 (down from 21)
- Cyclomatic complexity: ~3 (down from 7)
- Clear separation of concerns
- Easy to understand and maintain
- All tests passing
