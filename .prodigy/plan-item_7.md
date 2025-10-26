# Implementation Plan: Improve Testing and Refactor load_playbook_with_mapreduce

## Problem Summary

**Location**: ./src/cook/mod.rs:load_playbook_with_mapreduce:298
**Priority Score**: 15.56
**Debt Type**: TestingGap (coverage: 15%, cognitive complexity: 67, cyclomatic complexity: 16)
**Current Metrics**:
- Lines of Code: 125
- Cyclomatic Complexity: 16
- Nesting Depth: 6
- Coverage: 15.28%

**Issue**: Complex business logic with 85% coverage gap. Cyclomatic complexity of 16 requires at least 16 test cases for full path coverage. The function handles YAML/JSON parsing with extensive error handling and formatting logic, creating 60+ uncovered lines across multiple branches.

## Target State

**Expected Impact**:
- Complexity Reduction: 4.8 (from 16 to ~11)
- Coverage Improvement: 42.36% (from 15% to ~57%)
- Risk Reduction: 6.53

**Success Criteria**:
- [ ] Coverage increases from 15% to at least 57% (42 point improvement)
- [ ] At least 7-8 new test cases covering critical branches
- [ ] Extract 3-5 pure functions to reduce complexity
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Tests for Core Parsing Paths

**Goal**: Establish baseline test coverage for the main parsing branches (YAML MapReduce, YAML regular, JSON) to prevent regressions before refactoring.

**Changes**:
- Add test for successful MapReduce YAML parsing
- Add test for successful regular YAML parsing
- Add test for successful JSON parsing
- Add test for file extension detection (.yml, .yaml, .json)

**Testing**:
```bash
cargo test --lib load_playbook_with_mapreduce
cargo tarpaulin --lib --out Stdout | grep load_playbook_with_mapreduce
```

**Success Criteria**:
- [ ] 4 new tests passing
- [ ] Coverage increases to ~30%
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Add Tests for Error Handling Paths

**Goal**: Cover error scenarios and edge cases to reach 50%+ coverage.

**Changes**:
- Add test for MapReduce parse error (invalid structure)
- Add test for YAML parse error with location information
- Add test for JSON parse error
- Add test for file read failure (use non-existent path)
- Add test for YAML with invalid mode field

**Testing**:
```bash
cargo test --lib load_playbook_with_mapreduce
cargo tarpaulin --lib --out Stdout | grep load_playbook_with_mapreduce
```

**Success Criteria**:
- [ ] 5 new tests passing (9 total)
- [ ] Coverage increases to ~50%
- [ ] Error messages validated in tests
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Pure Error Formatting Functions

**Goal**: Reduce complexity by extracting error message construction into pure functions.

**Changes**:
- Extract `format_yaml_parse_error(e: &serde_yaml::Error, content: &str, path: &Path) -> String`
- Extract `format_mapreduce_parse_error(e: &anyhow::Error, path: &Path) -> String`
- Extract `format_json_parse_error(e: &serde_json::Error, path: &Path) -> String`
- Update main function to use extracted functions

**Testing**:
- Add unit tests for each formatting function
- Verify error messages still match expected format
```bash
cargo test --lib format_.*_error
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 3 formatting functions extracted
- [ ] Each formatting function has 2-3 tests
- [ ] Main function complexity reduced by ~3 points
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Extract File Type Detection Logic

**Goal**: Further reduce complexity by extracting file type detection into a pure function.

**Changes**:
- Extract `detect_file_format(path: &Path) -> FileFormat` enum
  - `FileFormat::YamlMapReduce`, `FileFormat::Yaml`, `FileFormat::Json`
- Add helper `is_mapreduce_content(content: &str) -> bool`
- Refactor main function to use detection logic

**Testing**:
- Add tests for file format detection (various extensions)
- Add tests for MapReduce content detection
```bash
cargo test --lib detect_file_format
cargo test --lib is_mapreduce_content
```

**Success Criteria**:
- [ ] File format detection extracted
- [ ] 4-5 tests for detection logic
- [ ] Main function complexity reduced by ~2 points
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 5: Extract Parsing Strategy Functions

**Goal**: Separate parsing logic into strategy functions, achieving target complexity reduction.

**Changes**:
- Extract `parse_mapreduce_yaml(content: &str, path: &Path) -> Result<...>`
- Extract `parse_regular_yaml(content: &str, path: &Path) -> Result<...>`
- Extract `parse_json(content: &str, path: &Path) -> Result<...>`
- Refactor main function to delegate to strategy functions

**Testing**:
- Add integration tests for each parsing strategy
- Verify all parsing paths work end-to-end
```bash
cargo test --lib parse_mapreduce_yaml
cargo test --lib parse_regular_yaml
cargo test --lib parse_json
cargo tarpaulin --lib --out Stdout | grep "load_playbook_with_mapreduce\|parse_.*_yaml\|parse_json"
```

**Success Criteria**:
- [ ] 3 parsing strategy functions extracted
- [ ] Each strategy has 2-3 tests
- [ ] Main function complexity reduced to ~11 or less
- [ ] Coverage reaches 57%+ target
- [ ] All existing tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo tarpaulin --lib` to verify coverage improvement
4. Check that error messages are preserved and helpful

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib --out Stdout` - Verify 57%+ coverage
3. Verify cyclomatic complexity reduced to ~11 (can check with cargo-complexity if available)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or clippy warnings
3. Adjust the implementation
4. Retry the phase

## Notes

**Key Complexity Sources**:
- Nested conditionals for file type detection (extension checks)
- Three separate parsing paths (MapReduce YAML, regular YAML, JSON)
- Extensive error formatting with context extraction
- Line number and column number extraction for error messages

**Refactoring Strategy**:
- Test first to prevent regressions
- Extract error formatting (most complex, most lines)
- Extract file type detection (reduces nesting)
- Extract parsing strategies (reduces branching)

**Preservation Requirements**:
- Must maintain helpful error messages with line/column info
- Must preserve file content display in errors
- Must maintain backward compatibility with existing workflows
- Must handle all current file formats (YAML, JSON)

**Testing Priorities**:
1. Core happy paths (Phase 1) - prevent basic regressions
2. Error paths (Phase 2) - ensure errors are helpful
3. Extracted functions (Phases 3-5) - verify pure logic works independently
