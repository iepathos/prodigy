## Available Fields

Standard workflows support these top-level fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | No | Workflow name for identification (defaults to "default") |
| `commands` | Array<WorkflowCommand> | Yes* | List of commands to execute sequentially |
| `env` | Map<String, String> | No | Global environment variables |
| `secrets` | Map<String, SecretValue> | No | Secret environment variables (masked in logs) |
| `env_files` | Array<PathBuf> | No | Paths to .env files to load |
| `profiles` | Map<String, EnvProfile> | No | Named environment profiles for different contexts |
| `merge` | MergeWorkflow | No | Custom merge workflow for worktree integration |

**Source**: Type definitions from [src/config/workflow.rs:11-38](../../src/config/workflow.rs)

**Note:** `commands` is only required in the full format. Use the simple array format for quick workflows without environment configuration. Use the full format when you need environment variables, profiles, or custom merge workflows.

### Field Relationships and Precedence

Understanding how fields interact is important for effective workflow configuration:

1. **Environment Variable Resolution Order**:
   - Command-level `env` overrides all other sources
   - Profile-specific variables (when a profile is active) override global `env`
   - Global `env` variables provide base configuration
   - Variables from `env_files` are loaded first and can be overridden

2. **Secrets vs. Environment Variables**:
   - `secrets` are a special type of environment variable that are masked in logs and output
   - Both `env` and `secrets` are available to all commands
   - Secrets take precedence over regular environment variables with the same name

3. **Profile Activation**:
   - Profiles are activated via `--profile <name>` CLI flag
   - Profile variables merge with global `env`, with profile values taking precedence
   - Common use case: different configurations for dev, staging, and production

### Format Examples

**Simple Array Format** (from [examples/standard-workflow.yml](../../examples/standard-workflow.yml)):

```yaml
- shell: echo "Starting code analysis..."
- shell: cargo check --quiet
- shell: echo "Workflow complete"
```

Use this format when:
- You don't need environment variables
- You have a quick, straightforward sequence of commands
- You want minimal YAML verbosity

**Full Format with Environment** (from [workflows/environment-example.yml](../../workflows/environment-example.yml)):

```yaml
name: production-deploy

env:
  NODE_ENV: production
  API_URL: https://api.example.com

secrets:
  API_KEY: "${env:SECRET_API_KEY}"

env_files:
  - .env.production

profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000
  staging:
    NODE_ENV: staging
    API_URL: https://staging.api.example.com

commands:
  - shell: echo "Deploying with NODE_ENV=$NODE_ENV"
  - shell: npm run build
  - shell: npm run deploy
```

Use this format when:
- You need environment variables
- You have different configurations for different environments (profiles)
- You need to mask sensitive values (secrets)
- You want to load variables from `.env` files

### See Also

- [Environment Configuration](environment-configuration.md) - Detailed environment variable documentation
- [Merge Workflows](merge-workflows.md) - Custom merge workflow configuration
- [Command Types](command-types.md) - Available command types and their options
- [Full Workflow Structure](full-workflow-structure.md) - Complete workflow structure reference

