## Secrets Management

Store sensitive values securely using secret providers:

```yaml
secrets:
  # Provider-based secrets (recommended)
  AWS_SECRET:
    provider: aws
    key: "my-app/api-key"

  VAULT_SECRET:
    provider: vault
    key: "secret/data/myapp"
    version: "v2"  # Optional version

  # Environment variable reference
  API_KEY:
    provider: env
    key: "SECRET_API_KEY"

  # File-based secret
  DB_PASSWORD:
    provider: file
    key: "~/.secrets/db.pass"

  # Custom provider (extensible)
  CUSTOM_SECRET:
    provider: "my-custom-provider"
    key: "secret-id"

commands:
  - shell: "echo $API_KEY"  # Secrets are available as environment variables
```

**Supported Secret Providers:**

- `env` - Reference another environment variable
- `file` - Read secret from a file
- `vault` - HashiCorp Vault integration (requires Vault setup)
- `aws` - AWS Secrets Manager (requires AWS credentials)
- `custom` - Custom provider for advanced use cases. Requires implementing custom secret resolution logic in Prodigy's environment manager. Contact maintainers for extension points.

**Security Notes:**

- Secrets are masked in logs and output
- Secret values are only resolved at runtime
- Use secrets for API keys, passwords, tokens, and other sensitive data

---

