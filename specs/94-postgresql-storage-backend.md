---
number: 94
title: PostgreSQL Storage Backend Implementation
category: storage
priority: critical
status: draft
dependencies: [93]
created: 2025-01-17
---

# Specification 94: PostgreSQL Storage Backend Implementation

**Category**: storage
**Priority**: critical
**Status**: draft
**Dependencies**: [93 - Storage Abstraction Layer]

## Context

With the storage abstraction layer defined in Specification 93, we need a robust database backend implementation to support container-based deployments. PostgreSQL with JSONB provides the ideal balance of flexibility, performance, and operational simplicity. It offers ACID transactions, sophisticated indexing, built-in pub/sub capabilities, and mature ecosystem support while maintaining the schema flexibility needed for Prodigy's evolving data structures.

PostgreSQL's JSONB type allows us to store complex, nested data structures without rigid schema requirements, while still providing efficient querying through GIN indexes. The database's advisory locks provide distributed coordination, LISTEN/NOTIFY enables real-time event streaming, and partitioning supports efficient time-series data management for events.

## Objective

Implement a complete PostgreSQL storage backend that fulfills all requirements of the UnifiedStorage trait, providing production-ready database storage for Prodigy's container deployments. The implementation must support high concurrency, efficient queries, real-time event streaming, and seamless data migration while maintaining data integrity and performance at scale.

## Requirements

### Functional Requirements
- Implement all UnifiedStorage trait methods for PostgreSQL
- Design and implement optimized database schema with JSONB columns
- Provide connection pooling and efficient resource management
- Implement advisory locks for distributed coordination
- Support LISTEN/NOTIFY for real-time event streaming
- Enable time-based partitioning for events table
- Implement efficient batch operations for high throughput
- Support full-text search on JSONB fields
- Provide database migration and versioning system
- Enable point-in-time recovery capabilities

### Non-Functional Requirements
- Support 10,000+ concurrent connections through pooling
- Query response time <10ms for indexed operations
- Event insertion throughput >50,000 events/second
- Automatic connection retry and failover
- Comprehensive query optimization with EXPLAIN analysis
- Support for read replicas for scaling read operations
- Efficient storage with automatic vacuuming
- Monitoring and metrics exposure for operations

## Acceptance Criteria

- [ ] All UnifiedStorage trait methods implemented for PostgreSQL
- [ ] Database schema created with proper indexes and constraints
- [ ] Connection pooling configured with health checks
- [ ] Advisory locks prevent concurrent modification conflicts
- [ ] LISTEN/NOTIFY enables real-time event subscriptions
- [ ] Events table partitioned by month with automatic creation
- [ ] Batch operations reduce round-trip overhead by >80%
- [ ] Performance benchmarks meet or exceed requirements
- [ ] Migration tools transfer data from file storage
- [ ] Integration tests pass with PostgreSQL backend
- [ ] Monitoring exposes key metrics (connections, query time, lock waits)

## Technical Details

### Implementation Approach

1. **Schema Design**
   ```sql
   -- Sessions with JSONB for flexibility
   CREATE TABLE sessions (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       session_id TEXT UNIQUE NOT NULL,
       repo_name TEXT NOT NULL,
       state JSONB NOT NULL,
       metadata JSONB DEFAULT '{}',
       created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       version INTEGER DEFAULT 1
   );
   CREATE INDEX idx_sessions_repo ON sessions(repo_name);
   CREATE INDEX idx_sessions_state ON sessions USING GIN(state);

   -- Events with time-based partitioning
   CREATE TABLE events (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       job_id TEXT NOT NULL,
       event_type TEXT NOT NULL,
       correlation_id TEXT,
       data JSONB NOT NULL,
       created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
   ) PARTITION BY RANGE (created_at);

   CREATE INDEX idx_events_job ON events(job_id, created_at DESC);
   CREATE INDEX idx_events_type ON events(event_type, created_at DESC);
   CREATE INDEX idx_events_correlation ON events(correlation_id) WHERE correlation_id IS NOT NULL;
   CREATE INDEX idx_events_data ON events USING GIN(data);

   -- Automatic monthly partitions
   CREATE OR REPLACE FUNCTION create_monthly_partitions()
   RETURNS void AS $$
   DECLARE
       start_date date;
       end_date date;
       partition_name text;
   BEGIN
       start_date := date_trunc('month', CURRENT_DATE);
       end_date := start_date + interval '1 month';
       partition_name := 'events_' || to_char(start_date, 'YYYY_MM');

       EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF events FOR VALUES FROM (%L) TO (%L)',
           partition_name, start_date, end_date);
   END;
   $$ LANGUAGE plpgsql;

   -- Checkpoints with versioning
   CREATE TABLE checkpoints (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       workflow_id TEXT NOT NULL,
       version INTEGER NOT NULL,
       state JSONB NOT NULL,
       metadata JSONB DEFAULT '{}',
       created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       UNIQUE(workflow_id, version)
   );
   CREATE INDEX idx_checkpoints_workflow ON checkpoints(workflow_id, version DESC);

   -- DLQ with failure tracking
   CREATE TABLE dlq_items (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       job_id TEXT NOT NULL,
       item_id TEXT NOT NULL,
       item_data JSONB NOT NULL,
       failure_details JSONB NOT NULL,
       retry_count INTEGER DEFAULT 0,
       reprocess_eligible BOOLEAN DEFAULT true,
       created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       UNIQUE(job_id, item_id)
   );
   CREATE INDEX idx_dlq_job ON dlq_items(job_id);
   CREATE INDEX idx_dlq_eligible ON dlq_items(reprocess_eligible) WHERE reprocess_eligible = true;

   -- Advisory locks registry
   CREATE TABLE storage_locks (
       lock_key TEXT PRIMARY KEY,
       holder_id TEXT NOT NULL,
       acquired_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
       expires_at TIMESTAMPTZ NOT NULL,
       metadata JSONB DEFAULT '{}'
   );
   CREATE INDEX idx_locks_expires ON storage_locks(expires_at);
   ```

2. **Connection Pool Configuration**
   ```rust
   pub struct PostgresStorage {
       pool: PgPool,
       config: PostgresConfig,
       metrics: Arc<StorageMetrics>,
   }

   impl PostgresStorage {
       pub async fn new(config: PostgresConfig) -> Result<Self> {
           let pool = PgPoolOptions::new()
               .max_connections(config.max_connections)
               .min_connections(config.min_connections)
               .connect_timeout(config.connect_timeout)
               .idle_timeout(config.idle_timeout)
               .max_lifetime(config.max_lifetime)
               .before_acquire(|conn, _| Box::pin(async move {
                   conn.ping().await?;
                   Ok(true)
               }))
               .connect(&config.url)
               .await?;

           Ok(Self {
               pool,
               config,
               metrics: Arc::new(StorageMetrics::new()),
           })
       }
   }
   ```

3. **Advisory Lock Implementation**
   ```rust
   impl PostgresStorage {
       async fn acquire_lock(&self, key: &str, ttl: Duration) -> Result<StorageLock> {
           let lock_id = hash_to_bigint(key);
           let holder_id = Uuid::new_v4().to_string();

           // Try advisory lock
           let acquired: bool = sqlx::query_scalar(
               "SELECT pg_try_advisory_lock($1)"
           )
           .bind(lock_id)
           .fetch_one(&self.pool)
           .await?;

           if !acquired {
               return Err(StorageError::LockContention(key.to_string()));
           }

           // Record in locks table
           let expires_at = Utc::now() + ttl;
           sqlx::query(
               "INSERT INTO storage_locks (lock_key, holder_id, expires_at)
                VALUES ($1, $2, $3)
                ON CONFLICT (lock_key) DO UPDATE SET
                holder_id = $2, expires_at = $3, acquired_at = CURRENT_TIMESTAMP"
           )
           .bind(key)
           .bind(&holder_id)
           .bind(expires_at)
           .execute(&self.pool)
           .await?;

           Ok(StorageLock {
               key: key.to_string(),
               holder: holder_id,
               lock_id,
               expires_at,
           })
       }
   }
   ```

4. **Event Streaming with LISTEN/NOTIFY**
   ```rust
   impl EventStorage for PostgresStorage {
       async fn subscribe(&self, filter: EventFilter) -> Result<EventSubscription> {
           let mut listener = PgListener::connect(&self.config.url).await?;

           let channel = format!("events_{}", filter.job_id.unwrap_or_default());
           listener.listen(&channel).await?;

           let (tx, rx) = mpsc::channel(1000);

           tokio::spawn(async move {
               while let Ok(notification) = listener.recv().await {
                   if let Ok(event) = serde_json::from_str::<EventRecord>(&notification.payload()) {
                       if filter.matches(&event) {
                           let _ = tx.send(event).await;
                       }
                   }
               }
           });

           Ok(EventSubscription::new(rx))
       }

       async fn append(&self, events: Vec<EventRecord>) -> Result<()> {
           let mut tx = self.pool.begin().await?;

           // Batch insert for efficiency
           for chunk in events.chunks(1000) {
               let mut query_builder = QueryBuilder::new(
                   "INSERT INTO events (job_id, event_type, correlation_id, data) "
               );

               query_builder.push_values(chunk, |mut b, event| {
                   b.push_bind(&event.job_id)
                    .push_bind(&event.event_type)
                    .push_bind(&event.correlation_id)
                    .push_bind(&event.data);
               });

               query_builder.build().execute(&mut tx).await?;

               // Notify listeners
               for event in chunk {
                   let channel = format!("events_{}", event.job_id);
                   let payload = serde_json::to_string(&event)?;

                   sqlx::query("SELECT pg_notify($1, $2)")
                       .bind(&channel)
                       .bind(&payload)
                       .execute(&mut tx)
                       .await?;
               }
           }

           tx.commit().await?;
           Ok(())
       }
   }
   ```

### Architecture Changes

- Add database connection management layer
- Implement retry logic with exponential backoff
- Add query builder for complex dynamic queries
- Integrate with existing metrics system
- Add database health monitoring

### Data Structures

```rust
pub struct PostgresConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
    pub ssl_mode: SslMode,
    pub application_name: String,
}

pub struct StorageMetrics {
    pub queries_total: Counter,
    pub query_duration: Histogram,
    pub connections_active: Gauge,
    pub lock_contentions: Counter,
    pub errors_total: Counter,
}
```

## Dependencies

- **Prerequisites**: [93 - Storage Abstraction Layer]
- **Affected Components**: All storage-dependent modules
- **External Dependencies**:
  - sqlx (0.7) - PostgreSQL driver with compile-time checking
  - tokio-postgres - Async PostgreSQL client
  - deadpool-postgres - Connection pooling
  - postgres-types - Type conversions
  - pg_advisory_lock - Advisory lock utilities

## Testing Strategy

- **Unit Tests**: Mock database for trait implementation tests
- **Integration Tests**: Real PostgreSQL instance via Docker
- **Performance Tests**: Benchmark against requirements
- **Concurrency Tests**: Multiple containers accessing same data
- **Failure Tests**: Connection loss, lock timeouts, partition failures
- **Migration Tests**: File to PostgreSQL data transfer
- **Load Tests**: 10,000 concurrent connections simulation

## Documentation Requirements

- **Setup Guide**: PostgreSQL installation and configuration
- **Schema Documentation**: Table descriptions and relationships
- **Performance Tuning**: Index optimization and query tuning
- **Operations Guide**: Backup, recovery, monitoring procedures
- **Migration Documentation**: Step-by-step migration from file storage

## Implementation Notes

- Use prepared statements for all queries to prevent SQL injection
- Implement connection retry with circuit breaker pattern
- Use COPY for bulk data operations when possible
- Consider read replicas for read-heavy workloads
- Implement automatic partition management for events
- Use EXPLAIN ANALYZE to optimize slow queries
- Add prometheus metrics for monitoring
- Consider using pgbouncer for connection pooling at scale

## Migration and Compatibility

- Provide tool to export file storage to SQL dump
- Support dual-write mode during migration
- Implement checksum validation for migrated data
- Allow rollback to file storage if needed
- Version all schema changes with migrations
- Support zero-downtime schema updates