# Specification 57a: Subprocess Migration

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [57-subprocess-abstraction-layer]

## Context

Specification 57 successfully implemented a comprehensive subprocess abstraction layer, but the migration was only partially completed. Many parts of the codebase still use direct `tokio::process::Command` calls, which prevents us from fully leveraging the benefits of the abstraction layer:

- Unit testing is still difficult for modules using direct subprocess calls
- Error handling remains inconsistent across different subprocess operations
- No centralized logging or debugging for subprocess operations
- Cannot add features like timeout, retry, or output filtering uniformly
- Mock testing requires complex workarounds or environment variables

Current direct subprocess usage is found in:
- Cook module execution paths
- Worktree management operations
- Context analysis tools
- Metrics collection commands
- Init command template installations
- Various test utilities

## Objective

Complete the migration of all direct subprocess calls to use the new subprocess abstraction layer, enabling comprehensive unit testing and consistent subprocess management throughout the codebase.

## Requirements

### Functional Requirements
- Replace all remaining `tokio::process::Command` usage with subprocess abstraction
- Maintain backward compatibility with existing functionality
- Preserve all current error messages and behaviors
- Support all existing subprocess features (pipes, environment, working directory)
- Enable mock testing for all subprocess-dependent modules

### Non-Functional Requirements
- Zero behavioral changes for end users
- No performance degradation
- Maintain thread safety where required
- Keep changes isolated to subprocess calls only
- Preserve existing module boundaries

## Acceptance Criteria

- [ ] All direct `Command` usage replaced in cook module
- [ ] All direct `Command` usage replaced in worktree module
- [ ] All direct `Command` usage replaced in context module
- [ ] All direct `Command` usage replaced in metrics module
- [ ] All direct `Command` usage replaced in init module
- [ ] All direct `Command` usage replaced in test utilities
- [ ] Existing tests continue to pass without modification
- [ ] New unit tests added for previously untestable code
- [ ] No compilation warnings related to unused imports
- [ ] Documentation updated for subprocess usage patterns

## Technical Details

### Implementation Approach

1. **Module-by-Module Migration**
   - Start with leaf modules that don't have dependencies
   - Progress to core modules like cook and worktree
   - Update tests last to ensure no regressions

2. **Dependency Injection Pattern**
   ```rust
   // Before: Direct command usage
   pub async fn run_command() -> Result<String> {
       let output = Command::new("git").args(&["status"]).output().await?;
       // ...
   }
   
   // After: Using subprocess abstraction
   pub struct MyModule {
       subprocess: SubprocessManager,
   }
   
   impl MyModule {
       pub async fn run_command(&self) -> Result<String> {
           let output = self.subprocess.git().status(Path::new(".")).await?;
           // ...
       }
   }
   ```

3. **Test Infrastructure Updates**
   ```rust
   #[cfg(test)]
   mod tests {
       #[tokio::test]
       async fn test_my_function() {
           let (subprocess, mut mock) = SubprocessManager::mock();
           mock.expect_command("git")
               .with_args(|args| args == &["status"])
               .returns_success()
               .finish();
           
           let module = MyModule { subprocess };
           let result = module.run_command().await.unwrap();
           // assertions...
       }
   }
   ```

### Architecture Changes

1. **Cook Module**
   - Update `RealCommandRunner` to use `SubprocessManager` (already done)
   - Update workflow executors to accept subprocess manager
   - Update git operations to use git runner
   - Remove direct command calls from retry logic

2. **Worktree Module**
   - Create worktree operations using git runner
   - Update manager to accept subprocess manager
   - Remove all direct git command calls

3. **Context Module**
   - Update analyzers to accept subprocess manager
   - Use specialized runners for language-specific tools
   - Remove direct cargo/npm/etc. command calls

4. **Metrics Module**
   - Update collectors to use subprocess abstraction
   - Create specialized runners for metric tools if needed
   - Remove direct command calls for coverage tools

5. **Init Module**
   - Update template installation to use subprocess
   - Remove direct git command calls

### Data Structures

No new data structures required. Use existing:
- `SubprocessManager` for dependency injection
- `ProcessCommandBuilder` for command construction
- `MockProcessRunner` for testing

## Dependencies

- **Prerequisites**: Spec 57 (Subprocess Abstraction Layer) must be completed
- **Affected Components**: 
  - Cook module (all submodules)
  - Worktree module
  - Context module analyzers
  - Metrics collectors
  - Init command
  - Test utilities
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Add comprehensive unit tests for all migrated code
  - Use mock subprocess runner for deterministic behavior
  - Test error scenarios and edge cases
  - Verify timeout and retry logic
- **Integration Tests**: 
  - Ensure existing integration tests continue to pass
  - Add new integration tests for complex workflows
  - Test real subprocess execution paths
- **Performance Tests**: 
  - Verify no performance regression
  - Measure subprocess call overhead
- **User Acceptance**: 
  - All existing commands work identically
  - Error messages remain unchanged
  - No visible behavior changes

## Documentation Requirements

- **Code Documentation**: 
  - Document subprocess manager usage patterns
  - Add examples for common migration scenarios
  - Document testing patterns with mocks
- **Migration Guide**: 
  - Step-by-step guide for migrating modules
  - Common pitfalls and solutions
  - Testing strategies for migrated code
- **Architecture Updates**: 
  - Update ARCHITECTURE.md with subprocess usage
  - Document dependency injection patterns
  - Add subprocess abstraction to module descriptions

## Implementation Notes

1. **Phased Migration Order**
   ```
   Phase 1: Simple modules (init, test utilities)
   Phase 2: Context analyzers
   Phase 3: Metrics collectors  
   Phase 4: Worktree management
   Phase 5: Cook module (most complex)
   Phase 6: Test infrastructure
   ```

2. **Common Patterns**
   - Git operations: Use `subprocess.git()` runner
   - Claude operations: Use `subprocess.claude()` runner
   - Generic commands: Use `subprocess.runner()` with `ProcessCommandBuilder`
   - File operations that shell out: Consider if subprocess is needed

3. **Error Handling**
   - Preserve existing error messages
   - Map `ProcessError` to module-specific errors
   - Maintain error context and chaining

4. **Testing Considerations**
   - Create test fixtures for common mock scenarios
   - Use builder pattern for complex mock setups
   - Test both success and failure paths

## Migration and Compatibility

1. **Backward Compatibility**
   - No changes to public APIs
   - No changes to CLI behavior
   - No changes to configuration formats

2. **Breaking Changes**
   - None for end users
   - Internal module constructors may change
   - Test infrastructure will require updates

3. **Migration Path**
   - Each module can be migrated independently
   - No flag day required
   - Gradual rollout possible

4. **Rollback Strategy**
   - Each commit should be independently revertable
   - Keep migrations small and focused
   - Extensive testing before each phase