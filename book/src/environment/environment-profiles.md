## Environment Profiles

Define named environment configurations for different contexts:

```yaml
# Define profiles with environment variables
profiles:
  development:
    description: "Development environment with debug enabled"
    NODE_ENV: development
    DEBUG: "true"
    API_URL: http://localhost:3000

  production:
    description: "Production environment configuration"
    NODE_ENV: production
    DEBUG: "false"
    API_URL: https://api.example.com

# Global environment still applies
env:
  APP_NAME: "my-app"

commands:
  - shell: "npm run build"
```

**Profile Structure:**

Profiles use a flat structure where environment variables are defined directly at the profile level (not nested under an `env:` key). The `description` field is optional and helps document the profile's purpose.

```yaml
profiles:
  staging:
    description: "Staging environment"  # Optional
    NODE_ENV: staging                   # Direct key-value pairs
    API_URL: https://staging.api.com
    DEBUG: "true"
```

**Note:** The profile infrastructure exists internally (EnvironmentConfig has an `active_profile` field), but there is currently no CLI flag (like `--profile`) to activate profiles at runtime. Profiles can be defined in YAML for future use, but cannot be activated in the current version.

---

