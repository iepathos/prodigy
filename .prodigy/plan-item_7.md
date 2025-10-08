# Implementation Plan: Add Test Coverage for FileTemplateStorage::list

## Problem Summary

**Location**: ./src/cook/workflow/composition/registry.rs:FileTemplateStorage::list:329
**Priority Score**: 31.89
**Debt Type**: ComplexityHotspot (Cognitive: 44, Cyclomatic: 12)
**Current Metrics**:
- Lines of Code: 45
- Cyclomatic Complexity: 12
- Cognitive Complexity: 44
- Nesting Depth: 5
- Coverage: 0%

**Issue**: Add 12 tests for 100% coverage gap. NO refactoring needed (complexity 12 is acceptable). Complexity 12 is manageable. Coverage at 0%. Focus on test coverage, not refactoring.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.0
- Coverage Improvement: 0.0
- Risk Reduction: 11.16

**Success Criteria**:
- [ ] 12 focused tests covering all branches in FileTemplateStorage::list
- [ ] 100% coverage for the list() function
- [ ] Each test is <15 lines and tests ONE path
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Test Empty Directory and Non-Existent Directory

**Goal**: Cover the early return paths when directory doesn't exist or is empty

**Changes**:
- Add test for non-existent base directory (returns empty Vec)
- Add test for existing but empty directory
- Add test for directory with no .yml files

**Testing**:
```bash
cargo test --lib file_template_storage_list
```

**Success Criteria**:
- [ ] test_list_non_existent_directory passes
- [ ] test_list_empty_directory passes
- [ ] test_list_no_yml_files passes
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Test YAML File Detection and Filtering

**Goal**: Cover the path filtering logic for .yml extensions and .meta files

**Changes**:
- Add test for directory with valid .yml files
- Add test for directory with .meta.json files (should be skipped)
- Add test for mixed file types (.yml, .txt, .json)
- Add test for files without extensions

**Testing**:
```bash
cargo test --lib file_template_storage_list
```

**Success Criteria**:
- [ ] test_list_yml_files_only passes
- [ ] test_list_skips_meta_files passes
- [ ] test_list_mixed_file_types passes
- [ ] test_list_files_without_extensions passes
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Test Metadata Loading Paths

**Goal**: Cover the metadata loading logic with all branches (exists/doesn't exist, valid/invalid JSON)

**Changes**:
- Add test for template with valid metadata file
- Add test for template without metadata file (uses default)
- Add test for template with invalid/corrupted metadata JSON (uses default)
- Add test for template with unreadable metadata file (uses default)

**Testing**:
```bash
cargo test --lib file_template_storage_list
```

**Success Criteria**:
- [ ] test_list_with_valid_metadata passes
- [ ] test_list_without_metadata passes
- [ ] test_list_with_invalid_metadata_json passes
- [ ] test_list_with_unreadable_metadata passes
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 4: Test Complete Scenarios

**Goal**: Cover end-to-end scenarios with multiple templates and various metadata states

**Changes**:
- Add test for multiple templates with and without metadata
- Add test verifying TemplateInfo structure is correctly populated (name, description, version, tags)

**Testing**:
```bash
cargo test --lib file_template_storage_list
cargo test --lib workflow_composition_test
```

**Success Criteria**:
- [ ] test_list_multiple_templates passes
- [ ] test_list_populates_template_info_correctly passes
- [ ] All existing workflow_composition tests pass
- [ ] Ready to commit

### Phase 5: Verify Coverage and Final Validation

**Goal**: Ensure 100% coverage achieved and all quality gates pass

**Changes**:
- Run coverage analysis on FileTemplateStorage::list
- Verify all 12 branches are covered
- Run full test suite and clippy

**Testing**:
```bash
cargo tarpaulin --lib --packages prodigy --out Stdout -- file_template_storage_list
cargo test --lib
cargo clippy
cargo fmt --check
```

**Success Criteria**:
- [ ] FileTemplateStorage::list shows 100% line coverage
- [ ] All 12 test cases pass
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib file_template_storage_list` to verify new tests pass
2. Run `cargo test --lib workflow_composition_test` to ensure integration tests still work
3. Run `cargo clippy` to check for warnings
4. Commit after each phase with descriptive message

**Test Structure Pattern**:
```rust
#[tokio::test]
async fn test_list_<scenario>() -> Result<()> {
    // Setup: Create temp directory and FileTemplateStorage
    // Action: Call list() under specific conditions
    // Assert: Verify expected behavior (one assertion per test)
    Ok(())
}
```

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo tarpaulin --lib --packages prodigy` - Coverage report
3. `cargo clippy` - No warnings
4. `cargo fmt` - Code formatted

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure output
3. Adjust the test implementation
4. Retry the phase

If tests break existing functionality:
1. Review what existing behavior was inadvertently changed
2. Adjust tests to match actual contract/behavior
3. If behavior is wrong, create separate issue for bug fix

## Notes

### Key Branches to Cover:
1. Base directory doesn't exist → return empty Vec
2. Directory exists but empty → return empty Vec
3. File has no .yml extension → skip
4. File has .yml extension but no stem → skip
5. File stem ends with ".meta" → skip (metadata file)
6. Metadata path exists + read succeeds + parse succeeds → use parsed metadata
7. Metadata path exists + read succeeds + parse fails → use default
8. Metadata path exists + read fails → use default
9. Metadata path doesn't exist → use default
10. Valid template file with metadata → add to results
11. Multiple files in directory → iterate all
12. Async read_dir iteration completes → return results

### Test File Location:
- Add new test module at end of `tests/workflow_composition_test.rs`
- Or create new file: `tests/workflow_composition_storage_test.rs`
- Use `tempfile::TempDir` for test isolation

### Important Considerations:
- Use async/await correctly (function is async)
- Create realistic test fixtures (actual .yml and .meta.json files)
- Test both success and error paths
- Follow existing test patterns in workflow_composition_test.rs
- Ensure tests are deterministic and don't depend on external state
