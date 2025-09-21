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

This redundancy makes it unclear which system to use, increases code complexity, and creates potential for bugs. While the container storage abstraction is well-designed for future distributed deployments (PostgreSQL + Redis), it's premature optimization for current single-machine use cases.

## Objective

Consolidate storage to use a single, file-based global storage system (default: `~/.prodigy/`) as the primary implementation, making it configurable so users can set custom paths. Remove legacy local storage while preserving the distributed storage abstraction layer as dormant code for future containerized deployments.

## Requirements

### Functional Requirements
- Migrate all storage operations to use configurable global storage
- Support custom storage paths via configuration (default: `~/.prodigy/`)
- Remove legacy local storage code and environment variable support
- Make file storage the default/only active backend
- Preserve distributed storage code but make it dormant (not compiled by default)
- Ensure automatic migration from local to global storage for existing users
- Maintain all current storage functionality (events, DLQ, state, sessions)

### Non-Functional Requirements
- Zero data loss during migration
- Clear migration messages for users
- Simplified codebase with reduced complexity
- Improved performance by removing abstraction overhead

## Acceptance Criteria

- [ ] All storage operations use configurable global storage (default: `~/.prodigy/`)
- [ ] `PRODIGY_USE_LOCAL_STORAGE` environment variable has no effect
- [ ] Legacy `.prodigy/` directories automatically migrated on first run
- [ ] Distributed storage code preserved but dormant (behind feature flags)
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

4. **Phase 4: Make Distributed Storage Dormant**
   - Add feature flags for distributed backends: `postgres`, `redis`, `s3`
   - Move distributed dependencies behind feature flags in Cargo.toml
   - Ensure distributed storage code compiles but isn't included in default builds
   - Add documentation about future distributed capabilities
   - Keep file-based storage as the only default backend

### Architecture Changes

Storage architecture with future distribution support:
```
Current (Default Build):
  GlobalStorage -> Direct file operations

Future (Feature Flags):
  StorageFactory -> UnifiedStorage trait -> Multiple backends
                 -> File (default) | PostgreSQL | Redis | S3

Removed:
  Local storage (.prodigy/) -> Migrate to Global
```

### Data Structures

Primary (always compiled):
- `GlobalStorage` struct for path management
- Direct file I/O operations
- Simple JSON serialization

Preserved (behind feature flags):
- `UnifiedStorage` trait (for future distributed use)
- `StorageConfig` and backend configurations (postgres, redis, s3 features)
- Backend implementations (postgres, redis, s3 features)
- Storage factory pattern (distributed feature)

### APIs and Interfaces

Simplified storage API with configuration:
```rust
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub base_dir: PathBuf,  // Configurable storage root
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".prodigy"),
        }
    }
}

pub struct GlobalStorage {
    config: StorageConfig,
    repo_name: String,
}

impl GlobalStorage {
    pub fn new(repo_path: &Path) -> Result<Self>
    pub fn with_config(repo_path: &Path, config: StorageConfig) -> Result<Self>
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
- **External Dependencies to Feature-Gate**:
  - aws-sdk-s3 (behind `s3` feature)
  - deadpool-redis (behind `redis` feature)
  - sqlx (behind `postgres` feature)
  - sea-orm (remove if only used for storage)

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
- Support multiple configuration methods:
  - Environment variable: `PRODIGY_STORAGE_DIR=/custom/path`
  - Config file: `~/.prodigy/config.toml`
  - CLI flag: `--storage-dir /custom/path`
- Add storage location to `prodigy info` command
- Log storage operations at debug level for troubleshooting
- Validate storage directory permissions on startup

### Feature Flag Strategy
```toml
# Default build (single-machine)
cargo build

# Future distributed build
cargo build --features postgres,redis

# All storage backends
cargo build --features postgres,redis,s3
```

### Configuration Priority (highest to lowest)
1. CLI flag `--storage-dir`
2. Environment variable `PRODIGY_STORAGE_DIR`
3. Config file setting
4. Default `~/.prodigy/`

### Future Distributed Architecture (PostgreSQL + Redis)
When distributed deployment is needed:
- **PostgreSQL**: Primary durable storage for all data
- **Redis**: High-speed cache, distributed locking, pub/sub coordination
- **S3**: Optional for large file archives and cross-region backups

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