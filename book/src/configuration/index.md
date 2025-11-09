# Configuration

Prodigy supports comprehensive configuration through multiple files with a clear precedence hierarchy. This chapter explains all configuration options and how to use them effectively.

## Quick Start

Minimal project configuration (`.prodigy/config.yml`):

```yaml
name: my-project  # Required: Project identifier
```

The `name` field is the only required field. All other settings have sensible defaults.

Minimal workflow configuration (`.prodigy/workflow.yml`):

```yaml
commands:  # List of commands to execute in sequence
  - prodigy-code-review  # Review code quality
  - prodigy-lint        # Run linter checks
```

Workflows define a sequence of commands to execute. Each command is a Prodigy slash command (the `/` prefix is optional in workflow files).

That's all you need to get started! Prodigy provides sensible defaults for everything else. See the subsections below for detailed configuration options.

## Configuration File Locations

Prodigy uses a search hierarchy to find configuration files. Configuration can come from multiple sources with the following precedence (highest to lowest):

1. **CLI Flags** - Command-line arguments override all other settings
2. **Environment Variables** - Environment variables (e.g., `PRODIGY_CLAUDE_API_KEY`)
3. **Project Config** - `.prodigy/config.yml` in your project directory
4. **Global Config** - `~/.prodigy/config.yml` in your home directory
5. **Defaults** - Built-in default values

For workflow files specifically:
- Explicit path provided via `prodigy run workflow.yml`
- `.prodigy/workflow.yml` in the project directory
- Default workflow configuration


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
- [Best Practices](best-practices.md)
- [Troubleshooting](troubleshooting.md)
- [Migration Guide: TOML to YAML](migration-guide-toml-to-yaml.md)
- [Related Documentation](related-documentation.md)
