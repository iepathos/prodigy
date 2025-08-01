# Specification 56a: Fix Orchestrator Compilation Issues

**Category**: optimization
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 56 (Cook Orchestrator Refactor)

## Context

Specification 56 successfully refactored the cook module from a 1929-line god component into focused, testable components with proper separation of concerns. However, the refactoring introduced compilation issues due to API mismatches with existing dependencies. These issues prevent the refactored code from being integrated into the main codebase.

The main issues identified include:
- API changes in `ConfigLoader`, `ContextAnalyzer`, and `ProjectAnalyzer`
- Missing `await` keywords for async functions
- Type mismatches between Arc<T> and trait bounds expecting T
- Missing imports and incorrect module paths
- Changed struct fields in `CookCommand`
- Interior mutability requirements for shared state

## Objective

Fix all compilation issues in the refactored cook orchestrator code while maintaining the architectural improvements achieved in Spec 56. Ensure the refactored code compiles, passes all tests, and integrates seamlessly with the existing codebase.

## Requirements

### Functional Requirements
- Fix all compilation errors in the refactored cook module
- Maintain the component-based architecture from Spec 56
- Preserve all existing cook functionality
- Ensure backward compatibility with existing API consumers
- Keep the dependency injection pattern intact

### Non-Functional Requirements
- No regression in functionality
- Maintain testability improvements from Spec 56
- Keep compilation time reasonable
- Preserve the modular structure

## Acceptance Criteria

- [ ] All compilation errors in cook module are resolved
- [ ] All unit tests in refactored components pass
- [ ] All integration tests continue to pass
- [ ] Cook command works identically to pre-refactor behavior
- [ ] No new compilation warnings introduced
- [ ] Refactored code maintains >80% test coverage potential
- [ ] CI/CD pipeline passes all checks

## Technical Details

### Implementation Approach

1. **API Alignment**
   - Update `ConfigLoader` usage to match current API
   - Fix `ContextAnalyzer` and `ProjectAnalyzer` async/await patterns
   - Align `CookCommand` struct fields with current definition
   - Fix trait implementations and module imports

2. **Type System Fixes**
   - Resolve Arc<T> vs T trait bound issues
   - Fix lifetime and ownership problems
   - Address interior mutability requirements
   - Correct async function signatures

3. **Module Structure Fixes**
   - Fix import paths for moved modules
   - Export necessary types from submodules
   - Resolve circular dependency issues
   - Ensure proper visibility modifiers

### Specific Fixes Required

1. **ConfigLoader API**
   ```rust
   // Current (broken)
   let config_loader = ConfigLoader::new().await?;
   let config = config_loader.load().await?;
   
   // Fixed
   let config_loader = ConfigLoader::new()?;
   let config = config_loader.load_config().await?;
   ```

2. **Arc<CommandRunner> Issue**
   ```rust
   // Option 1: Implement CommandRunner for Arc<T>
   impl<T: CommandRunner> CommandRunner for Arc<T> { ... }
   
   // Option 2: Use raw types and wrap later
   let runner = RealCommandRunner::new();
   let executor = ClaudeExecutorImpl::new(runner);
   let arc_executor = Arc::new(executor);
   ```

3. **SessionManager Mutability**
   - Change trait methods from `&mut self` to `&self`
   - Use interior mutability (Mutex/RwLock) in implementations
   - Update all call sites to remove mutable borrows

4. **Missing Async/Await**
   - Add `.await` to `ProjectAnalyzer::analyze()`
   - Fix async match expressions
   - Correct Future type mismatches

### APIs and Interfaces

No new APIs - only fixing existing ones to match current codebase expectations.

## Dependencies

- **Prerequisites**: Spec 56 must be implemented (completed)
- **Affected Components**: 
  - All cook submodules
  - Tests that depend on cook module
  - CLI main entry point
- **External Dependencies**: No new dependencies

## Testing Strategy

- **Compilation Tests**: Ensure clean compilation with no warnings
- **Unit Tests**: Run all existing unit tests for refactored components
- **Integration Tests**: Verify cook command behavior matches pre-refactor
- **Regression Tests**: Ensure no functionality is lost
- **Coverage Analysis**: Verify test coverage remains high

## Documentation Requirements

- **Code Documentation**: Update any outdated documentation in refactored code
- **Migration Guide**: Document any API changes if absolutely necessary
- **Architecture Updates**: None needed - architecture already documented in Spec 56

## Implementation Notes

1. **Incremental Fixes**
   - Fix compilation errors module by module
   - Run tests after each module is fixed
   - Commit working states frequently

2. **Preserving Architecture**
   - Do not compromise the modular design
   - Keep dependency injection intact
   - Maintain separation of concerns

3. **Backward Compatibility**
   - Ensure no breaking changes to public APIs
   - Keep CLI interface identical
   - Preserve git commit formats

## Migration and Compatibility

- No migration needed - fixing compilation only
- Full backward compatibility required
- No breaking changes to external interfaces