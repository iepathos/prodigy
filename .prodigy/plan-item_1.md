# Implementation Plan: Split Events CLI God Object into Focused Modules

## Problem Summary

**Location**: ./src/cli/events.rs:EventsCommand:14
**Priority Score**: 194.94196251207643
**Debt Type**: God Object + High Complexity
**Current Metrics**:
- Lines of Code: 2647
- Functions: 124
- Cyclomatic Complexity: 397 (avg 3.2 per function, max 15)
- Coverage: 8.87%

**Issue**: This file is a classic god object with 6 distinct responsibilities mixed together:
1. CLI command parsing and routing
2. File I/O and event reading
3. Data transformation and filtering
4. Event display formatting
5. Statistics aggregation
6. Retention policy management

The debtmap analysis identifies this as "URGENT" with recommended splits into:
- Core Operations module (~1900 lines, 96 functions)
- Data Access module (~120 lines, 6 functions)

However, a better split follows the data flow pattern: Input → Transform → Output.

## Target State

**Expected Impact**:
- Complexity Reduction: 79.4 points
- Maintainability Improvement: 19.49%
- Test Effort: 241.2 (high due to current low coverage)

**Success Criteria**:
- [ ] 4 focused modules created (CLI, I/O, Transform, Format)
- [ ] Each module has <30 functions and clear responsibility
- [ ] Pure functions separated from I/O functions
- [ ] All existing tests continue to pass (no modifications)
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Coverage maintained or improved

## Implementation Phases

### Phase 1: Extract Pure Transformation Functions

**Goal**: Create `src/cli/events/transform.rs` with all pure data transformation logic.

**Changes**:
- Create new module `src/cli/events/transform.rs`
- Move pure transformation functions:
  - `calculate_event_statistics`
  - `sort_statistics_by_count`
  - `event_matches_search`
  - `parse_event_line`
  - `convert_duration_to_days`
  - `convert_size_to_bytes`
  - `validate_retention_policy`
  - `event_matches_field`
  - `event_matches_type`
  - `event_is_recent`
  - `get_event_type`
  - `extract_timestamp`
  - `extract_nested_field`
  - `extract_job_id`
  - `extract_agent_id`
  - `extract_event_metadata`
  - `search_in_value`
  - `search_events_with_pattern`
  - `calculate_archived_count`
  - `aggregate_stats`
  - `extract_job_name`
- Move `EventFilter` struct and implementation
- Add module declaration to `src/cli/events.rs`
- Update imports in `src/cli/events.rs`

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo clippy` to check for warnings
- Verify all 15 existing tests in the file still pass

**Success Criteria**:
- [ ] New module created with ~20 pure functions
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Functions are properly documented
- [ ] Module is public and exports needed types

### Phase 2: Extract Formatting Functions

**Goal**: Create `src/cli/events/format.rs` with all output formatting logic.

**Changes**:
- Create new module `src/cli/events/format.rs`
- Move formatting functions:
  - `format_statistics_human`
  - `format_statistics_json`
  - `format_statistics_yaml`
  - `format_job_info` / `create_job_display_info`
  - `calculate_duration`
  - `calculate_elapsed`
  - `format_timestamp`
  - `create_cleanup_summary_message`
  - `create_cleanup_summary_json`
  - `create_cleanup_summary_human`
  - `display_job_started`
  - `display_job_completed`
  - `display_agent_progress`
  - `display_generic_event`
  - `display_event`
  - `format_event_details`
  - `print_table_header`
  - `print_event_row`
  - `extract_table_row_data`
  - `truncate_field`
  - `display_events_as_table`
  - `display_events_with_format`
  - `display_statistics_with_format`
  - `display_search_results`
  - `export_as_json`
  - `export_as_csv`
  - `export_as_markdown`
- Move `JobInfo` and `JobStatus` types
- Add module declaration
- Update imports

**Testing**:
- Run `cargo test --lib`
- Run `cargo clippy`
- Verify output formatting tests pass

**Success Criteria**:
- [ ] New module created with ~28 formatting functions
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Clear separation between pure formatting and I/O

### Phase 3: Extract I/O and Data Access Functions

**Goal**: Create `src/cli/events/io.rs` with all file I/O operations.

**Changes**:
- Create new module `src/cli/events/io.rs`
- Move I/O functions:
  - `get_available_jobs`
  - `read_job_status`
  - `process_event_for_status`
  - `find_event_files`
  - `resolve_job_event_file`
  - `resolve_event_file_with_fallback`
  - `get_all_event_files`
  - `read_and_filter_events`
  - `read_events_from_files`
  - `read_events_from_single_file`
  - `get_job_directories`
  - `display_existing_events`
  - `display_new_events`
  - `setup_file_for_watching`
  - `setup_file_watcher`
  - `determine_watch_path`
- Add module declaration
- Update imports

**Testing**:
- Run `cargo test --lib`
- Run `cargo clippy`
- Focus on I/O-related functionality

**Success Criteria**:
- [ ] New module created with ~15 I/O functions
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Clear separation between I/O and business logic

### Phase 4: Extract CLI Command Handlers

**Goal**: Create `src/cli/events/commands.rs` with command execution logic.

**Changes**:
- Create new module `src/cli/events/commands.rs`
- Move command execution functions:
  - `list_events`
  - `show_stats`
  - `show_aggregated_stats`
  - `search_events`
  - `search_aggregated_events`
  - `follow_events`
  - `watch_existing_file`
  - `wait_for_file_creation`
  - `export_events`
  - `export_aggregated_events`
  - `clean_events`
  - `build_retention_policy`
  - `analyze_retention_targets`
  - `confirm_cleanup`
  - `display_retention_policy`
  - `clean_specific_file`
  - `clean_global_storage`
  - `clean_local_storage`
  - `display_cleanup_summary`
  - `process_job_directory`
  - `process_event_file_dry_run`
  - `process_event_file_actual`
  - `display_available_jobs`
- Add module declaration
- Update imports

**Testing**:
- Run `cargo test --lib`
- Run `cargo clippy`
- Verify command execution logic works

**Success Criteria**:
- [ ] New module created with ~25 command functions
- [ ] All tests pass without modification
- [ ] No clippy warnings
- [ ] Command handlers delegate to transform/format/io modules

### Phase 5: Refactor Main Events Module

**Goal**: Clean up `src/cli/events.rs` to be a thin orchestration layer.

**Changes**:
- Keep only in `src/cli/events.rs`:
  - `EventsArgs` and `EventsCommand` types
  - `execute()` function (main entry point)
  - Module declarations
  - Re-exports of public types
- Add module structure:
  ```rust
  pub mod commands;
  pub mod format;
  pub mod io;
  pub mod transform;

  pub use format::{JobInfo, JobStatus};
  pub use transform::EventFilter;
  ```
- Update `execute()` to delegate to command handlers
- Remove all moved code
- Verify imports are correct

**Testing**:
- Run `cargo test --lib` - all tests must pass
- Run `cargo clippy` - no warnings
- Run `cargo fmt` - ensure proper formatting
- Manual test: `cargo run -- events ls` to verify CLI works

**Success Criteria**:
- [ ] Main file reduced to <200 lines
- [ ] Clear module structure
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] CLI commands work correctly
- [ ] Ready to commit

## Module Structure (Final State)

```
src/cli/events/
├── mod.rs (thin orchestration layer, ~150 lines)
│   ├── EventsArgs, EventsCommand types
│   ├── execute() function
│   └── module declarations
├── transform.rs (pure transformation logic, ~400 lines)
│   ├── EventFilter
│   ├── calculate_event_statistics
│   ├── event matching functions
│   └── data transformation functions
├── format.rs (output formatting, ~600 lines)
│   ├── JobInfo, JobStatus types
│   ├── format_statistics_*
│   ├── display_* functions
│   └── export_* functions
├── io.rs (file I/O operations, ~400 lines)
│   ├── get_available_jobs
│   ├── read_* functions
│   ├── find_event_files
│   └── file watching setup
└── commands.rs (command handlers, ~800 lines)
    ├── list_events
    ├── show_stats
    ├── search_events
    ├── follow_events
    ├── export_events
    └── clean_events
```

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Verify no test modifications are needed

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Code is formatted
4. Manual CLI test: `cargo run -- events ls`
5. Coverage check: `cargo tarpaulin` (maintain or improve 8.87%)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure cause
3. Identify missing dependencies or imports
4. Retry with corrections

## Notes

### Why This Split Pattern?

The data flow pattern (Input → Transform → Output) is superior to the debtmap suggestion because:
- **Transform module**: Pure functions with no I/O, highly testable
- **Format module**: Pure output formatting, no business logic
- **I/O module**: Centralized file operations, easy to mock
- **Commands module**: Thin orchestration, delegates to other modules

### Key Dependencies

- Transform module: No dependencies on other modules
- Format module: Depends on Transform for types (EventFilter, JobInfo)
- I/O module: Depends on Transform for filtering logic
- Commands module: Depends on all three modules

### Testing Approach

The current test suite is small (15 tests) and focused on pure functions. This refactoring:
- Preserves all existing tests
- Makes it easier to add tests for pure functions
- Maintains the same public API
- Enables future test improvements

### Coverage Improvement Strategy

After refactoring, focus on:
1. Testing pure transformation functions (easy wins)
2. Testing formatting functions (pure, no I/O)
3. Mocking I/O for command handler tests
4. Integration tests for CLI workflows
