## Secrets Management

Prodigy provides secure secret management through the `secrets` field in `EnvironmentConfig`. Secrets are environment variables that are masked in logs and output for security.

### Configuration Structure

Secrets are defined in the workflow-level `secrets:` block using the `SecretValue` type, which supports two variants:

**Source**: `src/cook/environment/config.rs:86-96`

=== "Simple Reference"

    ```yaml
    # Simple secret reference (SecretValue::Simple)
    secrets:
      API_KEY: "${env:SECRET_API_KEY}"
    ```

=== "Provider-Based"

    ```yaml
    # Provider-based secrets (SecretValue::Provider)
    secrets:
      DATABASE_URL:
        provider: env
        key: "DB_CONNECTION_STRING"

      SSH_KEY:
        provider: file
        key: "~/.ssh/deploy_key"
        version: "v1"  # Optional version field
    ```

### SecretValue Types

The `SecretValue` enum has two variants defined in `src/cook/environment/config.rs:86-96`:

#### 1. Simple String Reference

The `Simple` variant allows direct string references, typically using environment variable interpolation:

```yaml
secrets:
  # Reference environment variable directly
  API_TOKEN: "${env:GITHUB_TOKEN}"

  # Reference another env var with fallback
  DATABASE_PASS: "${env:DB_PASSWORD}"
```

**Resolution**: Simple values are resolved by looking up the referenced environment variable name (`src/cook/environment/manager.rs:318-320`).

#### 2. Provider-Based Secrets

The `Provider` variant supports structured secret resolution with different providers:

```yaml
secrets:
  # Environment variable provider
  API_KEY:
    provider: env
    key: "SECRET_API_KEY"

  # File-based secret
  SSH_PRIVATE_KEY:
    provider: file
    key: "/etc/secrets/ssh_key"

  # Custom provider (extensible)
  CUSTOM_SECRET:
    provider: custom
    key: "my-secret-id"
```

**Source**: Example from `workflows/mapreduce-env-example.yml:23-26`

### Supported Secret Providers

Prodigy defines five secret providers in the `SecretProvider` enum (`src/cook/environment/config.rs:101-112`):

| Provider | Status | Description | Source Reference |
|----------|--------|-------------|------------------|
| `env` | ✅ Implemented | Reads from environment variables | `manager.rs:324-325` |
| `file` | ✅ Implemented | Reads from filesystem | `manager.rs:326-331` |
| `vault` | 🔮 Planned | HashiCorp Vault integration | `config.rs:107` |
| `aws` | 🔮 Planned | AWS Secrets Manager | `config.rs:109` |
| `custom` | ⚙️ Extensible | Custom provider via SecretStore | `config.rs:111` |

!!! warning "Unimplemented Providers"
    Only `env` and `file` providers are fully implemented in `EnvironmentManager.resolve_secret()` (`src/cook/environment/manager.rs:316-339`). Vault and AWS providers are defined in the enum but delegate to `SecretStore` for implementation, which currently returns "not found" errors.

### Secret Resolution Flow

Secrets are resolved during environment setup with the following flow (`src/cook/environment/manager.rs:130-136`):

```mermaid
flowchart TD
    A[EnvironmentConfig.secrets loaded from YAML] --> B[For each secret, resolve_secret is called]
    B --> C{SecretValue variant?}
    C -->|Simple| D[Look up in std::env]
    C -->|Provider Env| E[Look up in std::env]
    C -->|Provider File| F[Read from filesystem]
    C -->|Provider Other| G[Delegate to SecretStore]
    D --> H[Add resolved value to environment HashMap]
    E --> H
    F --> H
    G --> H
    H --> I[Track secret key in EnvironmentContext.secrets for masking]
```

**Source**: `src/cook/environment/manager.rs:130-136`

### Secret Masking

Secret values are masked in logs and command output to prevent accidental exposure.

```mermaid
flowchart LR
    subgraph Input["Command Execution"]
        Cmd["curl -H 'Bearer sk-abc123'"]
    end

    subgraph Masking["Masking Engine"]
        Check{"Is value
        in secrets?"}
        Replace["Replace with ***"]
    end

    subgraph Output["Log Output"]
        Masked["curl -H 'Bearer ***'"]
    end

    Cmd --> Check
    Check -->|Yes| Replace
    Replace --> Masked
    Check -->|No| Masked

    style Input fill:#fff3e0
    style Masking fill:#e1f5ff
    style Output fill:#e8f5e9
```

**Figure**: Secret masking flow - values defined in `secrets:` are automatically masked in logs.

```yaml
secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

commands:
  - shell: "curl -H 'Authorization: Bearer $API_TOKEN' https://api.github.com"
```

**Output** (masked):
```
$ curl -H 'Authorization: Bearer ***' https://api.github.com
```

**How it works**: The `EnvironmentContext` struct tracks secret keys in a `Vec<String>` field (`src/cook/environment/manager.rs:24-31`). When commands are executed, output is scanned and secret values are replaced with `***`.

### SecretStore Architecture

For extensibility, Prodigy provides a `SecretStore` system that supports custom secret providers (`src/cook/environment/secret_store.rs:27-107`):

```mermaid
graph LR
    subgraph SecretStore["SecretStore"]
        direction TB
        Registry["Provider Registry"]
    end

    subgraph Providers["Secret Providers"]
        direction TB
        Env["EnvSecretProvider
        Environment Variables"]
        File["FileSecretProvider
        Filesystem"]
        Custom["Custom Providers
        (Extensible)"]
    end

    Request["get_secret(key)"] --> SecretStore
    SecretStore --> Registry
    Registry --> Env
    Registry --> File
    Registry --> Custom

    Env --> Result["Secret Value"]
    File --> Result
    Custom --> Result

    style SecretStore fill:#e1f5ff
    style Providers fill:#f3e5f5
    style Result fill:#e8f5e9
```

**Figure**: SecretStore architecture showing provider-based extensibility.

**Built-in Providers**:

- `EnvSecretProvider` - Environment variable lookup (`secret_store.rs:120-131`)
- `FileSecretProvider` - File-based secrets (`secret_store.rs:134-148`)

**Custom Providers**:

You can add custom secret providers by implementing the `SecretProvider` trait:

```rust
// Source: src/cook/environment/secret_store.rs:110-117
#[async_trait::async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get_secret(&self, key: &str) -> Result<String>;
    async fn has_secret(&self, key: &str) -> bool;
}
```

Custom providers can be registered with `SecretStore.add_provider()` (`secret_store.rs:79-81`).

### Real-World Examples

#### Example 1: Simple Environment Variable Secrets

From `workflows/environment-example.yml:21-23`:

```yaml
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

commands:
  - shell: "echo 'Deploying with API key: $API_KEY'"  # Value masked in logs
```

#### Example 2: Provider-Based Secrets in MapReduce

From `workflows/mapreduce-env-example.yml:23-26`:

```yaml
secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

reduce:
  - shell: "curl -H 'Authorization: Bearer $API_TOKEN' https://api.github.com/repos/notify"
```

#### Example 3: File-Based Secrets

!!! tip "Docker Secrets Pattern"
    The file provider works seamlessly with Docker secrets mounted at `/run/secrets/`. This is a common pattern for containerized deployments.

```yaml
secrets:
  DATABASE_PASSWORD:
    provider: file
    key: "/run/secrets/db_password"  # Docker secrets pattern

  SSH_DEPLOY_KEY:
    provider: file
    key: "~/.ssh/deploy_key"

commands:
  - shell: "psql postgresql://user:$DATABASE_PASSWORD@localhost/db"
  - shell: "ssh -i $SSH_DEPLOY_KEY deploy@server 'systemctl restart app'"
```

### Integration with Environment Configuration

Secrets are part of the global `EnvironmentConfig` structure and work alongside other environment features:

```yaml
# Global environment configuration
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secrets (masked in logs)
secrets:
  API_KEY:
    provider: env
    key: "SECRET_API_KEY"

# Environment profiles
profiles:
  production:
    NODE_ENV: production
  development:
    NODE_ENV: development

commands:
  - shell: "curl -H 'X-API-Key: $API_KEY' $API_URL/deploy"
```

**Source**: Structure from `src/cook/environment/config.rs:11-36`

### Security Best Practices

!!! note "Key Security Principles"
    Following these practices helps prevent accidental secret exposure in logs, error messages, and version control.

1. **Never commit secrets to version control**
   - Use environment variables or secret files
   - Add secret files to `.gitignore`

2. **Use the `secrets:` field for sensitive data**
   - Ensures masking in logs and output
   - Prevents accidental exposure in error messages

3. **Prefer environment variables or secure files**
   - Only `env` and `file` providers are currently implemented
   - Vault and AWS providers are planned for future releases

4. **Use profiles for environment-specific secrets**
   ```yaml
   profiles:
     production:
       DB_HOST: "prod-db.example.com"
     development:
       DB_HOST: "localhost"

   secrets:
     DB_PASSWORD:
       provider: env
       key: "DATABASE_PASSWORD"
   ```

5. **Test secret resolution in development**
   - Verify secrets load correctly before deploying
   - Use different secret sources per profile

### Troubleshooting

!!! tip "Debug Secret Resolution"
    Run your workflow with `-vv` to see detailed secret resolution logs without exposing actual values.

#### Issue: "Secret not found in environment"

**Cause**: Secret key doesn't exist in environment variables

**Solution**:
```bash
# Verify environment variable exists
echo $SECRET_API_KEY

# Set the variable before running workflow
export SECRET_API_KEY="your-secret-value"
prodigy run workflow.yml
```

#### Issue: "Failed to read secret file"

**Cause**: Secret file doesn't exist or insufficient permissions

**Solution**:
```bash
# Verify file exists and is readable
ls -la ~/.ssh/deploy_key

# Fix permissions if needed
chmod 600 ~/.ssh/deploy_key
```

#### Issue: Secrets not masked in output

**Cause**: Secret not defined in `secrets:` field

**Solution**: Move sensitive variables from `env:` to `secrets:` block:

```yaml
# Before (NOT masked)
env:
  API_KEY: "${env:SECRET_API_KEY}"

# After (masked)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"
```

### Related Documentation

- [Environment Variables](environment-profiles.md) - General environment variable configuration
- [Environment Profiles](environment-profiles.md) - Profile-based configuration
- [Environment Precedence](environment-precedence.md) - How environment values are resolved

### Implementation References

- **Configuration Types**: `src/cook/environment/config.rs:86-112`
- **Secret Resolution**: `src/cook/environment/manager.rs:316-339`
- **Secret Store**: `src/cook/environment/secret_store.rs:27-107`
- **Environment Setup**: `src/cook/environment/manager.rs:130-136`
- **Test Examples**: `tests/environment_workflow_test.rs:19-59`
- **Workflow Examples**: `workflows/environment-example.yml`, `workflows/mapreduce-env-example.yml`
