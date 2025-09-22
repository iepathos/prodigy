---
number: 103
title: Remove Migration Code and Simplify Storage
category: foundation
priority: critical
status: draft
dependencies: [101]
created: 2025-09-22
updated: 2025-09-22
---

# Specification 103: Remove Migration Code and Simplify Storage

## Context

The codebase has successfully transitioned to global storage (`~/.prodigy/`), but still contains migration code and legacy configuration structures. Since we're in early prototyping phase, backward compatibility is not a concern. This creates unnecessary complexity that violates VISION.md principles.

Current unnecessary code identified:
- Migration module in `storage/mod.rs::migration`
- Migration file `storage/migrate.rs`
- `LegacyStorageConfig` struct in `storage/mod.rs`
- `StorageMigrator`, `MigrationConfig`, `MigrationStats` exports
- Migration-related CLI commands and references
- Automatic migration checks on startup

## Objective

Remove all migration-related code and legacy storage configurations to simplify the codebase, since backward compatibility is not needed during prototyping phase. The global storage system is already the only implementation in use.

## Requirements

### Functional Requirements
- Remove all migration-related code and modules
- Remove `LegacyStorageConfig` struct and related code
- Remove `storage/migrate.rs` file completely
- Clean up imports and exports in `storage/mod.rs`
- Remove any migration CLI commands or subcommands
- Remove migration checks from application startup
- Ensure global storage remains the only implementation

### Non-Functional Requirements
- Reduced binary size from code elimination
- Simplified codebase with less maintenance burden
- Clearer code structure without legacy compatibility layers
- All existing tests should still pass after removal

## Acceptance Criteria

- [ ] `storage/migrate.rs` file deleted
- [ ] Migration module removed from `storage/mod.rs`
- [ ] `LegacyStorageConfig` struct removed
- [ ] Migration-related exports removed (`StorageMigrator`, `MigrationConfig`, `MigrationStats`)
- [ ] No references to `.prodigy/` local storage remain in code
- [ ] All migration CLI commands removed
- [ ] Application starts without checking for local storage
- [ ] All tests pass after removal
- [ ] Documentation updated to remove migration references

## Technical Details

### Implementation Approach

1. **Remove Migration Module from storage/mod.rs**
   - Delete the entire `migration` module (lines 166-297)
   - Remove `pub mod migrate;` import
   - Remove migration-related exports

2. **Delete storage/migrate.rs File**
   - Remove the entire file from the codebase
   - Update any references in other files

3. **Remove LegacyStorageConfig**
   - Delete `LegacyStorageConfig` struct and its implementations
   - Remove any usage of this struct throughout the codebase

4. **Clean Up References**
   - Search for and remove any references to `.prodigy/` local storage
   - Remove migration checks from application startup
   - Remove migration-related CLI commands

### Files to be Modified

- `storage/mod.rs` - Remove migration module, LegacyStorageConfig, and related exports
- `storage/migrate.rs` - Delete entirely
- `main.rs` - Remove any migration checks on startup
- `cli/*.rs` - Remove migration-related commands
- Any files with references to local `.prodigy/` storage

## Dependencies

- **Spec 101**: Proper error handling must be maintained in remaining global storage code

## Testing Strategy

- Ensure all existing storage tests pass after removal
- Verify no references to local storage remain
- Test that application starts without migration checks
- Confirm global storage handles all operations correctly
- Check that binary size is reduced after code removal

## Documentation Requirements

- Remove all references to local `.prodigy/` storage from documentation
- Remove migration instructions and troubleshooting guides
- Update CLAUDE.md to remove migration references
- Update README if it mentions local storage
- Ensure all documentation reflects global-only storage architecture