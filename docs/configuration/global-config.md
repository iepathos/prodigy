# Global Configuration

Prodigy's global configuration manages system-wide settings that apply across all projects. These settings can be overridden by project-specific configuration or environment variables.

## Configuration Structure

Global configuration is stored in `~/.prodigy/config.yml` and follows this structure:

```yaml
# Source: src/config/mod.rs:50-59
prodigy_home: ~/.prodigy
default_editor: vim
log_level: info
claude_api_key: your-api-key-here
max_concurrent_specs: 1
auto_commit: true
plugins:
  enabled: false
  directory: ~/.prodigy/plugins
  auto_load: []
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prodigy_home` | Path | `~/.prodigy` | Global Prodigy directory for storing data and configuration |
| `default_editor` | String | None | Default text editor for opening files |
| `log_level` | String | `"info"` | Logging verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `claude_api_key` | String | None | Claude API key for AI-powered features |
| `max_concurrent_specs` | Integer | `1` | Maximum number of specifications to process concurrently |
| `auto_commit` | Boolean | `true` | Automatically commit changes after successful operations |
| `plugins` | Object | None | Plugin system configuration (see below) |

## File Locations

Prodigy searches for configuration files in the following locations:

```yaml
# Source: src/config/loader.rs:46-50, src/config/mod.rs:26-31
# 1. Global configuration
~/.prodigy/config.yml

# 2. Project configuration
<project-root>/.prodigy/config.yml

# 3. Workflow configuration
<project-root>/.prodigy/workflow.yml
```

### Directory Structure

The global Prodigy directory (`~/.prodigy/`) contains:

```
~/.prodigy/
├── config.yml              # Global configuration
├── events/                 # MapReduce event logs
│   └── {repo_name}/
├── dlq/                    # Dead Letter Queue for failed items
│   └── {repo_name}/
├── state/                  # Session and job state
│   └── {repo_name}/
├── worktrees/              # Git worktrees for sessions
│   └── {repo_name}/
├── sessions/               # Session metadata
└── resume_locks/           # Resume operation locks
```

## Configuration Precedence

Prodigy applies configuration settings in the following order (later sources override earlier ones):

```
Global Config → Project Config → Environment Variables
```

### Precedence Hierarchy

```yaml
# Source: src/config/mod.rs:111-131, 133-154
# 1. Global configuration (~/.prodigy/config.yml)
log_level: info
auto_commit: true

# 2. Project configuration (.prodigy/config.yml) - overrides global
auto_commit: false

# 3. Environment variables - highest precedence
PRODIGY_AUTO_COMMIT=true  # This wins
```

### Merge Behavior

Configuration merging follows these rules:

- **Scalar values**: Later values completely replace earlier ones
- **Project overrides**: Project config fields override global config fields
- **Environment variables**: Always take precedence over file-based config
- **Null/None values**: Treated as "not set", allowing fallback to lower precedence

## Claude Settings

Configure Claude API access at global, project, or environment level.

### API Key Configuration

!!! warning "Security Best Practice"
    Never commit API keys to version control. Use environment variables or global configuration only.

**Option 1: Global Configuration**

```yaml
# Source: src/config/mod.rs:55
# ~/.prodigy/config.yml
claude_api_key: sk-ant-api03-...
```

**Option 2: Project Configuration**

```yaml
# Source: src/config/mod.rs:71
# .prodigy/config.yml
name: my-project
claude_api_key: sk-ant-api03-...  # Project-specific key
```

**Option 3: Environment Variable (Recommended)**

```bash
# Source: src/config/mod.rs:112-113
export PRODIGY_CLAUDE_API_KEY=sk-ant-api03-...
```

### API Key Precedence

```yaml
# Source: src/config/mod.rs:133-138
# Precedence order (highest to lowest):
# 1. Environment variable: PRODIGY_CLAUDE_API_KEY
# 2. Project config: .prodigy/config.yml → claude_api_key
# 3. Global config: ~/.prodigy/config.yml → claude_api_key
```

## Environment Variables

Prodigy supports environment variable overrides for all major configuration settings.

### Supported Environment Variables

| Variable | Maps To | Example | Description |
|----------|---------|---------|-------------|
| `PRODIGY_CLAUDE_API_KEY` | `claude_api_key` | `sk-ant-api03-...` | Claude API key |
| `PRODIGY_LOG_LEVEL` | `log_level` | `debug` | Logging verbosity |
| `PRODIGY_EDITOR` | `default_editor` | `vim` | Preferred text editor |
| `EDITOR` | `default_editor` | `nano` | Fallback editor (if `PRODIGY_EDITOR` not set) |
| `PRODIGY_AUTO_COMMIT` | `auto_commit` | `false` | Auto-commit behavior |

### Environment Variable Usage

```bash
# Source: src/config/mod.rs:111-131
# Set Claude API key
export PRODIGY_CLAUDE_API_KEY=sk-ant-api03-...

# Set log level to debug
export PRODIGY_LOG_LEVEL=debug

# Use vim as editor
export PRODIGY_EDITOR=vim

# Disable auto-commit
export PRODIGY_AUTO_COMMIT=false
```

### Editor Fallback Behavior

Prodigy checks for editor configuration in this order:

```bash
# Source: src/config/mod.rs:120-124
# 1. PRODIGY_EDITOR (highest priority)
export PRODIGY_EDITOR=emacs

# 2. EDITOR (fallback)
export EDITOR=nano

# 3. None (no default)
```

## Configuration Loading

Prodigy loads configuration using a hierarchical search process:

```yaml
# Source: src/config/loader.rs:31-55
# 1. Check for explicit path (command-line argument)
prodigy run --config /path/to/workflow.yml

# 2. Check for default workflow file
.prodigy/workflow.yml

# 3. Use default configuration
```

### Loading Process

1. **Initialize**: Create default `Config` with `GlobalConfig::default()`
2. **Load Global**: Read `~/.prodigy/config.yml` if it exists
3. **Load Project**: Read `.prodigy/config.yml` in project directory
4. **Merge Environment**: Apply environment variable overrides
5. **Validate**: Ensure configuration is valid and complete

## Plugin Configuration

Configure the plugin system to extend Prodigy's functionality.

### Plugin Structure

```yaml
# Source: src/config/mod.rs:81-86
plugins:
  enabled: true                    # Enable/disable plugin system
  directory: ~/.prodigy/plugins    # Plugin installation directory
  auto_load:                       # Plugins to load automatically
    - prodigy-git-hooks
    - prodigy-slack-notifier
```

### Plugin Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable or disable the plugin system |
| `directory` | Path | `~/.prodigy/plugins` | Directory containing plugin files |
| `auto_load` | Array | `[]` | List of plugin names to load on startup |

## Default Values

When no configuration is provided, Prodigy uses these defaults:

```rust
// Source: src/config/mod.rs:88-99
GlobalConfig {
    prodigy_home: ~/.prodigy,              // Platform-specific data directory
    default_editor: None,                  // No default editor
    log_level: Some("info"),               // Info-level logging
    claude_api_key: None,                  // No API key
    max_concurrent_specs: Some(1),         // Sequential processing
    auto_commit: Some(true),               // Auto-commit enabled
    plugins: None,                         // Plugins disabled
}
```

## Configuration Format

Prodigy requires YAML format (`.yml` extension) for all configuration files.

!!! note "Format Requirements"
    - **Supported**: YAML with `.yml` extension
    - **Deprecated**: TOML format (`.toml`) is no longer supported
    - **Validation**: Files are validated on load

```yaml
# Source: src/config/loader.rs:65-69
# Valid configuration file extensions:
# ✓ config.yml
# ✓ workflow.yml
# ✗ config.toml (deprecated)
# ✗ config.json (not supported)
```

## Examples

### Minimal Global Configuration

```yaml
# Source: src/config/loader.rs:238-259 (test examples)
# ~/.prodigy/config.yml
log_level: info
auto_commit: true
```

### Full Global Configuration

```yaml
# ~/.prodigy/config.yml - Complete example
prodigy_home: ~/.prodigy
default_editor: vim
log_level: debug
claude_api_key: sk-ant-api03-...
max_concurrent_specs: 3
auto_commit: true
plugins:
  enabled: true
  directory: ~/.prodigy/plugins
  auto_load:
    - prodigy-git-hooks
    - prodigy-notifications
```

### Project Configuration Override

```yaml
# .prodigy/config.yml - Project-specific settings
name: my-project
description: Example project configuration
version: 1.0.0
spec_dir: custom-specs
claude_api_key: sk-ant-api03-project-specific-key
auto_commit: false  # Override global setting
```

### Environment Variable Configuration

```bash
#!/bin/bash
# Use environment variables for sensitive data

# API key (recommended for security)
export PRODIGY_CLAUDE_API_KEY=sk-ant-api03-...

# Development settings
export PRODIGY_LOG_LEVEL=debug
export PRODIGY_EDITOR=code
export PRODIGY_AUTO_COMMIT=false

# Run Prodigy with env vars
prodigy run workflow.yml
```

## Related Documentation

- [Project Configuration](project-config.md) - Project-specific settings
- [Workflow Configuration](workflow-config.md) - Workflow definitions
- [Environment Variables](environment-variables.md) - Complete environment variable reference
