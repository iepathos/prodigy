## Default Values Reference

Complete reference of all default configuration values in Prodigy. These defaults are used when settings are not explicitly configured in global config, project config, or environment variables.

!!! info "How to Use This Reference"
    This page documents built-in defaults for all configuration settings. Values shown here are what Prodigy uses when no explicit configuration is provided. See [Environment Variables](environment-variables.md) for override options.

### Global Configuration Defaults

<!-- Source: src/config/mod.rs:108-120 (GlobalConfig::default) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `prodigy_home` | `~/.prodigy` | Global storage directory (platform-specific) |
| `default_editor` | None | Text editor (falls back to `$EDITOR`) |
| `log_level` | `"info"` | Logging verbosity (`trace`, `debug`, `info`, `warn`, `error`) |
| `claude_api_key` | None | Claude API key (use environment variable) |
| `max_concurrent_specs` | `1` | Maximum concurrent spec implementations |
| `auto_commit` | `true` | Automatically commit after successful commands |
| `plugins.enabled` | `false` | Enable plugin system |
| `plugins.directory` | `~/.prodigy/plugins` | Plugin directory |
| `plugins.auto_load` | `[]` | Plugins to load on startup |

### Project Configuration Defaults

<!-- Source: src/config/mod.rs:83-92 (ProjectConfig struct) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `name` | **Required** | Project identifier (no default) |
| `description` | None | Human-readable project description |
| `version` | None | Project version |
| `spec_dir` | `"specs"` | Directory containing specification files |
| `claude_api_key` | Inherits from global | Project-specific API key |
| `auto_commit` | Inherits from global | Project-specific auto-commit setting |
| `variables` | `{}` | Project variables for workflows |

### Storage Configuration Defaults

<!-- Source: src/storage/config.rs:25-56 (StorageConfig struct), 229-241 (Default impl) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `backend` | `"file"` | Storage backend type (`file` or `memory`) |
| `connection_pool_size` | `10` | Connection pool size (for future backends) |
| `timeout` | `30s` | Default operation timeout |
| `enable_locking` | `true` | Enable distributed locking |
| `enable_cache` | `false` | Enable caching layer |

### File Storage Defaults

<!-- Source: src/storage/config.rs:67-87 (FileConfig struct) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `base_dir` | `~/.prodigy` | Base storage directory |
| `use_global` | `true` | Use global storage (recommended) |
| `enable_file_locks` | `true` | Enable file-based locking |
| `max_file_size` | `104857600` (100MB) | Max file size before rotation |
| `enable_compression` | `false` | Compress archived files |

### Memory Storage Defaults

<!-- Source: src/storage/config.rs:90-112 (MemoryConfig struct and Default impl) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_memory` | `104857600` (100MB) | Maximum memory usage |
| `persist_to_disk` | `false` | Persist memory storage to disk |
| `persistence_path` | None | Path for disk persistence |

### Retry Policy Defaults

<!-- Source: src/storage/config.rs:115-147 (RetryPolicy struct and Default impl) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_retries` | `3` | Maximum retry attempts |
| `initial_delay` | `1s` | Initial retry delay |
| `max_delay` | `30s` | Maximum retry delay (with backoff) |
| `backoff_multiplier` | `2.0` | Exponential backoff multiplier |
| `jitter` | `true` | Add random jitter to delays |

### Cache Configuration Defaults

<!-- Source: src/storage/config.rs:151-173 (CacheConfig struct and Default impl) -->

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_entries` | `1000` | Maximum cached entries |
| `ttl` | `3600s` (1 hour) | Cache time-to-live |
| `cache_type` | `"memory"` | Cache implementation type |

### Environment Variable Defaults

These settings can be overridden by environment variables (see [Environment Variables](environment-variables.md)):

| Environment Variable | Corresponding Setting | Default |
|---------------------|----------------------|---------|
| `PRODIGY_CLAUDE_API_KEY` | `claude_api_key` | None |
| `PRODIGY_LOG_LEVEL` | `log_level` | `"info"` |
| `PRODIGY_EDITOR` | `default_editor` | None |
| `PRODIGY_AUTO_COMMIT` | `auto_commit` | `true` |
| `PRODIGY_STORAGE_TYPE` | `storage.backend` | `"file"` |
| `PRODIGY_STORAGE_BASE_PATH` | `storage.base_dir` | `~/.prodigy` |
| `PRODIGY_CLAUDE_STREAMING` | - | `true` |
| `PRODIGY_AUTOMATION` | - | Not set (set by Prodigy) |

### CLI Parameter Defaults

<!-- Source: src/cook/command.rs:11-81 (CookCommand struct) -->

These are CLI-level parameters, not workflow configuration fields:

| Parameter | Default Value | Description |
|-----------|---------------|-------------|
| `--max-iterations` | `1` | Number of workflow iterations to run |
| `--path` | Current directory | Repository path to run in |

### Command Metadata Defaults

<!-- Source: src/config/command.rs:132-154 (CommandMetadata struct) -->

Applied to individual commands when not specified:

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `retries` | `2` | Retry attempts for failed commands |
| `timeout` | `300` | Command timeout (seconds) |
| `continue_on_error` | `false` | Continue workflow on command failure |
| `commit_required` | `false` | Whether command must create git commits |
| `env` | `{}` | Environment variables for command |

### Understanding Defaults

!!! tip "Configuration Precedence"
    Configuration values are resolved in order from lowest to highest priority. Later sources override earlier ones.

**How defaults work:**

1. Prodigy starts with built-in defaults
2. Global config (`~/.prodigy/config.yml`) overrides defaults
3. Project config (`.prodigy/config.yml`) overrides global config
4. Environment variables override file config
5. CLI flags override everything

**Example precedence flow:**

```text title="Configuration Resolution Example"
Built-in default: log_level = "info"
       ↓
Global config: log_level = "warn"  (overrides default)
       ↓
Project config: (not specified, inherits "warn")
       ↓
Environment: PRODIGY_LOG_LEVEL=debug  (overrides all configs)
       ↓
Result: log_level = "debug"
```

### Practical Example: Overriding Storage Defaults

!!! example "Overriding Storage Defaults"
    This example shows how to override storage defaults in a project config:

    ```yaml title=".prodigy/config.yml"
    name: my-project

    storage:
      backend: file
      timeout: 60s  # Override default 30s
      enable_cache: true  # Override default false

      backend_config:
        file:
          base_dir: /custom/storage  # Override default ~/.prodigy
          max_file_size: 524288000  # 500MB (override default 100MB)
          enable_compression: true   # Override default false

      cache_config:
        max_entries: 5000  # Override default 1000
        ttl: 7200s  # 2 hours (override default 1 hour)
    ```

    **Effect of this configuration:**

    | Setting | Default | Overridden |
    |---------|---------|------------|
    | `timeout` | 30s | 60s |
    | `enable_cache` | `false` | `true` |
    | `max_file_size` | 100MB | 500MB |
    | `max_entries` | 1000 | 5000 |
    | `ttl` | 1 hour | 2 hours |
