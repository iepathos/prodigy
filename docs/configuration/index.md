# Configuration

Prodigy supports comprehensive configuration through multiple files with a clear precedence hierarchy. This chapter explains all configuration options and how to use them effectively.

## Quick Start

Prodigy uses two distinct types of configuration files:

1. **Project Configuration** (`.prodigy/config.yml`) - Project settings and metadata
2. **Workflow Configuration** (`.prodigy/workflow.yml` or explicit path) - Workflow definitions

### Minimal Project Configuration

Create `.prodigy/config.yml`:

```yaml
name: my-project  # Required: Project identifier
```

The `name` field is the only required field. All other settings have sensible defaults.

**Source**: ProjectConfig struct in src/config/mod.rs:66-74

### Minimal Workflow Configuration

Create `.prodigy/workflow.yml` or any `.yml` file:

```yaml
commands:  # List of commands to execute in sequence
  - prodigy-code-review  # Slash prefix is optional in workflow files
  - /prodigy-lint        # Both styles work the same
```

Workflows define a sequence of commands to execute. Each command is a Prodigy slash command - the `/` prefix is optional in workflow files.

**Source**: WorkflowConfig parsing in src/core/config/mod.rs:11-22, Examples in examples/mapreduce-json-input.yml

That's all you need to get started! Prodigy provides sensible defaults for everything else. See the subsections below for detailed configuration options.

## Configuration File Locations

Prodigy uses a search hierarchy to find configuration files. Configuration can come from multiple sources with the following precedence (highest to lowest):

1. **CLI Flags** - Command-line arguments override all other settings
2. **Environment Variables** - Environment variables (e.g., `PRODIGY_CLAUDE_API_KEY`)
3. **Project Config** - `.prodigy/config.yml` in your project directory
4. **Global Config** - `~/.prodigy/config.yml` in your home directory
5. **Defaults** - Built-in default values

**Source**: ConfigLoader.load_with_explicit_path() in src/config/loader.rs:31-55

### Workflow File Search Hierarchy

For workflow files specifically (different from project config):

1. **Explicit path** - Path provided via `prodigy run path/to/workflow.yml` (error if not found)
2. **Default location** - `.prodigy/workflow.yml` in the project directory (if exists)
3. **Built-in defaults** - Default workflow configuration

**Source**: ConfigLoader.load_with_explicit_path() in src/config/loader.rs:40-54

### Key Distinction: config.yml vs workflow.yml

- **`.prodigy/config.yml`**: Contains **project settings** (name, version, API keys, editor preferences)
  - Maps to `ProjectConfig` struct (src/config/mod.rs:66-74)
  - Loaded via ConfigLoader.load_project() (src/config/loader.rs:85-104)

- **`.prodigy/workflow.yml`**: Contains **workflow definitions** (commands to execute)
  - Maps to `WorkflowConfig` struct (src/config/workflow.rs)
  - Loaded via ConfigLoader.load_with_explicit_path() (src/config/loader.rs:35-55)

Both can exist in the `.prodigy/` directory and serve different purposes.

## Configuration Architecture

Prodigy uses a three-tier configuration structure internally:

```
Config (Root)
├── GlobalConfig      - User-wide settings (~/.prodigy/config.yml)
├── ProjectConfig     - Project-specific settings (.prodigy/config.yml)
└── WorkflowConfig    - Workflow definitions (.prodigy/workflow.yml or explicit path)
```

**Source**: Config struct hierarchy in src/config/mod.rs:38-43

**How it works:**
1. `Config::new()` creates the root with default `GlobalConfig`
2. `merge_project_config()` adds project-specific settings (src/core/config/mod.rs:36-40)
3. `merge_workflow_config()` adds workflow definitions (src/core/config/mod.rs:31-34)
4. `Config.merge_env_vars()` applies environment variable overrides (src/config/mod.rs:111-131)

This design allows:
- **Separation of concerns**: Global settings vs project settings vs workflows
- **Clear precedence**: Project settings override global defaults
- **Environment overrides**: Runtime configuration via env vars
- **Type safety**: Each config tier has its own validated struct

See [Global Configuration Structure](global-configuration-structure.md) and [Project Configuration Structure](project-configuration-structure.md) for detailed field definitions.

## Common Configuration Patterns

### Quick Validation

To verify which configuration Prodigy is using:

```bash
# Check if config files exist
ls -la .prodigy/config.yml .prodigy/workflow.yml

# Validate YAML syntax
prodigy validate workflow.yml

# Run with verbose output to see loaded configuration
prodigy run workflow.yml -v
```

**Validation rules** (src/core/config/mod.rs:43-50):
- Only `.yml` and `.yaml` extensions are supported (TOML is deprecated)
- Config files must be valid YAML syntax
- ProjectConfig requires `name` field
- WorkflowConfig requires `commands` array

### Configuration Not Found?

If Prodigy doesn't find your configuration:

1. **Workflow file**: Check explicit path vs `.prodigy/workflow.yml`
   - Explicit path: `prodigy run path/to/workflow.yml` (must exist or error)
   - Default location: `.prodigy/workflow.yml` (optional, uses defaults if missing)

2. **Project config**: Must be at `.prodigy/config.yml` in project root
   - Prodigy searches upward from current directory for `.prodigy/` folder
   - Check you're running from within the project directory

3. **Global config**: Optional, located at `~/.prodigy/config.yml`
   - Use for API keys and editor preferences across all projects

**Source**: ConfigLoader search logic in src/config/loader.rs:35-104

### Debugging Configuration Issues

Common issues and solutions:

| Issue | Cause | Solution |
|-------|-------|----------|
| "Config not found" | File in wrong location | Check `.prodigy/config.yml` exists in project root |
| "Invalid YAML" | Syntax error | Validate YAML with online parser or `yamllint` |
| "Unknown field" | Typo in field name | Check struct definitions in src/config/mod.rs |
| Settings not applied | Wrong precedence | CLI flags > env vars > project > global > defaults |
| Workflow not loaded | Wrong file used | Verify workflow.yml vs config.yml distinction |

## Additional Topics

See also:
- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Global Configuration Structure](global-configuration-structure.md)
- [Project Configuration Structure](project-configuration-structure.md)
- [Workflow Configuration](workflow-configuration.md)
- [Storage Configuration](storage-configuration.md)
- [Environment Variables](environment-variables.md)
- [Complete Configuration Examples](complete-configuration-examples.md)
- [Default Values Reference](default-values-reference.md)
- [Troubleshooting](troubleshooting.md)
