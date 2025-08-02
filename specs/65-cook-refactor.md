# Cook Module Refactoring Plan

## Problem Statement
The cook module currently acts as a god component with 13 direct dependencies, violating the Single Responsibility Principle and making it difficult to test and maintain.

## Proposed Solution: Facade Pattern with Specialized Coordinators

### 1. Create Specialized Coordinators

#### EnvironmentCoordinator
Manages all environment setup concerns:
```rust
pub struct EnvironmentCoordinator {
    config_loader: Arc<dyn ConfigLoader>,
    worktree_manager: Arc<dyn WorktreeManager>, 
    git_operations: Arc<dyn GitOperations>,
}
```
Responsibilities:
- Load and validate configuration
- Setup worktree if needed
- Verify git repository state
- Prepare execution environment

#### SessionCoordinator  
Manages session lifecycle:
```rust
pub struct SessionCoordinator {
    session_manager: Arc<dyn SessionManager>,
    state_manager: Arc<dyn StateManager>,
}
```
Responsibilities:
- Start/stop sessions
- Track session state
- Handle session persistence
- Resume interrupted sessions

#### ExecutionCoordinator
Manages command execution:
```rust
pub struct ExecutionCoordinator {
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    subprocess_manager: Arc<dyn SubprocessManager>,
}
```
Responsibilities:
- Execute system commands
- Execute Claude commands
- Manage subprocess lifecycle
- Handle execution retries

#### AnalysisCoordinator (already exists)
Keep current implementation for:
- Project analysis
- Metrics collection
- Report generation

#### WorkflowCoordinator
High-level workflow orchestration:
```rust
pub struct WorkflowCoordinator {
    workflow_executor: Arc<dyn WorkflowExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
}
```
Responsibilities:
- Execute workflow steps
- Handle user interactions
- Manage workflow state

### 2. Refactor CookOrchestrator

Reduce dependencies from 13 to 5:
```rust
pub struct DefaultCookOrchestrator {
    environment_coordinator: Arc<dyn EnvironmentCoordinator>,
    session_coordinator: Arc<dyn SessionCoordinator>,
    execution_coordinator: Arc<dyn ExecutionCoordinator>,
    analysis_coordinator: Arc<dyn AnalysisCoordinator>,
    workflow_coordinator: Arc<dyn WorkflowCoordinator>,
}
```

### 3. Implementation Steps

#### Phase 1: Create Coordinator Interfaces (Low Risk)
1. Define trait interfaces for each coordinator
2. Create default implementations wrapping existing functionality
3. Add unit tests for each coordinator

#### Phase 2: Refactor Factory Functions (Medium Risk)
1. Update `create_orchestrator()` to create coordinators
2. Inject coordinators instead of individual dependencies
3. Update existing tests to use new structure

#### Phase 3: Refactor Orchestrator Implementation (Medium Risk)
1. Update `DefaultCookOrchestrator` to use coordinators
2. Delegate responsibilities to appropriate coordinators
3. Ensure backward compatibility

#### Phase 4: Cleanup and Optimization (Low Risk)
1. Remove redundant code
2. Optimize coordinator interactions
3. Update documentation

## Benefits

1. **Reduced Complexity**: Each coordinator has 2-3 dependencies instead of 13
2. **Better Testability**: Can mock individual coordinators
3. **Clear Responsibilities**: Each coordinator has a single, well-defined purpose
4. **Easier Extension**: New features can be added to specific coordinators
5. **Improved Maintainability**: Changes isolated to relevant coordinators

## Migration Strategy

1. **Backward Compatibility**: Keep existing public APIs unchanged
2. **Incremental Migration**: Implement one coordinator at a time
3. **Feature Flags**: Use feature flags to switch between old/new implementations
4. **Comprehensive Testing**: Add integration tests before and after each phase

## Risk Mitigation

1. **Extensive Testing**: Unit tests for each coordinator
2. **Integration Tests**: End-to-end tests for cook workflows
3. **Gradual Rollout**: Deploy behind feature flags
4. **Monitoring**: Track performance metrics before/after
5. **Rollback Plan**: Keep old implementation available

## Timeline Estimate

- Phase 1: 2-3 days (create interfaces and implementations)
- Phase 2: 1-2 days (update factory functions)
- Phase 3: 2-3 days (refactor orchestrator)
- Phase 4: 1 day (cleanup)

Total: ~1.5-2 weeks for complete refactoring

## Success Metrics

1. Reduced coupling (from 13 to 5 dependencies)
2. Improved test coverage (target: 80%+)
3. No performance regression
4. All existing functionality preserved
5. Easier to add new features