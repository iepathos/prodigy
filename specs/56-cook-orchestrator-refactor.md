# Specification 56: Cook Orchestrator Refactor

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The cook module has grown into a "god component" with nearly 2000 lines of code that handles too many responsibilities. This makes it difficult to test (only 22% coverage) and maintain. The module currently handles:

- Command orchestration
- Git operations
- Workflow execution
- Metrics collection
- Session management
- Analysis coordination
- User interaction
- Error handling and retries
- Subprocess management
- State persistence

This specification outlines a refactoring to break down the cook module into focused, testable components using dependency injection and clear separation of concerns.

## Objective

Refactor the cook module into smaller, focused components that are independently testable, achieving at least 80% unit test coverage while maintaining all existing functionality.

## Requirements

### Functional Requirements
- Maintain all existing cook functionality without breaking changes
- Support the same CLI interface and behavior
- Preserve git-native workflow and worktree support
- Keep metrics collection and analysis features intact

### Non-Functional Requirements
- Achieve minimum 80% unit test coverage for refactored components
- Enable dependency injection for all external dependencies
- Support mocking of all subprocess operations
- Reduce maximum module size to under 300 lines
- Improve code maintainability and readability

## Acceptance Criteria

- [ ] Cook module split into at least 5 focused components
- [ ] Each component has >80% unit test coverage
- [ ] All existing integration tests still pass
- [ ] No external API changes (CLI remains the same)
- [ ] All subprocess operations mockable for testing
- [ ] Dependency injection pattern implemented throughout
- [ ] Maximum cyclomatic complexity per function reduced by 50%

## Technical Details

### Implementation Approach

1. **Extract Orchestrator Pattern**
   - Create `CookOrchestrator` trait that defines the high-level workflow
   - Implement concrete orchestrator that delegates to focused components
   - Enable testing with mock orchestrator

2. **Component Breakdown**
   ```
   cook/
   ├── orchestrator.rs      # Main orchestration logic
   ├── session/
   │   ├── mod.rs          # Session management
   │   ├── state.rs        # Session state handling
   │   └── tracker.rs      # Progress tracking
   ├── execution/
   │   ├── mod.rs          # Command execution
   │   ├── runner.rs       # Subprocess runner trait
   │   └── claude.rs       # Claude-specific execution
   ├── analysis/
   │   ├── mod.rs          # Analysis coordination
   │   ├── runner.rs       # Analysis execution
   │   └── cache.rs        # Analysis caching
   ├── metrics/
   │   ├── mod.rs          # Metrics coordination
   │   ├── collector.rs    # Metrics collection
   │   └── reporter.rs     # Metrics reporting
   └── interaction/
       ├── mod.rs          # User interaction
       ├── prompts.rs      # User prompts
       └── display.rs      # Progress display
   ```

3. **Dependency Injection**
   - Define traits for all external dependencies
   - Create factory pattern for component creation
   - Support both production and test configurations

### Architecture Changes

1. **New Trait Definitions**
   ```rust
   // Core orchestrator trait
   pub trait CookOrchestrator {
       async fn run(&self, config: CookConfig) -> Result<()>;
   }

   // Subprocess execution trait
   pub trait CommandRunner {
       async fn run_command(&self, cmd: &str, args: &[String]) -> Result<Output>;
   }

   // Git operations trait (already exists)
   pub trait GitOperations {
       // Existing methods
   }

   // User interaction trait
   pub trait UserInteraction {
       async fn prompt_yes_no(&self, message: &str) -> Result<bool>;
       fn display_progress(&self, message: &str);
   }

   // Analysis runner trait
   pub trait AnalysisRunner {
       async fn run_analysis(&self, path: &Path, with_coverage: bool) -> Result<AnalysisResult>;
   }
   ```

2. **Orchestrator Implementation**
   ```rust
   pub struct DefaultOrchestrator {
       session_manager: Box<dyn SessionManager>,
       executor: Box<dyn CommandExecutor>,
       analyzer: Box<dyn AnalysisRunner>,
       metrics: Box<dyn MetricsCollector>,
       interaction: Box<dyn UserInteraction>,
   }
   ```

### Data Structures

1. **Simplified Cook Configuration**
   ```rust
   pub struct CookConfig {
       pub command: CookCommand,
       pub project_path: PathBuf,
       pub workflow: WorkflowConfig,
   }
   ```

2. **Execution Context**
   ```rust
   pub struct ExecutionContext {
       pub session: SessionState,
       pub git_ops: Box<dyn GitOperations>,
       pub command_runner: Box<dyn CommandRunner>,
   }
   ```

### APIs and Interfaces

The public API remains unchanged - only internal structure changes.

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - cook module and all submodules
  - Integration tests that depend on cook internals
- **External Dependencies**: No new dependencies

## Testing Strategy

- **Unit Tests**: 
  - Mock all traits for isolated testing
  - Test each component independently
  - Use property-based testing for complex logic
- **Integration Tests**: 
  - Use existing integration test suite
  - Add tests for component integration
- **Performance Tests**: 
  - Ensure no performance regression
  - Measure overhead of trait objects
- **User Acceptance**: 
  - All existing commands work identically
  - No visible behavior changes

## Documentation Requirements

- **Code Documentation**: 
  - Document all new traits and their contracts
  - Add examples for using mock implementations
- **Architecture Updates**: 
  - Update ARCHITECTURE.md with new component structure
  - Document dependency injection patterns
- **Testing Guide**: 
  - Document how to test with mocks
  - Provide examples of unit tests

## Implementation Notes

1. **Incremental Refactoring**
   - Start with extracting session management
   - Then extract execution logic
   - Finally extract user interaction
   - Keep cook/mod.rs as thin orchestration layer

2. **Backward Compatibility**
   - Use facade pattern to maintain existing API
   - Gradually migrate internal calls
   - No breaking changes to public interface

3. **Testing First**
   - Write tests for new components before implementation
   - Use TDD to ensure testability
   - Mock external dependencies from the start

## Migration and Compatibility

- No breaking changes to external API
- Internal module structure changes only
- Gradual migration path for dependent code
- All existing tests must continue to pass