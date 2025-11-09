## Default Values Reference

Complete reference of all default configuration values in Prodigy. These defaults are used when settings are not explicitly configured in global config, project config, or environment variables.

### Global Configuration Defaults

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

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `backend` | `"file"` | Storage backend type (`file` or `memory`) |
| `connection_pool_size` | `10` | Connection pool size (for future backends) |
| `timeout` | `30s` | Default operation timeout |
| `enable_locking` | `true` | Enable distributed locking |
| `enable_cache` | `false` | Enable caching layer |

### File Storage Defaults

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `base_dir` | `~/.prodigy` | Base storage directory |
| `use_global` | `true` | Use global storage (recommended) |
| `enable_file_locks` | `true` | Enable file-based locking |
| `max_file_size` | `104857600` (100MB) | Max file size before rotation |
| `enable_compression` | `false` | Compress archived files |

### Memory Storage Defaults

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_memory` | `104857600` (100MB) | Maximum memory usage |
| `persist_to_disk` | `false` | Persist memory storage to disk |
| `persistence_path` | None | Path for disk persistence |

### Retry Policy Defaults

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_retries` | `3` | Maximum retry attempts |
| `initial_delay` | `1s` | Initial retry delay |
| `max_delay` | `30s` | Maximum retry delay (with backoff) |
| `backoff_multiplier` | `2.0` | Exponential backoff multiplier |
| `jitter` | `true` | Add random jitter to delays |

### Cache Configuration Defaults

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

### Workflow Defaults

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `max_iterations` | None | No limit on workflow iterations |
| `timeout` | None | No timeout on workflow execution |
| `continue_on_error` | `false` | Stop workflow on command failure |

### Command Metadata Defaults

Applied to individual commands when not specified:

| Setting | Default Value | Description |
|---------|---------------|-------------|
| `retries` | `2` | Retry attempts for failed commands |
| `timeout` | `300` | Command timeout (seconds) |
| `continue_on_error` | `false` | Continue workflow on command failure |

### Understanding Defaults

**How defaults work:**

1. Prodigy starts with built-in defaults
2. Global config (`~/.prodigy/config.yml`) overrides defaults
3. Project config (`.prodigy/config.yml`) overrides global config
4. Environment variables override file config
5. CLI flags override everything

**Example precedence flow:**

```
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

### See Also

- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Global Configuration Structure](global-configuration-structure.md)
- [Project Configuration Structure](project-configuration-structure.md)
- [Storage Configuration](storage-configuration.md)
- [Environment Variables](environment-variables.md)

