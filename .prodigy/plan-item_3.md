# Implementation Plan: Refactor Events CLI God Module

## Problem Summary

**Location**: ./src/cli/events/mod.rs:file:0
**Priority Score**: 103.18
**Debt Type**: God Object (File-level)
**Current Metrics**:
- Lines of Code: 1793
- Functions: 81
- Cyclomatic Complexity: 284 (average 3.5 per function)
- Max Complexity: 15
- Coverage: 0.0%
- Responsibilities: 6 (Processing, Utilities, Parsing & Input, Filtering & Selection, Construction, Data Access)

**Issue**: URGENT - This 1793-line module with 81 functions violates the single responsibility principle. It mixes command routing, file I/O, event processing, display logic, retention analysis, and test code. The debtmap recommendation is to split by data flow: 1) Input/parsing functions 2) Core logic/transformation 3) Output/formatting into 3 focused modules with <30 functions each.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 56.8
- Maintainability Improvement: 10.32
- Test Effort: 179.3

**Success Criteria**:
- [ ] Main module reduced to <200 lines (command routing only)
- [ ] Each new module has <30 functions and single clear responsibility
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt
- [ ] All functions remain testable
- [ ] Pure functions separated from I/O operations

## Implementation Phases

### Phase 1: Extract File I/O Operations

**Goal**: Create a new `io.rs` module with all file system operations, separating I/O from business logic.

**Changes**:
- Create `src/cli/events/io.rs`
- Move all file reading/writing functions:
  - `find_event_files`
  - `read_events_from_files`
  - `read_events_from_single_file`
  - `read_and_filter_events`
  - `get_all_event_files`
  - `display_existing_events`
  - `display_new_events`
- Move path resolution functions:
  - `resolve_job_event_file`
  - `resolve_event_file_with_fallback`
  - `build_global_events_path`
  - `determine_watch_path`
- Add proper documentation to each function
- Update `mod.rs` to use `io` module functions

**Testing**:
- Run `cargo test --lib` to verify no breakage
- Run `cargo clippy` to check for warnings
- Verify file reading still works: `cargo run -- events list --help`

**Success Criteria**:
- [ ] New `io.rs` module created with ~15-20 functions
- [ ] All file I/O operations moved out of main module
- [ ] All tests pass
- [ ] Main module reduced by ~300 lines
- [ ] Ready to commit

### Phase 2: Extract Job Status and Analysis Logic

**Goal**: Create `src/cli/events/analysis.rs` module for job status analysis and retention logic.

**Changes**:
- Create `src/cli/events/analysis.rs`
- Move job status functions:
  - `get_available_jobs`
  - `read_job_status`
  - `process_event_for_status`
- Move retention analysis functions:
  - `analyze_retention_targets`
  - `aggregate_job_retention`
  - `get_job_directories`
  - `should_analyze_global_storage` (pure function)
- Move aggregation functions:
  - `show_aggregated_stats`
  - `search_aggregated_events`
  - `export_aggregated_events`
- Add comprehensive documentation

**Testing**:
- Run `cargo test --lib`
- Test job listing: `cargo run -- events list`
- Test stats: `cargo run -- events stats`
- Run `cargo clippy`

**Success Criteria**:
- [ ] New `analysis.rs` module created with ~12-15 functions
- [ ] Job status and retention analysis separated from main module
- [ ] All tests pass
- [ ] Main module reduced by another ~400 lines
- [ ] Ready to commit

### Phase 3: Extract Event Display Functions

**Goal**: Consolidate all display/output functions into existing `format.rs` module or create new `display.rs`.

**Changes**:
- Evaluate if `format.rs` is appropriate or create `src/cli/events/display.rs`
- Move display functions:
  - `display_available_jobs`
  - `display_event`
  - `display_job_started`
  - `display_job_completed`
  - `display_agent_progress`
  - `display_generic_event`
  - `display_retention_policy`
- Move user interaction:
  - `confirm_cleanup`
- Consolidate with existing format functions where appropriate
- Add clear separation between formatting (pure) and display (I/O)

**Testing**:
- Run `cargo test --lib`
- Test event display: `cargo run -- events list --file <test-file>`
- Test cleanup dry-run: `cargo run -- events clean --dry-run --older-than 30d`
- Run `cargo clippy`

**Success Criteria**:
- [ ] Display functions consolidated (either in format.rs or new display.rs)
- [ ] ~10-15 display functions moved
- [ ] Clear separation between formatting and displaying
- [ ] All tests pass
- [ ] Main module reduced by another ~250 lines
- [ ] Ready to commit

### Phase 4: Extract Cleanup/Retention Operations

**Goal**: Create `src/cli/events/cleanup.rs` module for event cleanup and retention operations.

**Changes**:
- Create `src/cli/events/cleanup.rs`
- Move cleanup functions:
  - `clean_events`
  - `build_retention_policy`
  - `clean_specific_file`
  - `process_event_file_dry_run`
  - `process_event_file_actual`
  - `process_job_directory`
  - `clean_global_storage`
  - `clean_local_storage`
- Ensure these functions use the `io` and `analysis` modules
- Document retention policy construction

**Testing**:
- Run `cargo test --lib`
- Test cleanup dry-run: `cargo run -- events clean --dry-run --older-than 7d`
- Test cleanup with file: `cargo run -- events clean --file <test-file> --dry-run`
- Run `cargo clippy`

**Success Criteria**:
- [ ] New `cleanup.rs` module created with ~10 functions
- [ ] All cleanup operations separated from main module
- [ ] All tests pass
- [ ] Main module reduced by another ~400 lines
- [ ] Ready to commit

### Phase 5: Simplify Main Module to Command Router

**Goal**: Reduce main `mod.rs` to pure command routing and coordination, delegating all work to specialized modules.

**Changes**:
- Keep only in `mod.rs`:
  - `EventsArgs` and `EventsCommand` structs (lines 16-158)
  - `execute()` function as command router (simplified)
  - Module declarations and re-exports
  - Watch/follow functions (as they coordinate multiple modules)
- Update `execute()` to delegate to specialized modules:
  - `list_events` → use `io` + `format`
  - `show_stats` → use `analysis` + `format`
  - `search_events` → use `io` + `transform` + `format`
  - `clean_events` → use `cleanup`
- Move remaining orchestration functions to appropriate modules
- Reduce test count to only integration-style tests

**Testing**:
- Run full test suite: `cargo test --lib`
- Test each command type:
  - `cargo run -- events list`
  - `cargo run -- events stats`
  - `cargo run -- events search "test"`
  - `cargo run -- events follow --help`
  - `cargo run -- events export --format json`
  - `cargo run -- events clean --dry-run --older-than 7d`
- Run `cargo clippy`
- Run `cargo fmt`

**Success Criteria**:
- [ ] Main `mod.rs` reduced to ~150-200 lines
- [ ] Only command routing and module coordination remains
- [ ] All commands delegate to specialized modules
- [ ] All 81 functions redistributed across 5-6 modules
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Manual testing of affected CLI commands

**Phase-specific testing**:
- Phase 1: Test file operations with actual event files
- Phase 2: Test job listing and stats commands
- Phase 3: Test event display in various formats
- Phase 4: Test cleanup commands with dry-run
- Phase 5: Full integration testing of all commands

**Final verification**:
1. `cargo test --lib --all-features` - All tests pass
2. `cargo clippy -- -D warnings` - No warnings
3. `cargo fmt --check` - Proper formatting
4. Manual testing of key user flows:
   - Listing events for a job
   - Viewing statistics
   - Searching events
   - Following events in real-time
   - Exporting events
   - Cleaning old events
5. Verify module structure:
   - `mod.rs`: <200 lines (command routing)
   - `io.rs`: ~15-20 functions (file operations)
   - `analysis.rs`: ~12-15 functions (status & retention)
   - `format.rs`: existing + ~10 display functions
   - `cleanup.rs`: ~10 functions (retention operations)
   - `transform.rs`: existing (pure transformations)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation or test errors
3. Identify the issue (missing import, function signature mismatch, etc.)
4. Adjust the plan to address the issue
5. Retry the phase with corrections

## Notes

**Key Architectural Principles**:
- **Separation of Concerns**: I/O in `io.rs`, logic in `analysis.rs`, display in `format.rs`
- **Pure Functions**: Keep transformation logic in `transform.rs` (already exists)
- **Single Responsibility**: Each module has one clear purpose
- **Testability**: All functions remain independently testable

**Module Responsibilities After Refactoring**:
- `mod.rs`: Command routing and coordination (~150-200 lines)
- `io.rs`: File system operations (read/write/find) (~15-20 functions)
- `analysis.rs`: Job status, retention analysis, aggregation (~12-15 functions)
- `format.rs`: Formatting and display functions (~30+ functions total)
- `cleanup.rs`: Retention policy and cleanup operations (~10 functions)
- `transform.rs`: Pure transformation functions (already exists)

**Potential Challenges**:
1. **Circular dependencies**: Ensure modules depend only downward (io → transform, analysis → io, cleanup → io/analysis)
2. **Function signatures**: May need to adjust parameters when moving between modules
3. **Test imports**: Update test imports to reference new module paths
4. **Watch/follow functions**: These coordinate multiple modules, keep in main or create separate module

**Success Indicators**:
- Each module has <30 functions
- Main module is <200 lines
- Total line count unchanged or slightly reduced
- All tests pass without modification
- No clippy warnings
- Clear module boundaries and responsibilities
