# Best Practices & Troubleshooting

This page covers best practices, common patterns, and solutions for environment variable issues in Prodigy workflows.

## Best Practices

!!! tip "Use Secrets for Sensitive Data"
    Always mark API keys, tokens, passwords, and credentials as secrets to enable automatic masking.

!!! tip "Parameterize Project-Specific Values"
    Use environment variables instead of hardcoding paths, URLs, and configuration values. This improves portability and maintainability.

!!! tip "Document Required Variables"
    Add comments in workflow files documenting expected variables and their purposes.

!!! tip "Use Profiles for Environments"
    Separate development, staging, and production configurations using profiles rather than maintaining separate workflow files.

!!! tip "Prefer Bracketed Syntax"
    Use `${VAR}` instead of `$VAR` for explicitness and to avoid ambiguity in complex expressions.

## Common Patterns

### Project Configuration

```yaml
env:
  PROJECT_NAME: "my-app"
  VERSION: "1.2.3"
  BUILD_DIR: "dist"
  RELEASE_CHANNEL: "stable"
```

### API Integration

```yaml
env:
  API_URL: "https://api.example.com"
  API_TIMEOUT: "30"

secrets:
  API_KEY: "${env:EXTERNAL_API_KEY}"

commands:
  - shell: "curl -H 'Authorization: Bearer $API_KEY' $API_URL/data"
```

### Multi-Environment Configuration

```yaml
env:
  APP_ENV: "development"
  LOG_LEVEL: "debug"

profiles:
  staging:
    APP_ENV: "staging"
    LOG_LEVEL: "info"

  production:
    APP_ENV: "production"
    LOG_LEVEL: "warn"
```

### Feature Flags

```yaml
env:
  ENABLE_CACHING: "true"
  ENABLE_ANALYTICS: "false"
  MAX_WORKERS: "4"

commands:
  - shell: |
      if [ "$ENABLE_CACHING" = "true" ]; then
        echo "Caching enabled"
      fi
```

## Troubleshooting

### Variable Not Found

**Symptom:** `$VAR` appears literally in output or command fails with "command not found"

**Cause:** Variable not defined or incorrect interpolation syntax

**Solution:**

1. Verify variable is defined in `env` block or environment file
2. Check spelling and case (variable names are case-sensitive)
3. Ensure proper interpolation syntax (`$VAR` or `${VAR}`)
4. Use `--profile` flag if variable is profile-specific

### Secret Not Masked

**Symptom:** Sensitive value appears in logs or output

**Cause:** Variable not marked as secret

**Solution:**

```yaml
# Before (not masked)
env:
  API_KEY: "sk-abc123"

# After (masked)
env:
  API_KEY:
    secret: true
    value: "sk-abc123"
```

### Profile Variables Not Applied

**Symptom:** Default values used instead of profile values

**Cause:** Profile not activated with `--profile` flag

**Solution:**

```bash
# Activate profile
prodigy run workflow.yml --profile production
```

### Environment File Not Loaded

**Symptom:** Variables from `.env` file not available

**Cause:** File path incorrect or file doesn't exist

**Solution:**

1. Verify file path is relative to workflow file location
2. Check file exists: `ls .env.production`
3. Verify file syntax (KEY=VALUE format, no spaces around `=`)
