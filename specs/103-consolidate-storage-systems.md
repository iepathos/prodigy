---
number: 103
title: Consolidate Duplicate Storage Systems
category: foundation
priority: critical
status: draft
dependencies: [101]
created: 2025-09-22
---

# Specification 103: Consolidate Duplicate Storage Systems

## Context

The codebase currently maintains two parallel storage implementations: legacy local storage (`.prodigy/`) and newer global storage (`~/.prodigy/`), along with migration logic. This duplication violates VISION.md principles of simplicity and creates maintenance burden, potential data consistency issues, and user confusion.

Current duplication identified:
- Legacy local storage in `.prodigy/`
- Global storage in `~/.prodigy/`
- Migration logic in `storage/migrate.rs`
- Different session management approaches
- Inconsistent error handling between systems

## Objective

Consolidate to a single, unified storage system using global storage architecture, eliminating legacy local storage while ensuring zero data loss during migration and simplifying the codebase.

## Requirements

### Functional Requirements
- Remove all legacy local storage code
- Ensure automatic migration of existing local data
- Maintain backward compatibility during transition period
- Provide clear migration status and error reporting
- Support rollback if migration fails
- Unified API for all storage operations

### Non-Functional Requirements
- Zero data loss during migration
- Migration must be atomic (all or nothing)
- Performance should improve with single storage system
- Reduced binary size from code elimination
- Clear error messages for migration issues

## Acceptance Criteria

- [ ] All legacy local storage code removed
- [ ] Single storage implementation in global architecture
- [ ] Automatic migration on first run with new version
- [ ] Migration includes verification and rollback capability
- [ ] All storage tests pass with unified system
- [ ] Documentation updated to reflect global-only storage
- [ ] CLI help reflects current storage behavior
- [ ] Reduced binary size from code elimination

## Technical Details

### Implementation Approach

1. **Phase 1: Strengthen Global Storage**
   - Ensure global storage handles all legacy use cases
   - Add comprehensive error handling and validation
   - Implement atomic operations for critical data

2. **Phase 2: Enhanced Migration**
   - Improve migration robustness with verification
   - Add rollback capability for failed migrations
   - Provide detailed migration progress and logging

3. **Phase 3: Legacy Code Removal**
   - Remove local storage implementations
   - Remove conditional logic that switches between systems
   - Simplify API to single storage interface

### Migration Safety Pattern

```rust
pub fn migrate_to_global_storage() -> Result<MigrationResult> {
    let backup = create_backup_of_local_storage()?;

    match attempt_migration() {
        Ok(result) => {
            verify_migration_integrity(&result)?;
            remove_local_storage()?;
            cleanup_backup(backup)?;
            Ok(result)
        }
        Err(error) => {
            restore_from_backup(backup)?;
            Err(error.context("Migration failed, local storage restored"))
        }
    }
}
```

### Files to be Removed/Simplified

- `storage/local.rs` - Legacy local storage implementation
- `storage/migrate.rs` - Complex migration logic (simplify to one-way)
- Conditional storage selection logic throughout codebase
- Legacy session management in various modules

## Dependencies

- **Spec 101**: Proper error handling required for safe migration

## Testing Strategy

- Create test scenarios with various local storage states
- Test migration failure and rollback scenarios
- Verify data integrity after migration
- Performance tests comparing unified vs dual systems
- Integration tests for storage API consistency

## Documentation Requirements

- Update storage architecture documentation
- Remove references to local storage from user guides
- Document migration process and troubleshooting
- Update development setup instructions
- Add storage layout documentation for global system