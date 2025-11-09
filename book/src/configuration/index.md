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
