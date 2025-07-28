# Specification 33: Batch Spec Implementation

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [14, 19, 21, 28]

## Context

Currently, `mmm improve` follows a cycle of code-review → implement-spec → lint. However, when developers have already written specifications in the `specs/` directory, they need a way to implement multiple specs without the code-review step. This would enable batch implementation of planned features and improvements that have already been thought through and documented.

This specification introduces a new subcommand that allows implementing one or more existing specifications through an automated development loop, streamlining the implementation of pre-planned improvements.

## Objective

Create a new `mmm implement` subcommand that accepts a list of specification files and implements them sequentially using a simplified loop of implement-spec → lint for each specification.

## Requirements

### Functional Requirements
- New subcommand: `mmm implement <spec-files...>`
- Accept one or more spec file paths as arguments
- Support glob patterns (e.g., `specs/pending/*.md`)
- Execute implement-spec → lint cycle for each spec
- Continue to next spec even if one fails (with clear error reporting)
- Support --worktree flag for parallel execution
- Track progress across multiple specs
- Generate summary report of implemented specs

### Non-Functional Requirements
- Reuse existing Claude integration infrastructure
- Maintain git-native audit trail
- Support all existing CLI flags (--verbose, --max-iterations)
- Integrate with existing state management

## Acceptance Criteria

- [ ] `mmm implement specs/31-*.md` implements a single spec
- [ ] `mmm implement specs/*.md` implements multiple specs sequentially
- [ ] Each spec goes through implement → lint cycle
- [ ] Failed specs don't stop processing of remaining specs
- [ ] Clear progress indication for batch operations
- [ ] Summary report shows success/failure for each spec
- [ ] --worktree flag enables parallel execution
- [ ] --dry-run flag shows what would be implemented
- [ ] Git commits maintain clear audit trail per spec

## Technical Details

### Implementation Approach
1. Parse command-line arguments for spec file paths
2. Resolve glob patterns to actual file list
3. Validate spec files exist and are readable
4. For each spec:
   - Extract spec ID from filename or content
   - Call `/mmm-implement-spec` with the spec ID
   - Call `/mmm-lint` to clean up
   - Track success/failure
5. Generate summary report

### Architecture Changes
- Add new `implement` subcommand module
- Extend CLI parser for spec file arguments
- Add batch processing logic to improve module

### Data Structures
```rust
struct BatchImplementState {
    specs: Vec<PathBuf>,
    completed: Vec<(String, bool)>, // (spec_id, success)
    current_spec: Option<String>,
    start_time: Instant,
}

struct ImplementCommand {
    spec_files: Vec<String>,
    worktree: bool,
    verbose: bool,
    dry_run: bool,
    max_iterations: Option<usize>,
}
```

### APIs and Interfaces
```bash
# Single spec
mmm implement specs/31-product-management-command.md

# Multiple specs
mmm implement specs/30-*.md specs/31-*.md

# All pending specs
mmm implement specs/pending/*.md

# With options
mmm implement --worktree --verbose specs/*.md

# Dry run
mmm implement --dry-run specs/*.md
```

## Dependencies

- **Prerequisites**: 
  - Spec 14: Real Claude Loop
  - Spec 19: Git-Native Flow
  - Spec 21: Configurable Workflow
  - Spec 28: Structured Commands
- **Affected Components**: 
  - CLI parser (new subcommand)
  - Improve module (shared logic)
  - State management (batch tracking)
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Spec file path resolution
  - Glob pattern expansion
  - Batch state tracking
- **Integration Tests**: 
  - Single spec implementation
  - Multiple spec batch processing
  - Error handling for invalid specs
  - Worktree integration
- **Performance Tests**: 
  - Large batch performance
  - Memory usage with many specs
- **User Acceptance**: 
  - Clear progress feedback
  - Useful error messages
  - Intuitive CLI interface

## Documentation Requirements

- **Code Documentation**: 
  - New module documentation
  - CLI help text
- **User Documentation**: 
  - Add to README.md
  - Include in command reference
  - Workflow examples for batch implementation
- **Architecture Updates**: 
  - Add implement subcommand to ARCHITECTURE.md

## Implementation Notes

### Error Handling
- Invalid spec files should be reported but not stop batch
- Failed implementations should be tracked and reported
- Provide option to stop on first failure (--fail-fast)

### Progress Reporting
```
Implementing 5 specifications...

[1/5] ✓ specs/30-interrupted-worktree-recovery.md (2m 15s)
[2/5] ✗ specs/31-product-management-command.md (failed: compilation error)
[3/5] ⚡ specs/32-cli-help-default.md (in progress...)
[4/5] ⏸ specs/33-batch-implementation.md (pending)
[5/5] ⏸ specs/34-example.md (pending)

Summary: 1 succeeded, 1 failed, 3 remaining
```

### Workflow Integration
This command could also be used in custom workflows:
```yaml
commands:
  - name: mmm-spec-scan
    focus: pending-features
  - implement-specs: specs/temp/*.md
  - mmm-test-all
```

## Migration and Compatibility

No migration required. This is a new additive feature that doesn't affect existing commands. The `mmm improve` command continues to work as before, while `mmm implement` provides a focused tool for pre-written specifications.