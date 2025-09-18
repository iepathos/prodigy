---
number: 93
title: Storage Abstraction Layer for Container Support
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-17
---

# Specification 93: Storage Abstraction Layer for Container Support

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently uses a file-based storage architecture that works well for single-machine execution but has significant limitations for container-based and distributed deployments. The existing implementation directly couples business logic to filesystem operations, making it difficult to support alternative storage backends needed for containerized environments where multiple containers need to coordinate and share state.

The current storage components (sessions, checkpoints, events, DLQ, workflows) are scattered across different modules with inconsistent interfaces and no unified abstraction. This tight coupling to filesystem operations creates issues with concurrent access, lacks distributed locking mechanisms, has poor query performance for large datasets, and provides no ACID guarantees for critical state updates.

## Objective

Create a comprehensive storage abstraction layer that decouples Prodigy's business logic from specific storage implementations, enabling seamless support for both file-based storage (for local development) and database-backed storage (for container deployments). This abstraction must maintain backward compatibility while providing the foundation for horizontal scaling, distributed execution, and cloud-native deployments.

## Requirements

### Functional Requirements
- Define trait-based abstractions for all storage operations (sessions, events, checkpoints, DLQ, workflows)
- Support multiple storage backends through a common interface (file, PostgreSQL, Redis, S3)
- Provide distributed locking mechanisms for concurrent access control
- Enable atomic operations and transaction support where applicable
- Support both synchronous and asynchronous storage operations
- Implement efficient query and filtering capabilities across all data types
- Provide storage backend discovery and automatic configuration from environment
- Support streaming interfaces for large datasets (events, logs)
- Enable storage backend health checks and monitoring

### Non-Functional Requirements
- Zero breaking changes to existing APIs and workflows
- Minimal performance overhead for abstraction layer (<5% latency increase)
- Support for at least 10,000 concurrent operations
- Storage operations must be idempotent and retry-safe
- Comprehensive error handling with detailed context
- Full backward compatibility with existing file-based storage
- Support for gradual migration without downtime
- Extensible design for future storage backends

## Acceptance Criteria

- [ ] All existing storage operations work through the new abstraction layer
- [ ] File-based backend passes all existing tests without modification
- [ ] PostgreSQL backend implementation with full feature parity
- [ ] Distributed locking mechanism prevents concurrent modification conflicts
- [ ] Storage backend can be selected via environment configuration
- [ ] Performance benchmarks show <5% overhead for abstraction layer
- [ ] Migration tool successfully transfers data between storage backends
- [ ] Integration tests pass with both file and database backends
- [ ] Documentation covers all trait methods and backend configuration
- [ ] Error handling provides clear, actionable error messages

## Technical Details

### Implementation Approach

1. **Trait Definition Phase**
   - Create `UnifiedStorage` trait with all storage operations
   - Define sub-traits for specific storage domains (SessionStorage, EventStorage, etc.)
   - Implement error types and result types for storage operations
   - Design configuration structures for different backends

2. **File Backend Refactoring**
   - Extract existing file operations into trait implementations
   - Maintain exact same behavior and file formats
   - Add proper error handling and retry logic
   - Implement file-based locking mechanism

3. **Database Backend Implementation**
   - Start with PostgreSQL as primary database backend
   - Use connection pooling for efficiency
   - Implement schema migrations with versioning
   - Add indexes for common query patterns

4. **Backend Selection Logic**
   - Environment-based backend detection
   - Fallback chain for configuration sources
   - Runtime backend switching capability
   - Health check and failover support

### Architecture Changes

```rust
// Core trait hierarchy
pub trait UnifiedStorage: Send + Sync {
    type Lock: StorageLock;

    fn session_storage(&self) -> &dyn SessionStorage;
    fn event_storage(&self) -> &dyn EventStorage;
    fn checkpoint_storage(&self) -> &dyn CheckpointStorage;
    fn dlq_storage(&self) -> &dyn DLQStorage;
    fn workflow_storage(&self) -> &dyn WorkflowStorage;

    async fn acquire_lock(&self, key: &str, ttl: Duration) -> Result<Self::Lock>;
    async fn health_check(&self) -> Result<HealthStatus>;
}

// Storage factory
pub struct StorageFactory;
impl StorageFactory {
    pub fn from_config(config: &StorageConfig) -> Result<Box<dyn UnifiedStorage>>;
    pub fn from_env() -> Result<Box<dyn UnifiedStorage>>;
}
```

### Data Structures

```rust
pub struct StorageConfig {
    pub backend: BackendType,
    pub connection_pool_size: usize,
    pub retry_policy: RetryPolicy,
    pub timeout: Duration,
    pub backend_specific: BackendConfig,
}

pub enum BackendConfig {
    File(FileConfig),
    Postgres(PostgresConfig),
    Redis(RedisConfig),
    S3(S3Config),
}

pub struct StorageLock {
    pub key: String,
    pub holder: String,
    pub acquired_at: DateTime<Utc>,
    pub ttl: Duration,
}
```

### APIs and Interfaces

```rust
#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn save(&self, session: &PersistedSession) -> Result<()>;
    async fn load(&self, id: &SessionId) -> Result<Option<PersistedSession>>;
    async fn list(&self, filter: SessionFilter) -> Result<Vec<SessionId>>;
    async fn delete(&self, id: &SessionId) -> Result<()>;
    async fn update_state(&self, id: &SessionId, state: SessionState) -> Result<()>;
}

#[async_trait]
pub trait EventStorage: Send + Sync {
    async fn append(&self, events: Vec<EventRecord>) -> Result<()>;
    async fn query(&self, filter: EventFilter) -> Result<EventStream>;
    async fn aggregate(&self, job_id: &str) -> Result<EventStats>;
    async fn subscribe(&self, filter: EventFilter) -> Result<EventSubscription>;
}
```

## Dependencies

- **Prerequisites**: None (foundation specification)
- **Affected Components**:
  - session/storage.rs
  - session/persistence.rs
  - cook/workflow/checkpoint.rs
  - cook/execution/events/
  - cook/execution/dlq.rs
  - storage/mod.rs
- **External Dependencies**:
  - async-trait for trait definitions
  - tokio for async runtime
  - serde for serialization
  - Database drivers (sqlx for PostgreSQL, redis-rs for Redis)

## Testing Strategy

- **Unit Tests**: Mock implementations for each trait method
- **Integration Tests**: Full workflow execution with each backend
- **Performance Tests**: Benchmark abstraction overhead
- **Concurrency Tests**: Distributed locking and race condition tests
- **Migration Tests**: Data transfer between different backends
- **Compatibility Tests**: Ensure file format compatibility maintained
- **Failure Tests**: Network failures, storage unavailability, lock timeouts

## Documentation Requirements

- **Code Documentation**: Document all trait methods with examples
- **Migration Guide**: Step-by-step backend migration instructions
- **Configuration Reference**: Complete backend configuration options
- **Architecture Updates**: Update ARCHITECTURE.md with storage layer design

## Implementation Notes

- Start with read operations before implementing writes
- Use feature flags to gradually roll out database backend
- Implement comprehensive logging for storage operations
- Consider using backend-specific optimizations where appropriate
- Design for eventual consistency where strong consistency not required
- Cache frequently accessed data to reduce storage calls
- Implement circuit breakers for backend failures

## Migration and Compatibility

- File-based storage remains default for backward compatibility
- Automatic detection and migration of existing file storage
- Parallel operation during migration for zero downtime
- Rollback capability if migration fails
- Version markers in storage for format evolution
- Clear upgrade path documentation