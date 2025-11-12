# Global Configuration

Global configuration settings apply across all Prodigy projects and workflows. These settings are stored in your user home directory and can be overridden by project-specific configuration or environment variables.

## Configuration Structure

The global configuration is defined by the `GlobalConfig` struct, which contains system-wide settings for Prodigy's behavior, external tool integration, and operational parameters.

```rust
// Source: src/config/mod.rs:45-59
/// Global configuration settings for Prodigy
///
/// These settings apply across all projects and workflows. Can be overridden
/// by project-specific configuration. Stored in the user's home
/// directory under ~/.prodigy/config.yml.
pub struct GlobalConfig {
    pub prodigy_home: PathBuf,
    pub default_editor: Option<String>,
    pub log_level: Option<String>,
    pub claude_api_key: Option<String>,
    pub max_concurrent_specs: Option<u32>,
    pub auto_commit: Option<bool>,
    pub plugins: Option<PluginConfig>,
}
```

### Configuration Fields

| Field | Type | Description | Default |
|-------|------|-------------|---------|
| `prodigy_home` | `PathBuf` | Directory for Prodigy data, sessions, and state | `~/.prodigy` |
| `default_editor` | `Option<String>` | Default text editor for editing files | `None` (uses `$EDITOR`) |
| `log_level` | `Option<String>` | Logging verbosity level | `"info"` |
| `claude_api_key` | `Option<String>` | Claude API key for authentication | `None` |
| `max_concurrent_specs` | `Option<u32>` | Maximum number of concurrent spec executions | `1` |
| `auto_commit` | `Option<bool>` | Automatically commit changes after successful commands | `true` |
| `plugins` | `Option<PluginConfig>` | Plugin system configuration | `None` |

## File Locations

### Global Configuration

The global configuration file is stored in the Prodigy data directory:

```
~/.prodigy/config.yml
```

The global directory location is determined by platform-specific conventions:

```rust
// Source: src/config/mod.rs:26-31
pub fn get_global_prodigy_dir() -> Result<PathBuf> {
    ProjectDirs::from("com", "prodigy", "prodigy")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or_else(|| anyhow!("Could not determine home directory"))
}
```

**Platform-specific paths:**

=== "Linux"
    ```
    ~/.local/share/prodigy/
    ```

=== "macOS"
    ```
    ~/Library/Application Support/com.prodigy.prodigy/
    ```

=== "Windows"
    ```
    C:\Users\<username>\AppData\Roaming\prodigy\prodigy\
    ```

### Project Configuration

Project-specific configuration files are stored within the project directory:

```
<project-root>/.prodigy/config.yml
```

## Configuration Precedence

Prodigy uses a hierarchical configuration system where settings can be overridden at multiple levels. The precedence order from highest to lowest is:

```mermaid
graph TD
    Env[Environment Variables<br/>PRODIGY_*] --> Project[Project Config<br/>.prodigy/config.yml]
    Project --> Global[Global Config<br/>~/.prodigy/config.yml]
    Global --> Defaults[Default Values<br/>Hardcoded]

    Env -.->|Highest Priority| Resolution[Final Value]
    Project -.->|Override Global| Resolution
    Global -.->|Override Defaults| Resolution
    Defaults -.->|Lowest Priority| Resolution

    style Env fill:#e8f5e9
    style Project fill:#e1f5ff
    style Global fill:#fff3e0
    style Defaults fill:#f5f5f5
    style Resolution fill:#e8eaf6
```

**Figure**: Configuration precedence hierarchy showing how values are resolved from highest to lowest priority.

!!! note "Precedence Order"
    Configuration values are resolved in this order: **Environment Variables** → **Project Config** → **Global Config** → **Defaults**. The first defined value wins.

### Precedence Examples

**Claude API Key Resolution:**

```rust
// Source: src/config/mod.rs:133-138
pub fn get_claude_api_key(&self) -> Option<&str> {
    self.project
        .as_ref()
        .and_then(|p| p.claude_api_key.as_deref())
        .or(self.global.claude_api_key.as_deref())
}
```

1. `PRODIGY_CLAUDE_API_KEY` environment variable (highest priority)
2. `claude_api_key` in project `.prodigy/config.yml`
3. `claude_api_key` in global `~/.prodigy/config.yml`
4. `None` (no API key configured)

**Auto-Commit Behavior:**

```rust
// Source: src/config/mod.rs:140-146
pub fn get_auto_commit(&self) -> bool {
    self.project
        .as_ref()
        .and_then(|p| p.auto_commit)
        .or(self.global.auto_commit)
        .unwrap_or(true)
}
```

1. `PRODIGY_AUTO_COMMIT` environment variable
2. `auto_commit` in project config
3. `auto_commit` in global config
4. `true` (default)

## Environment Variables

Environment variables provide the highest-priority configuration mechanism, useful for CI/CD environments, temporary overrides, and secrets management.

!!! tip "When to Use Environment Variables"
    Use environment variables for:

    - **CI/CD pipelines** - Keep secrets out of version control
    - **Temporary overrides** - Test different settings without modifying config files
    - **Multi-environment deployments** - Same workflow, different configs per environment
    - **Sensitive credentials** - API keys and tokens that shouldn't be stored in files

### Supported Environment Variables

```rust
// Source: src/config/mod.rs:111-131
pub fn merge_env_vars(&mut self) {
    if let Ok(api_key) = std::env::var("PRODIGY_CLAUDE_API_KEY") {
        self.global.claude_api_key = Some(api_key);
    }

    if let Ok(log_level) = std::env::var("PRODIGY_LOG_LEVEL") {
        self.global.log_level = Some(log_level);
    }

    if let Ok(editor) = std::env::var("PRODIGY_EDITOR") {
        self.global.default_editor = Some(editor);
    } else if let Ok(editor) = std::env::var("EDITOR") {
        self.global.default_editor = Some(editor);
    }

    if let Ok(auto_commit) = std::env::var("PRODIGY_AUTO_COMMIT") {
        if let Ok(value) = auto_commit.parse::<bool>() {
            self.global.auto_commit = Some(value);
        }
    }
}
```

| Variable | Purpose | Example |
|----------|---------|---------|
| `PRODIGY_CLAUDE_API_KEY` | Override Claude API key | `sk-ant-api03-...` |
| `PRODIGY_LOG_LEVEL` | Set logging verbosity | `debug`, `info`, `warn`, `error` |
| `PRODIGY_EDITOR` | Override default editor | `vim`, `code`, `nano` |
| `EDITOR` | System default editor (fallback) | `vim`, `emacs` |
| `PRODIGY_AUTO_COMMIT` | Enable/disable auto-commit | `true`, `false` |

### Environment Variable Usage

```bash
# Temporary API key override for a single command
PRODIGY_CLAUDE_API_KEY=sk-ant-api03-xxx prodigy run workflow.yml

# Enable debug logging for troubleshooting
PRODIGY_LOG_LEVEL=debug prodigy run workflow.yml

# Disable auto-commit for manual control
PRODIGY_AUTO_COMMIT=false prodigy run workflow.yml
```

## Claude Settings

Claude integration is configured through the `claude_api_key` field, which can be set at multiple levels.

### API Key Configuration

**Global Configuration:**

```yaml
# ~/.prodigy/config.yml
claude_api_key: sk-ant-api03-xxxxxxxxxxxxx
log_level: info
```

**Project Configuration Override:**

```yaml
# .prodigy/config.yml
name: my-project
claude_api_key: sk-ant-api03-project-specific-key
```

**Environment Variable (Recommended for CI/CD):**

```bash
export PRODIGY_CLAUDE_API_KEY=sk-ant-api03-xxxxxxxxxxxxx
```

!!! warning "API Key Security"
    Never commit API keys to version control. Use environment variables or secure secret management systems for production environments.

### Best Practices for API Key Management

!!! tip "API Key Storage Strategy"
    Choose the right storage level for your use case:

    1. **Development**: Use global config file (`~/.prodigy/config.yml`) - convenient for local work
    2. **CI/CD**: Use environment variables (`PRODIGY_CLAUDE_API_KEY`) - keeps secrets out of code
    3. **Multi-tenant**: Use project-specific config for different API keys per project
    4. **Production/Sensitive**: Use secret management tools (Vault, AWS Secrets Manager, GitHub Secrets)

## Configuration Loading

Prodigy uses a multi-stage configuration loading process with clear precedence rules.

### Loading Hierarchy

```rust
// Source: src/config/loader.rs:31-55
pub async fn load_with_explicit_path(
    &self,
    project_path: &Path,
    explicit_path: Option<&Path>,
) -> Result<()> {
    match explicit_path {
        Some(path) => {
            // Load from explicit path, error if not found
            self.load_from_path(path).await?;
        }
        None => {
            // Check for .prodigy/workflow.yml
            let default_path = project_path.join(".prodigy").join("workflow.yml");
            if default_path.exists() {
                self.load_from_path(&default_path).await?;
            }
            // Otherwise use defaults (already set in new())
        }
    }
    Ok(())
}
```

**Load Order:**

!!! note "Configuration Load Sequence"
    Prodigy loads configuration in this sequence:

    1. **Explicit Path**: If `-f` or `--file` flag provided, load that exact file (error if not found)
    2. **Project Default**: Check for `.prodigy/workflow.yml` in project directory
    3. **Global Defaults**: Use default `GlobalConfig` values if no project config exists

### Configuration Format

Prodigy configuration files must use **YAML format** with the `.yml` extension.

!!! warning "YAML Only"
    Configuration files MUST use `.yml` extension. Files with `.toml` or other extensions will be rejected with a validation error.

```rust
// Source: src/config/loader.rs:65-69
let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

validate_config_format(extension)
    .with_context(|| format!("Invalid configuration file: {}", path.display()))?;
```

!!! note "TOML Deprecated"
    Earlier versions of Prodigy supported TOML format (`.toml`), but this has been deprecated in favor of YAML for consistency with workflow definitions.

!!! info "Internal Representation"
    While all Prodigy configuration files must be written in YAML format, some internal data structures (like `ProjectConfig.variables`) use `toml::Table` for historical reasons and backward compatibility. This is an implementation detail - **users should always write configuration in YAML format** with the `.yml` extension.

## Default Values

When no configuration is explicitly provided, Prodigy uses these default values:

```rust
// Source: src/config/mod.rs:88-99
impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            prodigy_home: get_global_prodigy_dir().unwrap_or_else(|_| PathBuf::from("~/.prodigy")),
            default_editor: None,
            log_level: Some("info".to_string()),
            claude_api_key: None,
            max_concurrent_specs: Some(1),
            auto_commit: Some(true),
            plugins: None,
        }
    }
}
```

| Setting | Default Value | Reason |
|---------|---------------|--------|
| `prodigy_home` | Platform-specific (see above) | Standard user data directory |
| `default_editor` | `None` | Uses `$EDITOR` or `$PRODIGY_EDITOR` |
| `log_level` | `"info"` | Balance between visibility and noise |
| `claude_api_key` | `None` | Must be explicitly configured |
| `max_concurrent_specs` | `1` | Sequential execution for safety |
| `auto_commit` | `true` | Automatic change tracking |
| `plugins` | `None` | Plugins disabled by default |

## Plugin Configuration

The plugin system allows extending Prodigy with custom commands and workflows.

### Plugin Structure

```rust
// Source: src/config/mod.rs:81-86
pub struct PluginConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub auto_load: Vec<String>,
}
```

### Enabling Plugins

```yaml
# ~/.prodigy/config.yml
plugins:
  enabled: true
  directory: ~/.prodigy/plugins
  auto_load:
    - git-helpers
    - code-review
    - custom-validators
```

**Fields:**

- `enabled`: Master switch for plugin system
- `directory`: Location where plugins are stored
- `auto_load`: List of plugins to load automatically on startup

## Complete Configuration Examples

### Minimal Global Configuration

```yaml
# ~/.prodigy/config.yml
# Minimal configuration with only API key
claude_api_key: sk-ant-api03-xxxxxxxxxxxxx
```

### Full Global Configuration

```yaml
# ~/.prodigy/config.yml
# Complete configuration with all options
prodigy_home: ~/.prodigy                        # (1)!
default_editor: code                            # (2)!
log_level: info                                 # (3)!
claude_api_key: sk-ant-api03-xxxxxxxxxxxxx      # (4)!
max_concurrent_specs: 1                         # (5)!
auto_commit: true                               # (6)!

plugins:
  enabled: true                                 # (7)!
  directory: ~/.prodigy/plugins                 # (8)!
  auto_load:                                    # (9)!
    - git-helpers
    - code-review
```

1. Directory for sessions, state, events, and DLQ data
2. Editor for `prodigy edit` commands (falls back to `$EDITOR`)
3. Logging verbosity: `debug`, `info`, `warn`, `error`
4. Claude API key for authentication (keep secret!)
5. Maximum concurrent MapReduce agents (start low, increase carefully)
6. Automatically commit changes after successful commands
7. Enable/disable the plugin system
8. Where plugin scripts are stored
9. Plugins to load automatically on startup

### Project Configuration Override

```yaml
# <project>/.prodigy/config.yml
# Project-specific overrides
name: my-project                                        # (1)!
description: Example project with custom settings       # (2)!
version: 1.0.0

# Override global API key for this project
claude_api_key: sk-ant-api03-project-specific-key      # (3)!

# Override auto-commit for manual control
auto_commit: false                                      # (4)!

# Project-specific variables
variables:                                              # (5)!
  deploy_env: staging
  api_url: https://staging.api.example.com
```

1. Project identifier used in state and event tracking
2. Human-readable description for documentation
3. Project-specific API key (overrides global config)
4. Disable auto-commit for this project (manual `git commit` control)
5. Variables available in workflows as `${variable_name}`

### Environment Variable Workflow

```bash
#!/bin/bash
# CI/CD workflow using environment variables

# Set configuration via environment
export PRODIGY_CLAUDE_API_KEY=${CLAUDE_API_KEY_SECRET}
export PRODIGY_LOG_LEVEL=debug
export PRODIGY_AUTO_COMMIT=true

# Run workflow with environment-based config
prodigy run .prodigy/workflow.yml
```

## Related Documentation

- [Project Configuration](project-configuration-structure.md) - Project-specific settings and overrides
- [Workflow Configuration](workflow-configuration.md) - Workflow definition and structure
- [Environment Variables](environment-variables.md) - Complete environment variable reference
- [Installation](../installation.md) - Initial setup and configuration
