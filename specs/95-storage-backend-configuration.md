---
number: 95
title: Storage Backend Configuration System
category: storage
priority: high
status: draft
dependencies: [93, 94]
created: 2025-01-18
---

# Specification 95: Storage Backend Configuration System

**Category**: storage
**Priority**: high
**Status**: draft
**Dependencies**: [93 - Storage Abstraction Layer, 94 - PostgreSQL Storage Backend]

## Context

With the storage abstraction layer (Spec 93) and PostgreSQL backend (Spec 94) in place, Prodigy needs a clean configuration system to allow users to choose their storage backend based on their deployment environment. Rather than complex migration tools, users should be able to simply configure their preferred storage backend through environment variables or configuration files.

This approach simplifies deployment scenarios: developers use file storage locally for simplicity, while production deployments use PostgreSQL for scalability and concurrent access. Container deployments can leverage database storage from day one without migration concerns, and teams can choose the storage backend that best fits their operational requirements.

## Objective

Create a configuration-driven storage backend selection system that allows users to choose between file-based storage (current default) and database storage (PostgreSQL initially) through simple environment variables or configuration files. The system must maintain backward compatibility, provide sensible defaults, validate configurations at startup, and support future storage backends without code changes.

## Requirements

### Functional Requirements
- Environment variable-based storage backend selection
- Configuration file support for complex setups
- Automatic backend detection and initialization
- Connection validation and health checks at startup
- Graceful fallback to file storage if database unavailable
- Support for backend-specific configuration options
- Configuration validation with clear error messages
- Runtime configuration discovery without restart
- Support for multiple storage backend types (file, postgres, sqlite)
- Per-backend connection pooling and resource management

### Non-Functional Requirements
- Zero breaking changes for existing file storage users
- Startup time increase <2 seconds for backend initialization
- Clear error messages for configuration issues
- Support for Docker secrets and environment injection
- Minimal memory overhead for configuration management
- Thread-safe configuration access
- Support for configuration hot-reload (future)
- Extensible design for additional backends

## Acceptance Criteria

- [ ] File storage remains default without any configuration
- [ ] PostgreSQL backend activates with PRODIGY_STORAGE_BACKEND=postgres
- [ ] Database connection validates at startup with clear errors
- [ ] Invalid configurations fail fast with actionable messages
- [ ] Configuration from environment variables works correctly
- [ ] Configuration from file (prodigy.toml) works correctly
- [ ] Backend-specific options (connection pool, timeouts) configurable
- [ ] Docker deployments can use environment-based configuration
- [ ] Documentation covers all configuration options
- [ ] Integration tests pass with both storage backends

## Technical Details

### Implementation Approach

1. **Configuration Schema**
   ```rust
   use serde::{Deserialize, Serialize};

   #[derive(Debug, Clone, Deserialize, Serialize)]
   pub struct StorageConfiguration {
       #[serde(default = "default_backend")]
       pub backend: StorageBackend,

       #[serde(flatten)]
       pub backend_config: BackendConfiguration,

       #[serde(default)]
       pub connection_pool: PoolConfiguration,

       #[serde(default)]
       pub retry_policy: RetryConfiguration,
   }

   #[derive(Debug, Clone, Deserialize, Serialize)]
   #[serde(rename_all = "lowercase")]
   pub enum StorageBackend {
       File,
       Postgres,
       #[serde(skip)]
       Sqlite, // Future
   }

   #[derive(Debug, Clone, Deserialize, Serialize)]
   #[serde(untagged)]
   pub enum BackendConfiguration {
       File(FileConfiguration),
       Postgres(PostgresConfiguration),
       Sqlite(SqliteConfiguration),
   }

   fn default_backend() -> StorageBackend {
       StorageBackend::File
   }
   ```

2. **Environment Variable Schema**
   ```bash
   # Backend selection
   PRODIGY_STORAGE_BACKEND=file|postgres|sqlite

   # File storage configuration
   PRODIGY_FILE_STORAGE_PATH=~/.prodigy
   PRODIGY_FILE_STORAGE_PERMISSIONS=0755

   # PostgreSQL configuration
   PRODIGY_POSTGRES_URL=postgresql://user:pass@localhost/prodigy
   PRODIGY_POSTGRES_MAX_CONNECTIONS=10
   PRODIGY_POSTGRES_MIN_CONNECTIONS=2
   PRODIGY_POSTGRES_CONNECT_TIMEOUT=30
   PRODIGY_POSTGRES_IDLE_TIMEOUT=300
   PRODIGY_POSTGRES_MAX_LIFETIME=3600
   PRODIGY_POSTGRES_SSL_MODE=prefer

   # SQLite configuration (future)
   PRODIGY_SQLITE_PATH=~/.prodigy/data.db
   PRODIGY_SQLITE_JOURNAL_MODE=wal
   PRODIGY_SQLITE_SYNCHRONOUS=normal

   # Common configuration
   PRODIGY_STORAGE_RETRY_MAX_ATTEMPTS=3
   PRODIGY_STORAGE_RETRY_BACKOFF_MS=100
   PRODIGY_STORAGE_HEALTH_CHECK_INTERVAL=60
   ```

3. **Configuration File Format (prodigy.toml)**
   ```toml
   [storage]
   backend = "postgres"  # or "file", "sqlite"

   [storage.postgres]
   url = "postgresql://localhost/prodigy"
   max_connections = 20
   min_connections = 5
   connect_timeout = 30
   ssl_mode = "require"

   [storage.pool]
   max_lifetime = 3600
   idle_timeout = 300

   [storage.retry]
   max_attempts = 3
   backoff_ms = 100
   max_backoff_ms = 5000

   [storage.file]
   path = "~/.prodigy"
   permissions = "0755"
   ```

4. **Configuration Loading Strategy**
   ```rust
   pub struct ConfigurationLoader;

   impl ConfigurationLoader {
       pub async fn load() -> Result<StorageConfiguration> {
           // Priority order:
           // 1. Environment variables (highest priority)
           // 2. Configuration file (prodigy.toml)
           // 3. Default values (lowest priority)

           let mut config = Self::load_defaults();

           // Load from config file if exists
           if let Ok(file_config) = Self::load_from_file().await {
               config.merge(file_config)?;
           }

           // Override with environment variables
           if let Ok(env_config) = Self::load_from_env() {
               config.merge(env_config)?;
           }

           // Validate final configuration
           config.validate()?;

           Ok(config)
       }

       fn load_from_env() -> Result<StorageConfiguration> {
           let backend = env::var("PRODIGY_STORAGE_BACKEND")
               .unwrap_or_else(|_| "file".to_string());

           let backend_config = match backend.as_str() {
               "postgres" => BackendConfiguration::Postgres(
                   PostgresConfiguration::from_env()?
               ),
               "sqlite" => BackendConfiguration::Sqlite(
                   SqliteConfiguration::from_env()?
               ),
               _ => BackendConfiguration::File(
                   FileConfiguration::from_env()?
               ),
           };

           Ok(StorageConfiguration {
               backend: backend.parse()?,
               backend_config,
               connection_pool: PoolConfiguration::from_env()?,
               retry_policy: RetryConfiguration::from_env()?,
           })
       }
   }
   ```

5. **Storage Factory with Configuration**
   ```rust
   pub struct StorageFactory;

   impl StorageFactory {
       pub async fn create_from_config(
           config: &StorageConfiguration
       ) -> Result<Box<dyn UnifiedStorage>> {
           match &config.backend_config {
               BackendConfiguration::File(file_config) => {
                   let storage = FileStorage::new(file_config)?;
                   Ok(Box::new(storage))
               }
               BackendConfiguration::Postgres(pg_config) => {
                   let storage = PostgresStorage::new(
                       pg_config,
                       &config.connection_pool
                   ).await?;

                   // Validate connection
                   storage.health_check().await
                       .map_err(|e| anyhow!(
                           "PostgreSQL connection failed: {}. \
                            Falling back to file storage.", e
                       ))?;

                   Ok(Box::new(storage))
               }
               BackendConfiguration::Sqlite(sqlite_config) => {
                   let storage = SqliteStorage::new(sqlite_config).await?;
                   Ok(Box::new(storage))
               }
           }
       }

       pub async fn create_with_fallback() -> Result<Box<dyn UnifiedStorage>> {
           let config = ConfigurationLoader::load().await?;

           match Self::create_from_config(&config).await {
               Ok(storage) => Ok(storage),
               Err(e) => {
                   warn!("Failed to create configured storage: {}. \
                          Falling back to file storage.", e);

                   let file_config = FileConfiguration::default();
                   Ok(Box::new(FileStorage::new(&file_config)?))
               }
           }
       }
   }
   ```

### Architecture Changes

- Add configuration module to storage subsystem
- Integrate with existing ConfigLoader for unified configuration
- Modify application initialization to select storage backend
- Add health check endpoints for storage backends
- Update Docker images with environment variable support

### Data Structures

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresConfiguration {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
    #[serde(default)]
    pub ssl_mode: SslMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileConfiguration {
    #[serde(default = "default_storage_path")]
    pub path: PathBuf,
    #[serde(default = "default_permissions")]
    pub permissions: u32,
    #[serde(default)]
    pub use_memory_cache: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoolConfiguration {
    pub max_lifetime: Option<u64>,
    pub idle_timeout: Option<u64>,
    pub connection_timeout: u64,
}

fn default_storage_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".prodigy")
}
```

### APIs and Interfaces

```rust
// Main application integration
pub async fn initialize_storage() -> Result<Arc<dyn UnifiedStorage>> {
    let storage = StorageFactory::create_with_fallback().await?;

    info!("Storage backend initialized: {:?}", storage.backend_type());

    // Run startup health check
    storage.health_check().await?;

    Ok(Arc::new(storage))
}

// Health check endpoint
pub async fn storage_health(
    storage: &dyn UnifiedStorage
) -> Result<HealthStatus> {
    storage.health_check().await
}

// Configuration validation
pub trait ConfigurationValidator {
    fn validate(&self) -> Result<()>;
    fn validate_connection(&self) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: [93 - Storage Abstraction Layer, 94 - PostgreSQL Storage Backend]
- **Affected Components**:
  - Main application initialization
  - Docker container configuration
  - CLI commands that initialize storage
  - Test infrastructure
- **External Dependencies**:
  - config - Configuration management
  - dotenv - Environment file support
  - dirs - Platform-specific directory paths

## Testing Strategy

- **Unit Tests**: Configuration parsing and validation
- **Integration Tests**: Backend initialization with various configs
- **Environment Tests**: Docker container configuration
- **Fallback Tests**: Graceful degradation to file storage
- **Validation Tests**: Invalid configuration handling
- **Performance Tests**: Startup time with different backends

## Documentation Requirements

- **Configuration Guide**: Complete reference for all options
- **Docker Documentation**: Container deployment with databases
- **Migration Guide**: Moving from file to database storage
- **Troubleshooting**: Common configuration issues and solutions

## Implementation Notes

- Use lazy static for configuration to avoid repeated parsing
- Implement connection pooling for database backends
- Add prometheus metrics for storage backend monitoring
- Consider using figment for unified configuration management
- Support Docker secrets for sensitive configuration
- Validate database schema version at startup
- Log configuration (with secrets redacted) at startup for debugging

## Migration and Compatibility

- File storage remains the default with zero configuration
- Existing installations continue working without changes
- Configuration file is optional and backwards compatible
- Environment variables take precedence for container deployments
- Future backends can be added without breaking changes
- Support for gradual rollout through feature flags