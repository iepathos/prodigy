# Configuration

Prodigy supports comprehensive configuration through multiple files with a clear precedence hierarchy. This chapter explains all configuration options and how to use them effectively.

## Quick Start

Minimal project configuration (`.prodigy/config.yml`):

```yaml
name: my-project
```

Minimal workflow configuration (`.prodigy/workflow.yml`):

```yaml
commands:
  - prodigy-code-review
  - prodigy-lint
```

That's all you need to get started! Prodigy provides sensible defaults for everything else.

## Configuration File Locations

Prodigy uses a search hierarchy to find configuration files:

### Workflow Configuration

1. **Explicit path via CLI** (highest priority):
   ```bash
   prodigy run custom-workflow.yml
   ```

2. **Project workflow file**:
   ```
   .prodigy/workflow.yml
   ```

3. **Default configuration** (if no file found):
   - Uses built-in defaults
   - Minimal workflow with standard commands

### Project Configuration

Project settings are loaded from:
```
.prodigy/config.yml
```

### Global Configuration

Global user settings are stored in:
```
~/.prodigy/config.yml
```

### Format Support

**Supported formats:**
- YAML (`.yml`, `.yaml`) - **Recommended and required format**

**Unsupported formats:**
- TOML (`.toml`) - No longer supported. Prodigy will reject TOML files with an error during validation. See the Migration Guide below for converting to YAML.
- JSON (`.json`) - Not supported

All configuration files must use YAML format.

## Configuration Precedence Rules

Prodigy merges configuration from multiple sources with clear precedence:

### Overall Precedence Hierarchy

1. **Environment variables** (highest priority)
2. **Project configuration** (`.prodigy/config.yml`)
3. **Global configuration** (`~/.prodigy/config.yml`)
4. **Default values** (lowest priority)

### Specific Settings Precedence

#### Claude API Key
```
Project config > Global config (with env var overrides) > Defaults
```

**Important:** Environment variables are merged into global config via the `merge_env_vars()` function before the project config check, so the actual evaluation order is:

1. **Project config** (highest priority) - Explicit project-level API key
2. **Global config with environment variable overrides** - Environment variables override values in `~/.prodigy/config.yml`
3. **Defaults** (lowest priority)

Example:
```yaml
# .prodigy/config.yml (highest priority - takes precedence over everything)
claude_api_key: "sk-project-key"
```

```yaml
# ~/.prodigy/config.yml (can be overridden by env vars)
claude_api_key: "sk-global-key"
```

```bash
# Environment variable (overrides global config, but not project config)
export PRODIGY_CLAUDE_API_KEY="sk-env-key"
```

If both `~/.prodigy/config.yml` has `claude_api_key: "sk-global-key"` and `PRODIGY_CLAUDE_API_KEY="sk-env-key"` is set, the environment variable wins for the merged global config. However, if `.prodigy/config.yml` has a `claude_api_key`, it overrides both.

#### Auto-Commit Setting
```
Project config > Global config > Default (true)
```

#### Log Level
```
Environment variable > Global config > Default (info)
```

Example:
```bash
# Override log level via environment
export PRODIGY_LOG_LEVEL="debug"
prodigy run workflow.yml
```

## Global Configuration Structure

Global configuration is stored in `~/.prodigy/config.yml` and applies to all projects.

### GlobalConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prodigy_home` | Path | `~/.prodigy` | Base directory for Prodigy data |
| `default_editor` | String | None | Editor for interactive operations |
| `log_level` | String | `"info"` | Logging verbosity (`info`, `debug`, `trace`) |
| `claude_api_key` | String | None | API key for Claude integration |
| `max_concurrent_specs` | Number | `1` | Parallel execution limit |
| `auto_commit` | Boolean | `true` | Automatic git commits |
| `plugins` | Object | None | Plugin system configuration |

### Example Global Configuration

```yaml
# ~/.prodigy/config.yml
prodigy_home: /Users/username/.prodigy
default_editor: vim
log_level: info
claude_api_key: sk-ant-your-key-here
max_concurrent_specs: 1
auto_commit: true
```

## Project Configuration Structure

Project configuration is stored in `.prodigy/config.yml` and overrides global settings for a specific project.

### ProjectConfig Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | String | **Yes** | - | Project name |
| `description` | String | No | None | Project description |
| `version` | String | No | None | Project version |
| `spec_dir` | Path | No | `"specs"` | Directory for specification files |
| `claude_api_key` | String | No | None | Project-specific API key |
| `auto_commit` | Boolean | No | None | Project-specific auto-commit setting |
| `variables` | Map | No | None | Custom project variables as YAML map |

### Example Project Configuration

**Minimal:**
```yaml
# .prodigy/config.yml
name: my-project
```

**Complete:**
```yaml
# .prodigy/config.yml
name: my-awesome-project
description: A fantastic project using Prodigy
version: 2.1.0
spec_dir: custom-specs
claude_api_key: sk-ant-project-key
auto_commit: false

variables:
  PROJECT_ROOT: /path/to/project
  API_URL: https://api.example.com
```

## Workflow Configuration

Workflow configuration defines commands to execute and their environment.

### WorkflowConfig Structure

| Field | Type | Description |
|-------|------|-------------|
| `commands` | Array | List of commands to execute (required) |
| `env` | Object | Global environment variables |
| `secrets` | Object | Secret environment variables (masked in logs) |
| `env_files` | Array | List of `.env` files to load |
| `profiles` | Object | Named environment profiles |
| `merge` | Object | Custom merge workflow configuration |

### Basic Workflow Examples

**Simple command list:**
```yaml
commands:
  - prodigy-code-review
  - prodigy-implement-spec
  - prodigy-lint
```

**With environment variables:**
```yaml
env:
  PROJECT_NAME: prodigy
  VERSION: 1.0.0

commands:
  - prodigy-build
  - prodigy-test
```

**With secrets (masked in logs):**
```yaml
env:
  PROJECT_NAME: prodigy

secrets:
  API_KEY:
    secret: true
    value: sk-secret-key-here

commands:
  - prodigy-deploy
```

**With profiles (environment-specific):**
```yaml
env:
  PROJECT_NAME: prodigy

  API_URL:
    default: http://localhost:3000
    staging: https://staging.api.com
    prod: https://api.com

commands:
  - prodigy-deploy
```

Run with a profile:
```bash
prodigy run workflow.yml --profile prod
```

**With custom merge workflow:**
```yaml
commands:
  - prodigy-implement-feature

merge:
  commands:
    - shell: "cargo test"
    - shell: "cargo clippy"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 600
```

## Storage Configuration

Storage configuration controls where and how Prodigy stores data (events, state, DLQ).

### StorageConfig Structure

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `backend` | String | `"file"` | Storage backend type (`file`, `memory`) |
| `connection_pool_size` | Number | `10` | Pool size for database backends |
| `retry_policy` | Object | See below | Retry configuration |
| `timeout` | Duration | `30s` | Default operation timeout |
| `backend_config` | Object | See below | Backend-specific settings |
| `enable_locking` | Boolean | `true` | Distributed locking |
| `enable_cache` | Boolean | `false` | Caching layer |
| `cache_config` | Object | See below | Cache configuration |

### File Storage Configuration

The file backend stores data in the filesystem (recommended for production).

#### FileConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base_dir` | Path | `~/.prodigy` | Base directory for storage |
| `use_global` | Boolean | `true` | Use global storage (local storage deprecated) |
| `enable_file_locks` | Boolean | `true` | File-based locking |
| `max_file_size` | Number | `104857600` | Max file size before rotation (100MB, same as MemoryConfig.max_memory default) |
| `enable_compression` | Boolean | `false` | Compression for archived files |

#### Example File Storage Configuration

```yaml
storage:
  backend: file
  backend_config:
    base_dir: /custom/prodigy/data
    use_global: true
    enable_file_locks: true
    max_file_size: 104857600  # 100MB
    enable_compression: false
```

### Memory Storage Configuration

The memory backend stores data in RAM (useful for testing).

#### MemoryConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_memory` | Number | `104857600` | Maximum memory usage (100MB) |
| `persist_to_disk` | Boolean | `false` | Enable persistence to disk |
| `persistence_path` | Path | None | Persistence file path |

#### Example Memory Storage Configuration

```yaml
storage:
  backend: memory
  backend_config:
    max_memory: 104857600  # 100MB
    persist_to_disk: false
```

### Retry Policy Configuration

Controls retry behavior for failed storage operations.

#### RetryPolicy Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_retries` | Number | `3` | Maximum retry attempts |
| `initial_delay` | Duration | `1s` | Initial retry delay |
| `max_delay` | Duration | `30s` | Maximum retry delay |
| `backoff_multiplier` | Number | `2.0` | Exponential backoff multiplier |
| `jitter` | Boolean | `true` | Enable jitter |

#### Example Retry Policy

```yaml
storage:
  retry_policy:
    max_retries: 5
    initial_delay: 1s
    max_delay: 60s
    backoff_multiplier: 2.0
    jitter: true
```

### Cache Configuration

Optional caching layer for improved performance.

#### CacheConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_entries` | Number | `1000` | Cache size limit (entries) |
| `ttl` | Duration | `3600s` | Cache TTL (1 hour) |
| `cache_type` | String | `"memory"` | Cache implementation |

#### Example Cache Configuration

```yaml
storage:
  enable_cache: true
  cache_config:
    max_entries: 5000
    ttl: 600s  # 10 minutes
    cache_type: memory
```

## Environment Variables

Prodigy recognizes the following environment variables:

### Core Environment Variables

| Variable | Type | Description |
|----------|------|-------------|
| `PRODIGY_CLAUDE_API_KEY` | String | Override Claude API key |
| `PRODIGY_LOG_LEVEL` | String | Override log level (`info`, `debug`, `trace`) |
| `PRODIGY_EDITOR` | String | Override default editor |
| `EDITOR` | String | Fallback editor variable |
| `PRODIGY_AUTO_COMMIT` | Boolean | Override auto-commit setting (`true`, `false`) |

### Storage Environment Variables

| Variable | Type | Description |
|----------|------|-------------|
| `PRODIGY_STORAGE_TYPE` | String | Storage backend type (`file`, `memory`) |
| `PRODIGY_STORAGE_BASE_PATH` | Path | Custom storage directory (recommended) |
| `PRODIGY_STORAGE_DIR` | Path | Alternative storage directory variable (fallback) |
| `PRODIGY_STORAGE_PATH` | Path | Alternative storage directory variable (fallback) |

**Note:** Multiple environment variable names are supported for the storage base path. They are checked in the following order:
1. `PRODIGY_STORAGE_BASE_PATH` (recommended)
2. `PRODIGY_STORAGE_DIR` (fallback)
3. `PRODIGY_STORAGE_PATH` (fallback)

Use `PRODIGY_STORAGE_BASE_PATH` for consistency with the config file field name.

### Automation Environment Variables

These variables are set automatically by Prodigy during execution:

| Variable | Value | Description |
|----------|-------|-------------|
| `PRODIGY_AUTOMATION` | `"true"` | Signals automated execution mode |
| `PRODIGY_CLAUDE_STREAMING` | `"true"` or `"false"` | Automatically set by Prodigy based on verbosity level (`-v` flag). Can be manually set to `"false"` to disable JSON streaming in CI/CD environments with storage constraints. |

### Examples

**Override log level:**
```bash
export PRODIGY_LOG_LEVEL=debug
prodigy run workflow.yml
```

**Set API key:**
```bash
export PRODIGY_CLAUDE_API_KEY=sk-ant-your-key
prodigy run workflow.yml
```

**Disable auto-commit:**
```bash
export PRODIGY_AUTO_COMMIT=false
prodigy run workflow.yml
```

**Custom storage location:**
```bash
export PRODIGY_STORAGE_BASE_PATH=/custom/storage
prodigy run workflow.yml
```

## Complete Configuration Examples

### Minimal Setup

**Project config (`.prodigy/config.yml`):**
```yaml
name: my-project
```

**Workflow (`.prodigy/workflow.yml`):**
```yaml
commands:
  - prodigy-code-review
  - prodigy-lint
```

### Full-Featured Setup

**Global config (`~/.prodigy/config.yml`):**
```yaml
prodigy_home: /Users/username/.prodigy
default_editor: vim
log_level: info
claude_api_key: sk-ant-global-key
max_concurrent_specs: 1
auto_commit: true
```

**Project config (`.prodigy/config.yml`):**
```yaml
name: production-app
description: Our production application
version: 2.0.0
spec_dir: specifications
auto_commit: false

variables:
  PROJECT_ROOT: /app
  ENV: production
```

**Workflow with environments (`.prodigy/workflow.yml`):**
```yaml
env:
  PROJECT_NAME: production-app
  VERSION: 2.0.0

  DATABASE_URL:
    default: postgres://localhost/dev
    staging: postgres://staging-db/app
    prod: postgres://prod-db/app

secrets:
  DB_PASSWORD:
    secret: true
    value: super-secret

commands:
  - name: prodigy-code-review
    options:
      focus: security
  - shell: "cargo build --release"
  - shell: "cargo test --all"
  - prodigy-deploy

merge:
  commands:
    - shell: "cargo test"
    - shell: "cargo clippy -- -D warnings"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 600
```

## Default Values Reference

Quick reference of all default values:

| Setting | Default Value |
|---------|--------------|
| `log_level` | `"info"` |
| `max_concurrent_specs` | `1` |
| `auto_commit` | `true` |
| `spec_dir` | `"specs"` |
| `storage.backend` | `"file"` |
| `storage.use_global` | `true` |
| `storage.enable_locking` | `true` |
| `storage.enable_cache` | `false` |
| `storage.connection_pool_size` | `10` |
| `storage.timeout` | `30s` |
| `storage.retry_policy.max_retries` | `3` |
| `storage.retry_policy.initial_delay` | `1s` |
| `storage.retry_policy.max_delay` | `30s` |
| `storage.retry_policy.backoff_multiplier` | `2.0` |
| `storage.file.max_file_size` | `104857600` (100MB) |
| `storage.cache.max_entries` | `1000` |
| `storage.cache.ttl` | `3600s` (1 hour) |

## Best Practices

### Managing API Keys Securely

**DO:**
- Store API keys in global config (`~/.prodigy/config.yml`)
- Use environment variables for CI/CD environments
- Use workflow `secrets` for sensitive values
- Add `.prodigy/config.yml` to `.gitignore` if it contains secrets

**DON'T:**
- Commit API keys to version control
- Share project configs containing secrets
- Log or print secret values

### When to Use Each Configuration Level

**Global config (`~/.prodigy/config.yml`):**
- Personal preferences (editor, log level)
- Default Claude API key
- System-wide settings

**Project config (`.prodigy/config.yml`):**
- Project name and metadata
- Project-specific spec directory
- Project-specific API key (if needed)
- Custom variables shared by team

**Workflow config (`.prodigy/workflow.yml`):**
- Command sequences
- Environment variables for commands
- Environment-specific settings (profiles)
- Workflow-level secrets

**Environment variables:**
- CI/CD overrides
- Temporary testing configurations
- Dynamic runtime values

### Environment-Specific Configurations

Use profiles for different environments:

```yaml
env:
  # Common variables
  PROJECT_NAME: my-app

  # Environment-specific
  API_URL:
    default: http://localhost:3000
    dev: http://localhost:3000
    staging: https://staging.api.com
    prod: https://api.com

  LOG_LEVEL:
    default: debug
    prod: info

commands:
  - prodigy-deploy
```

Run for specific environment:
```bash
prodigy run workflow.yml --profile prod
```

## Troubleshooting

### Common Configuration Errors

**Error: "Unsupported configuration file format"**
- **Cause:** Using TOML or JSON instead of YAML
- **Solution:** Convert your config to YAML format (`.yml` or `.yaml`)

**Error: "Failed to read configuration file"**
- **Cause:** File doesn't exist or permissions issue
- **Solution:** Check file path and permissions

**Error: "Failed to parse configuration"**
- **Cause:** Invalid YAML syntax
- **Solution:** Validate YAML syntax (use `yamllint` or online validator)

**Error: "Claude API key not found"**
- **Cause:** No API key configured
- **Solution:** Set `claude_api_key` in global or project config, or use `PRODIGY_CLAUDE_API_KEY` environment variable

### Validating Configuration

Check your configuration syntax:
```bash
# Validate YAML syntax
yamllint .prodigy/workflow.yml

# Test configuration loading
prodigy run workflow.yml --dry-run
```

### Configuration Precedence Debugging

To see which configuration values are being used:
```bash
# Enable debug logging
export PRODIGY_LOG_LEVEL=debug
prodigy run workflow.yml -v
```

This will show which config files are loaded and how values are merged.

## Migration Guide: TOML to YAML

If you're upgrading from an older version that used TOML:

**Old TOML format (`.prodigy/config.toml`):**
```toml
name = "my-project"
description = "My project"

[variables]
PROJECT_ROOT = "/app"
```

**New YAML format (`.prodigy/config.yml`):**
```yaml
name: my-project
description: My project

variables:
  PROJECT_ROOT: /app
```

**Key differences:**
- Use `:` instead of `=` for assignments
- Indentation matters (use 2 spaces)
- No need for `[section]` headers (use nested structure)
- Strings usually don't need quotes (unless they contain special characters)

## Related Documentation

- [Workflow System](./workflows.md) - Learn about workflow execution
- [MapReduce](./mapreduce.md) - Configure MapReduce workflows
- [CLI Reference](./cli-reference.md) - Command-line options
- [Storage System](./storage.md) - Understanding storage backends
