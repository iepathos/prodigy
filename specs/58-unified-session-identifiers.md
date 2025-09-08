---
number: 58
title: Unified Session Identifiers
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-08
---

# Specification 58: Unified Session Identifiers

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently uses two different identifier formats for the same conceptual session, creating confusion and inconsistency:

1. **Worktree directories**: Named using `session-{uuid}` format (e.g., `session-9e18ed72-befe-45c0-8d0a-07b73c0ea3c9`)
2. **Session tracking**: Uses `cook-{timestamp}` format (e.g., `cook-1757314884`)

This dual-ID system causes several problems:
- Users cannot intuitively resume sessions using the visible worktree directory name
- The `--resume` flag requires the internal `cook-{timestamp}` ID which isn't obvious from the worktree path
- Session state files reference different IDs than their containing directories
- Debugging and correlating sessions across logs and filesystem is unnecessarily complex
- The timestamp-based IDs can theoretically collide in rapid parallel executions

The codebase already has a `SessionId` type that generates UUID-based identifiers in `src/session/mod.rs`, but the cook module bypasses this to generate its own timestamp-based IDs. This specification defines the changes needed to unify these systems into a single, consistent identifier format throughout the application.

## Objective

Unify all session identification to use a single, consistent UUID-based format (`session-{uuid}`) throughout the codebase, enabling intuitive session management where the worktree directory name directly corresponds to the resumable session ID.

## Requirements

### Functional Requirements

#### Session ID Generation
- All session IDs MUST use the existing `SessionId::new()` method that generates `session-{uuid}` format
- Remove all timestamp-based `cook-{timestamp}` ID generation
- Ensure session IDs are globally unique using UUID v4
- Maintain backward compatibility for reading existing session states

#### Worktree Integration
- Worktree directory names MUST match the session ID exactly
- Session state files MUST reference the same ID as their containing directory
- The `--resume` flag MUST accept the worktree directory name as the session ID

#### Session Management
- Session tracking MUST use the unified ID throughout all modules
- MapReduce job IDs MUST derive from the parent session ID
- Event logging MUST reference the unified session ID
- Session state persistence MUST use the unified ID

### Non-Functional Requirements

#### Backward Compatibility
- MUST be able to read and migrate existing `cook-{timestamp}` session states
- MUST handle existing worktrees without breaking active sessions
- SHOULD provide clear migration messages for old session formats

#### Performance
- UUID generation MUST not impact session startup time significantly
- Session ID lookups MUST remain O(1) operations
- No additional filesystem operations for ID correlation

#### Usability
- Session IDs MUST be copyable from directory listings
- Resume operations MUST work with partial ID matches (first 8 chars minimum)
- Error messages MUST clearly indicate which session ID to use

## Acceptance Criteria

- [ ] All new sessions use `session-{uuid}` format consistently
- [ ] Worktree directory name matches the session ID exactly
- [ ] `prodigy cook --resume session-{uuid}` works directly with worktree names
- [ ] Session state files contain the same ID as their directory
- [ ] MapReduce agents inherit consistent ID patterns from parent sessions
- [ ] Existing `cook-{timestamp}` sessions can still be read (backward compatibility)
- [ ] No timestamp-based IDs remain in the codebase for new sessions
- [ ] All tests pass with the unified ID system
- [ ] Documentation updated to reflect single ID format

## Technical Details

### Implementation Approach

1. **Centralize ID Generation**
   - Update `src/cook/mod.rs:143` to use `SessionId::new().to_string()` instead of `format!("cook-{}", timestamp)`
   - Update `src/cook/orchestrator.rs:151` similarly
   - Remove the `generate_session_id()` method from orchestrator

2. **Update Session Manager**
   - Modify `SessionTrackerImpl::new()` to accept the unified session ID
   - Ensure session state persistence uses the new format
   - Add migration logic for reading old `cook-{timestamp}` formats

3. **Fix Resume Logic**
   - Update resume command parsing to handle `session-{uuid}` format
   - Add partial ID matching for convenience (minimum 8 characters)
   - Improve error messages when session not found

4. **Update Tests**
   - Fix test fixtures that expect `cook-{timestamp}` format
   - Add tests for backward compatibility
   - Add tests for partial ID matching

### Architecture Changes

```rust
// Before (in src/cook/mod.rs)
let session_manager = Arc::new(session::tracker::SessionTrackerImpl::new(
    format!("cook-{}", chrono::Utc::now().timestamp()),
    project_path.to_path_buf(),
));

// After
let session_id = session::SessionId::new();
let session_manager = Arc::new(session::tracker::SessionTrackerImpl::new(
    session_id.to_string(),
    project_path.to_path_buf(),
));
```

### Data Structures

No new data structures required. The existing `SessionId` type in `src/session/mod.rs` will be used consistently:

```rust
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        Self(format!("session-{}", Uuid::new_v4()))
    }
}
```

### APIs and Interfaces

- `--resume` flag will accept `session-{uuid}` format
- Session state JSON will use `session_id` field with new format
- Worktree manager will use the session ID directly for directory naming

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/mod.rs` - Session initialization
  - `src/cook/orchestrator.rs` - Session ID generation
  - `src/session/tracker.rs` - Session state management
  - `src/worktree/manager.rs` - Worktree directory naming
  - `src/cook/resume.rs` - Resume functionality
- **External Dependencies**: `uuid` crate (already in use)

## Testing Strategy

### Unit Tests
- Test unified ID generation and format
- Test backward compatibility with old session formats
- Test partial ID matching for resume operations
- Test session state serialization with new IDs

### Integration Tests
- Test full cook workflow with unified IDs
- Test resume operations using worktree directory names
- Test MapReduce job ID derivation
- Test migration of existing sessions

### User Acceptance
- Verify users can resume sessions using visible directory names
- Verify clear error messages for missing sessions
- Verify backward compatibility doesn't break existing workflows

## Documentation Requirements

### Code Documentation
- Update inline documentation for session ID generation
- Document the unified ID format in module docs
- Add migration notes for backward compatibility

### User Documentation
- Update README with new resume syntax
- Update CLAUDE.md with session ID explanation
- Add migration guide for existing sessions

### Architecture Updates
- Update ARCHITECTURE.md to reflect unified ID system
- Document the session ID lifecycle
- Explain the worktree-session relationship

## Implementation Notes

### Migration Strategy
1. Add compatibility layer to read both formats
2. Update generation to use new format only
3. Optionally provide migration command for old sessions
4. Remove compatibility layer in future major version

### Error Handling
- Provide clear messages when session not found
- Suggest checking worktree directory names
- Handle partial matches with confirmation

### Future Considerations
- Consider adding session ID aliases for user-friendly names
- Consider session ID prefix customization for organizations
- Consider session archival with ID preservation

## Migration and Compatibility

### Breaking Changes
- None for reading existing sessions
- New sessions will use different ID format
- Resume commands will need adjustment for new sessions

### Migration Path
1. Deploy with backward compatibility
2. New sessions use unified format
3. Existing sessions continue to work
4. Document migration for users
5. Eventually deprecate old format support

### Compatibility Matrix
| Prodigy Version | Read cook-{timestamp} | Write cook-{timestamp} | Read session-{uuid} | Write session-{uuid} |
|-----------------|----------------------|------------------------|---------------------|----------------------|
| Current         | ✓                    | ✓                      | ✗                   | ✗                    |
| After Update    | ✓                    | ✗                      | ✓                   | ✓                    |
| Future (v2.0)   | ✗                    | ✗                      | ✓                   | ✓                    |