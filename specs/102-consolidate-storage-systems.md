---
number: 102
title: Consolidate Storage Systems
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-21
---

# Specification 102: Consolidate Storage Systems

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently has three parallel storage systems that create confusion and maintenance overhead:
1. Legacy local storage (`.prodigy/`)
2. Global storage (`~/.prodigy/`)
3. Container storage abstraction layer (`src/storage/`)

This redundancy makes it unclear which system to use, increases code complexity, and creates potential for bugs. The container storage abstraction is over-engineered for current needs and not actively used.

## Objective

Consolidate all storage into a single, file-based global storage system (`~/.prodigy/`), removing legacy local storage and deferring container abstractions until actually needed.

## Requirements

### Functional Requirements
- Migrate all storage operations to use global storage exclusively
- Remove legacy local storage code and environment variable support
- Remove unused container storage abstraction layer
- Ensure automatic migration from local to global storage for existing users
- Maintain all current storage functionality (events, DLQ, state, sessions)

### Non-Functional Requirements
- Zero data loss during migration
- Clear migration messages for users
- Simplified codebase with reduced complexity
- Improved performance by removing abstraction overhead

## Acceptance Criteria

- [ ] All storage operations use `~/.prodigy/` exclusively
- [ ] `PRODIGY_USE_LOCAL_STORAGE` environment variable has no effect
- [ ] Legacy `.prodigy/` directories automatically migrated on first run
- [ ] Container storage abstraction code completely removed
- [ ] All tests pass using global storage
- [ ] Storage-related code reduced by at least 40%
- [ ] No references to local storage in documentation

## Technical Details

### Implementation Approach

1. **Phase 1: Audit Storage Usage**
   - Identify all storage access points
   - Map current usage of each storage system
   - Document migration requirements

2. **Phase 2: Consolidate to Global Storage**
   - Update all storage operations to use `GlobalStorage` from `src/storage/mod.rs`
   - Remove conditional logic for local vs global storage
   - Ensure consistent path resolution

3. **Phase 3: Remove Legacy Local Storage**
   - Delete local storage implementation
   - Remove environment variable checks
   - Clean up migration code after making it run once

4. **Phase 4: Remove Container Abstractions**
   - Delete entire `src/storage/backends/` directory
   - Remove `src/storage/traits.rs` abstraction layer
   - Delete unused storage factory and configuration
   - Remove Redis, S3, and PostgreSQL dependencies

### Architecture Changes

Simplified storage architecture:
```
Before:
  StorageFactory -> UnifiedStorage trait -> Multiple backends
                 -> Local/Global file storage

After:
  GlobalStorage -> Direct file operations
```

### Data Structures

Keep only:
- `GlobalStorage` struct for path management
- Direct file I/O operations
- Simple JSON serialization

Remove:
- `UnifiedStorage` trait
- `StorageConfig` and backend configurations
- All backend implementations
- Storage factory pattern

### APIs and Interfaces

Simplified storage API:
```rust
pub struct GlobalStorage {
    base_dir: PathBuf,
    repo_name: String,
}

impl GlobalStorage {
    pub fn new(repo_path: &Path) -> Result<Self>
    pub async fn read_state(&self, job_id: &str) -> Result<State>
    pub async fn write_state(&self, job_id: &str, state: &State) -> Result<()>
    pub async fn read_events(&self, job_id: &str) -> Result<Vec<Event>>
    pub async fn append_event(&self, job_id: &str, event: &Event) -> Result<()>
    // Similar methods for DLQ and sessions
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - All components using storage
  - Session management
  - MapReduce execution
  - DLQ processing
  - Event tracking
- **External Dependencies to Remove**:
  - aws-sdk-s3
  - deadpool-redis
  - sea-orm (if only used for storage)

## Testing Strategy

- **Unit Tests**: Update all storage tests to use global storage
- **Integration Tests**: Test migration from local to global storage
- **Migration Tests**: Verify data integrity during migration
- **Performance Tests**: Ensure no performance degradation

## Documentation Requirements

- **Code Documentation**: Update all storage-related documentation
- **User Documentation**: Document storage location and structure
- **Architecture Updates**: Simplify storage section in ARCHITECTURE.md
- **Migration Guide**: Document automatic migration process

## Implementation Notes

- Ensure migration runs automatically on first use after update
- Keep migration code initially, remove in subsequent release
- Consider adding storage location to `prodigy info` command
- Log storage operations at debug level for troubleshooting

## Migration and Compatibility

Automatic migration on first run:
```
Migrating local storage to global storage...
  Moving .prodigy/events -> ~/.prodigy/your-project/events
  Moving .prodigy/dlq -> ~/.prodigy/your-project/dlq
  Moving .prodigy/state -> ~/.prodigy/your-project/state
Migration complete. Local storage removed.
```

After migration, the local `.prodigy/` directory can be safely deleted.