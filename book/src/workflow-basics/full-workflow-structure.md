## Full Workflow Structure

For more complex workflows, use the full format with explicit configuration:

```yaml
# Full format with environment and merge configuration
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

# Global environment variables (available to all commands)
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secret environment variables (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles (switch contexts easily)
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

# Custom merge workflow (for worktree integration)
merge:
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional timeout in seconds
```

