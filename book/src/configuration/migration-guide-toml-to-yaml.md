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

