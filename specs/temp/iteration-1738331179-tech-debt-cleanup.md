# Iteration 1738331179: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from MMM context analysis.

## Debt Items to Address

### 1. High Complexity in extract_spec_from_git
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/cook/workflow.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 34, making it difficult to maintain and test.

#### Required Changes:
- Split into smaller, focused functions
- Extract git operations into separate helper functions
- Create dedicated spec parsing logic
- Separate error handling paths

#### Implementation Steps:
- Extract `parse_spec_from_commit` helper function for commit message parsing
- Create `validate_spec_content` function for spec validation
- Move git operations to `GitSpecExtractor` struct
- Add unit tests for each extracted component
- Ensure test coverage remains at or above current level

### 2. High Complexity in execute_structured_command
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/cook/workflow.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 33, handling multiple command types in a single function.

#### Required Changes:
- Use command pattern to separate command execution logic
- Create individual command handlers
- Implement proper error propagation

#### Implementation Steps:
- Create `CommandHandler` trait with execute method
- Implement specific handlers: `AnalyzeHandler`, `SpecHandler`, `ReviewHandler`
- Use match statement with handler dispatch
- Add integration tests for each command type

### 3. High Complexity in run_standard
**Impact Score**: 8/10
**Effort Score**: 5/10
**Category**: Complexity
**File**: src/cook/mod.rs
**Priority**: Critical

#### Current State:
Function has cyclomatic complexity of 30, mixing multiple concerns.

#### Required Changes:
- Extract iteration logic into separate module
- Create workflow orchestrator
- Separate metrics collection from main flow

#### Implementation Steps:
- Create `IterationRunner` struct to handle iteration logic
- Extract `MetricsCollector` for metrics gathering
- Implement `WorkflowOrchestrator` for coordinating steps
- Add comprehensive error context with `.context()`
- Replace unwrap() calls with proper error handling

### 4. Deprecated Comment Markers
**Impact Score**: 8/10
**Effort Score**: 3/10
**Category**: Deprecated
**Files**: src/context/debt.rs (lines 57, 179, 180, 198), src/main.rs (line 179)
**Priority**: High

#### Current State:
Multiple deprecated comment markers without clear documentation of what's deprecated.

#### Required Changes:
- Review each deprecated marker
- Either remove deprecated code or document migration path
- Update all call sites if removing

#### Implementation Steps:
- Analyze usage of deprecated code sections
- Document replacement approaches in comments
- Create migration guide if needed
- Remove or properly annotate with `#[deprecated]` attribute

### 5. FIXME Comments Requiring Attention
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Fixme
**File**: src/context/debt.rs
**Priority**: High

#### Current State:
- Line 510: "This is a hack"
- Line 515: "This needs review"
- Multiple other FIXME markers

#### Required Changes:
- Address each FIXME with proper implementation
- Remove hack and implement proper solution
- Complete code review and resolve issues

#### Implementation Steps:
- Review hack at line 510 and implement proper solution
- Conduct thorough review of code at line 515
- Document decisions made for each FIXME resolution
- Add tests to prevent regression

### 6. God Component - Cook Module
**Impact Score**: 7/10
**Effort Score**: 6/10
**Category**: Architecture
**File**: src/cook/mod.rs
**Priority**: Medium

#### Current State:
Cook module has 13 dependencies, indicating it's doing too much.

#### Required Changes:
- Split responsibilities into focused modules
- Apply single responsibility principle
- Reduce coupling between components

#### Implementation Steps:
- Extract git operations to dedicated `git_integration` module
- Move workflow logic to `workflow_engine` module
- Create `iteration_manager` for iteration handling
- Implement dependency injection for better testability
- Update imports and module structure

## Dependency Cleanup

### Dependencies to Review:
- `atty` - Check if still needed with modern terminal detection
- `md5` - Consider using sha2 which is already included
- Review if all tokio features are actually used

## Code Organization Changes

### Modules to Restructure:
- Split cook module into: `workflow_engine`, `git_integration`, `iteration_manager`
- Create proper public API for abstractions module
- Organize test utilities into dedicated test module

## Success Criteria
- [ ] All functions with complexity > 20 refactored to < 15
- [ ] All deprecated markers addressed or properly documented
- [ ] All FIXME comments resolved
- [ ] Cook module dependencies reduced from 13 to < 8
- [ ] All files compile without warnings
- [ ] Tests pass with same or improved coverage
- [ ] Performance benchmarks maintained or improved
- [ ] Clippy lints resolved or explicitly allowed with justification

## Validation Commands
```bash
cargo check --all-targets
cargo test --all-features
cargo clippy -- -W clippy::all
cargo fmt --check
```