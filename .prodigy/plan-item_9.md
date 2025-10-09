# Implementation Plan: Add Test Coverage for FileTemplateStorage::load

## Problem Summary

**Location**: ./src/cook/workflow/composition/registry.rs:FileTemplateStorage::load:299
**Priority Score**: 30.99
**Debt Type**: ComplexityHotspot (Cognitive: 21, Cyclomatic: 7)
**Current Metrics**:
- Lines of Code: 29
- Cyclomatic Complexity: 7
- Coverage: 0%
- Function Length: 29 lines

**Issue**: Add 7 tests for 100% coverage gap. NO refactoring needed (complexity 7 is acceptable). The function has 0% test coverage despite being a critical path for loading workflow templates.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.5
- Coverage Improvement: 0% → 100%
- Risk Reduction: 10.85

**Success Criteria**:
- [ ] 7 focused tests covering all decision branches
- [ ] Each test is <15 lines and tests ONE path
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Code coverage reaches 100% for this function
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Happy Path Tests

**Goal**: Establish baseline test coverage for successful load scenarios

**Changes**:
- Add test for loading template with metadata file present
- Add test for loading template without metadata file (uses default)

**Tests**:
- `test_file_template_storage_load_with_metadata` - Creates temp dir, stores template+metadata, loads and verifies both
- `test_file_template_storage_load_without_metadata` - Creates temp dir, stores only template YAML, loads and verifies default metadata

**Success Criteria**:
- [ ] Both happy path tests pass
- [ ] Tests are independently runnable
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 2: Add Error Path Tests - Template File Issues

**Goal**: Cover error scenarios related to template file reading and parsing

**Changes**:
- Add test for missing template file
- Add test for malformed YAML in template file

**Tests**:
- `test_file_template_storage_load_missing_template` - Attempts to load non-existent template, expects error
- `test_file_template_storage_load_invalid_yaml` - Creates template with invalid YAML, expects parse error

**Success Criteria**:
- [ ] Error tests properly validate failure cases
- [ ] Tests check for appropriate error messages
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Add Error Path Tests - Metadata File Issues

**Goal**: Cover error scenarios related to metadata file reading and parsing

**Changes**:
- Add test for corrupted metadata file (exists but can't be read)
- Add test for malformed JSON in metadata file
- Add test for metadata file with invalid structure

**Tests**:
- `test_file_template_storage_load_invalid_metadata_json` - Creates template with malformed JSON metadata, expects error
- `test_file_template_storage_load_corrupted_metadata` - Creates template with invalid metadata structure, expects error or fallback

**Success Criteria**:
- [ ] Metadata error paths are covered
- [ ] Tests verify error handling behavior
- [ ] All tests pass
- [ ] Coverage reaches 100% for the function
- [ ] Ready to commit

### Phase 4: Verification and Documentation

**Goal**: Ensure complete coverage and document testing approach

**Changes**:
- Run coverage analysis to confirm 100% coverage
- Add module-level documentation for test strategy
- Verify all tests follow project conventions

**Testing**:
- Run `cargo test --lib -- registry` to verify all registry tests pass
- Run `cargo tarpaulin --lib` to measure coverage
- Verify coverage report shows 100% for `FileTemplateStorage::load`

**Success Criteria**:
- [ ] Coverage report confirms 100% coverage
- [ ] All 7+ tests are clear and focused
- [ ] No clippy warnings
- [ ] Tests follow existing patterns in the codebase
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests in the existing `#[cfg(test)] mod tests` section
2. Use `tempfile::TempDir` for isolated test environments (existing pattern)
3. Run `cargo test --lib -- registry::tests` to verify only these tests
4. Run `cargo test --lib` to ensure no regressions
5. Run `cargo clippy` to check for warnings

**Test Structure**:
- Each test should be async (`#[tokio::test]`)
- Use descriptive names: `test_file_template_storage_load_<scenario>`
- Keep tests focused: one scenario per test
- Use existing test utilities (TempDir, ComposableWorkflow::from_config)

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo tarpaulin --lib` - Verify coverage improvement
4. `cargo fmt` - Ensure formatting

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or error
3. Adjust the test implementation
4. Re-run tests before committing

## Notes

- **No refactoring needed**: The recommendation explicitly states complexity 7 is acceptable
- **Focus on coverage**: Priority is to test existing behavior, not change it
- **Use existing patterns**: The codebase already has test patterns with TempDir and async tests
- **Error context**: Tests should verify that error messages include helpful context
- **Metadata handling**: Pay special attention to the metadata_path.exists() branch (line 139)
- **Default behavior**: Verify that TemplateMetadata::default() is used when metadata file is missing

**Key Decision Branches to Cover**:
1. Template file read success vs failure
2. Template YAML parse success vs failure
3. Metadata file exists vs doesn't exist
4. Metadata file read success vs failure (if exists)
5. Metadata JSON parse success vs failure (if exists)
6. Default metadata fallback path
7. Final success path with TemplateEntry construction
