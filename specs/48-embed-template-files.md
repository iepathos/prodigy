---
number: 48
title: Embed Template Files in Binary
category: compatibility
priority: high
status: draft
dependencies: []
created: 2025-08-04
---

# Specification 48: Embed Template Files in Binary

**Category**: compatibility
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The MMM init command currently uses `include_str!` macro to embed command template markdown files at compile time. While this approach already embeds the files into the binary, there's a concern about ensuring this works correctly when the MMM binary is distributed to client servers without the source code repository. The current implementation in `src/init/templates.rs` uses relative paths like `include_str!("../../.claude/commands/mmm-code-review.md")` which are resolved at compile time, but we need to ensure this pattern is robust and properly handles all edge cases for binary distribution.

Currently, the init system installs these embedded templates to the user's `.claude/commands/` directory when they run `mmm init`. This allows users to have a standalone MMM binary that can bootstrap Claude Code integration without needing access to the original source repository.

## Objective

Ensure that all template files required by the `mmm init` command are properly embedded in the binary at compile time, allowing the MMM tool to function correctly when distributed as a standalone binary without requiring access to the source code repository or external template files.

## Requirements

### Functional Requirements
- All command template markdown files must be embedded in the binary at compile time
- The embedded templates must be accessible when running `mmm init` on any system
- No external file dependencies should be required for the init command to function
- The binary size increase from embedded templates should be reasonable (templates are text files)
- Support adding new templates without changing the embedding pattern

### Non-Functional Requirements
- Zero runtime file I/O for accessing template content
- Compile-time validation that all referenced template files exist
- Clear error messages if template embedding fails during compilation
- Maintain the current user experience with no changes to the init command interface

## Acceptance Criteria

- [ ] All template files in `.claude/commands/` are embedded using `include_str!` macro
- [ ] The `mmm init` command works correctly when the binary is run from any location
- [ ] The binary can be distributed standalone without the source repository
- [ ] No runtime errors occur when accessing embedded templates
- [ ] Adding a new template follows the same simple pattern
- [ ] Binary size increase is documented and reasonable (< 1MB for all templates)
- [ ] Tests verify that templates are properly embedded and accessible

## Technical Details

### Implementation Approach

The current implementation already uses the correct approach with `include_str!` macro. This specification documents and validates this approach:

1. **Current Embedding Pattern** (Already Implemented)
   ```rust
   pub const MMM_CODE_REVIEW: &str = include_str!("../../.claude/commands/mmm-code-review.md");
   ```
   This pattern is correct and ensures compile-time embedding.

2. **Verify Build Process**
   - Ensure `cargo build --release` includes all templates
   - Verify that the relative paths are resolved during compilation
   - Confirm no runtime file access is attempted

3. **Template Registry Pattern**
   - Maintain the current `get_all_templates()` function
   - Each template is a compile-time constant
   - No dynamic file loading at runtime

### Data Structures

The current structure is already correct:
```rust
pub struct CommandTemplate {
    pub name: &'static str,
    pub content: &'static str,  // Points to embedded string data
    pub description: &'static str,
}
```

### APIs and Interfaces

No API changes required. The current interface remains:
- `get_all_templates()` - Returns all embedded templates
- `get_templates_by_names()` - Filters templates by name
- `install_command()` - Writes embedded content to user's filesystem

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/init/templates.rs` - Template embedding (already correct)
  - `.claude/commands/*.md` - Source template files
  - Build process - Ensures templates are found during compilation
- **External Dependencies**: None (templates are embedded at compile time)

## Testing Strategy

- **Unit Tests**: 
  - Verify all templates have non-empty content
  - Test that `include_str!` constants are properly initialized
  - Ensure template count matches expected number
- **Integration Tests**: 
  - Run `mmm init` from different working directories
  - Test with binary moved to different locations
  - Verify templates install correctly without source repo
- **Distribution Tests**: 
  - Build release binary and test on clean system
  - Verify no file-not-found errors occur
  - Test binary portability across different environments
- **Build Tests**:
  - Ensure build fails if template files are missing
  - Verify compile-time errors for invalid paths

## Documentation Requirements

- **Code Documentation**: 
  - Document that templates are embedded at compile time
  - Explain the `include_str!` pattern for future templates
  - Note that relative paths are resolved during compilation
- **User Documentation**: 
  - Update README to clarify that MMM can be distributed as a standalone binary
  - Note that no external files are required for init command
- **Distribution Guide**:
  - Document how to build and distribute the MMM binary
  - Explain that templates are self-contained in the binary

## Implementation Notes

- The current implementation using `include_str!` is already correct
- This specification primarily documents and validates the existing approach
- Key insight: `include_str!` resolves paths at compile time, not runtime
- The macro reads file contents during compilation and embeds them as static strings
- This approach has been working correctly; this spec ensures it's properly understood
- Consider adding a compile-time test to ensure all templates are non-empty

## Migration and Compatibility

- No migration required - the current implementation is already correct
- The pattern has been in use and working properly
- This specification documents the approach for clarity and future reference
- Future templates should follow the same `include_str!` pattern
- Binary distribution has been implicitly supported since the feature was added