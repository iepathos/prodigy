## Environment Profiles

Environment profiles allow you to define named sets of environment variables for different execution contexts (development, staging, production, etc.). Each profile contains environment variables that are applied when the profile is activated.

**Source**: Profile infrastructure implemented in [src/cook/environment/config.rs](../../../src/cook/environment/config.rs) (EnvironmentConfig struct) and [src/cook/environment/manager.rs](../../../src/cook/environment/manager.rs) (profile application logic).

### Defining Profiles

Profiles use a flat structure where environment variables are defined directly at the profile level (not nested under an `env:` key):

```yaml
# Define multiple profiles for different environments
profiles:
  development:
    description: "Development environment with debug enabled"
    NODE_ENV: development
    DEBUG: "true"
    API_URL: http://localhost:3000
    LOG_LEVEL: debug

  staging:
    description: "Staging environment for QA"
    NODE_ENV: staging
    DEBUG: "true"
    API_URL: https://staging.api.example.com
    LOG_LEVEL: info

  production:
    description: "Production environment configuration"
    NODE_ENV: production
    DEBUG: "false"
    API_URL: https://api.example.com
    LOG_LEVEL: error

# Global environment variables (apply to all profiles)
env:
  APP_NAME: "my-app"
  VERSION: "1.0.0"

commands:
  - shell: "npm run build"
```

**Source**: Profile structure defined in [src/cook/environment/config.rs](../../../src/cook/environment/config.rs) (EnvProfile type).

**Profile Structure Details**:
- **description** (optional): Human-readable description of the profile's purpose
- **Environment variables**: Direct key-value pairs at the profile level
- All variable values must be strings in YAML

### Activating Profiles

**Design Note**: The profile activation infrastructure is architecturally complete in the codebase. The `EnvironmentConfig` struct has an `active_profile` field ([src/cook/environment/config.rs:33-35](../../../src/cook/environment/config.rs)), and the `EnvironmentManager` applies active profiles during environment setup ([src/cook/environment/manager.rs:118-120](../../../src/cook/environment/manager.rs)). Comprehensive integration tests demonstrate profile activation ([tests/environment_workflow_test.rs:63-132](../../../tests/environment_workflow_test.rs)).

**Current Implementation Status**: As of this documentation, the CLI wiring for profile activation (`--profile` flag and `PRODIGY_PROFILE` environment variable) is not yet connected to the argument parser. The profile application infrastructure exists and is tested, but requires the active profile to be set programmatically rather than via command-line arguments.

**Intended Usage** (when CLI wiring is complete):

```bash
# Activate profile via command line flag
prodigy run workflow.yml --profile production

# Activate profile via environment variable
export PRODIGY_PROFILE=staging
prodigy run workflow.yml
```

**Current Workaround**: Profiles can be activated programmatically in tests or by directly setting the `active_profile` field when constructing `EnvironmentConfig` objects.

### Common Use Cases

Profiles are ideal for managing environment-specific configuration:

1. **Different API Endpoints**
   ```yaml
   profiles:
     development:
       API_URL: http://localhost:3000
       AUTH_URL: http://localhost:4000

     production:
       API_URL: https://api.example.com
       AUTH_URL: https://auth.example.com
   ```

2. **Environment-Specific Credentials**
   ```yaml
   profiles:
     development:
       DB_HOST: localhost
       DB_NAME: myapp_dev
       DB_USER: dev_user

     production:
       DB_HOST: prod-db.example.com
       DB_NAME: myapp_prod
       DB_USER: prod_user
   ```

3. **Deployment Target Configuration**
   ```yaml
   profiles:
     aws:
       CLOUD_PROVIDER: aws
       REGION: us-east-1
       DEPLOY_COMMAND: "aws deploy"

     gcp:
       CLOUD_PROVIDER: gcp
       REGION: us-central1
       DEPLOY_COMMAND: "gcloud deploy"
   ```

### Environment Variable Precedence

When a profile is active, environment variables are resolved in this order (highest to lowest precedence):

1. **Step-level environment** - Variables defined in individual command `env:` blocks
2. **Active profile environment** - Variables from the activated profile
3. **Global environment** - Variables from top-level `env:` block
4. **System environment** - Variables inherited from the shell

**Source**: Precedence chain implemented in [src/cook/environment/manager.rs](../../../src/cook/environment/manager.rs) and tested in [tests/environment_workflow_test.rs](../../../tests/environment_workflow_test.rs).

For detailed information on precedence rules, see [Environment Precedence](environment-precedence.md).

### Profile Best Practices

**Define sensible defaults**:
```yaml
profiles:
  development:
    description: "Local development with debug enabled"
    DEBUG: "true"
    LOG_LEVEL: debug

  production:
    description: "Production environment with minimal logging"
    DEBUG: "false"
    LOG_LEVEL: error
```

**Combine with env_files for secrets**:
```yaml
profiles:
  production:
    API_URL: https://api.example.com
    DEBUG: "false"

env_files:
  - path: .env.production
    required: true  # Contains secrets like API_KEY
```

See [Environment Files](environment-files.md) for more on combining profiles with environment files.

**Override profile values at step level**:
```yaml
profiles:
  production:
    LOG_LEVEL: error

commands:
  - shell: "run-diagnostics.sh"
    env:
      LOG_LEVEL: debug  # Override for this step only
```

See [Per-Command Environment Overrides](per-command-environment-overrides.md) for step-level overrides.

### Troubleshooting

**Profile not applied**:
- Verify profile name matches exactly (case-sensitive)
- Check that profile is defined in `profiles:` section
- Confirm profile activation method is used correctly

**Variables not resolved**:
- Ensure variable names are correct in profile definition
- Check precedence - higher-precedence sources may override profile values
- Verify string values are quoted in YAML if they contain special characters

**See Also**:
- [Environment Precedence](environment-precedence.md) - Understanding variable resolution order
- [Environment Files](environment-files.md) - Loading variables from external files
- [Best Practices](best-practices.md) - Recommended patterns for environment configuration

---

