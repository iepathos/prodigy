## Related Documentation

This section provides links to related configuration and feature documentation.

### Core Configuration

- [Global Configuration Structure](global-configuration-structure.md) - System-wide settings in `~/.prodigy/config.yml`
- [Project Configuration Structure](project-configuration-structure.md) - Project settings in `.prodigy/config.yml`
- [Environment Variables](environment-variables.md) - System and workflow environment variables
- [Configuration Precedence Rules](configuration-precedence-rules.md) - How settings override each other
- [Default Values Reference](default-values-reference.md) - Built-in default values for all settings
- [Storage Configuration](storage-configuration.md) - Data storage backends and options

### Workflow Features

- [Workflow Configuration](workflow-configuration.md) - Workflow file structure and options
- [Environment Variables - Workflow Section](environment-variables.md#workflow-environment-variables) - Using `env:` blocks in workflows
- [Best Practices](best-practices.md) - Configuration and workflow best practices

### Migration and Troubleshooting

- [Migration Guide: TOML to YAML](migration-guide-toml-to-yaml.md) - Migrate from old TOML format
- [Troubleshooting](troubleshooting.md) - Common configuration issues and solutions

### Advanced Topics

- [Git Context (Advanced)](../git-context-advanced.md) - Pattern filtering and format modifiers for git variables
- [MapReduce Worktree Architecture](../mapreduce-worktree-architecture.md) - Understanding worktree isolation in MapReduce workflows

### Feature Documentation

- [MapReduce](../mapreduce/index.md) - Parallel workflow execution
- [Variables](../variables/index.md) - Variable interpolation and usage
- [Command Types](../commands.md) - Available command types (claude, shell, foreach, etc.)
- [Error Handling](../error-handling.md) - Error handling strategies and retry policies

### Reference Material

- [Complete Configuration Examples](complete-configuration-examples.md) - Real-world configuration examples
- [YAML Specification](https://yaml.org/spec/) - Official YAML documentation
