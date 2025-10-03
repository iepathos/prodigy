# Workflow Basics

## Workflow Types

Prodigy supports two primary workflow types:

1. **Standard Workflows**: Sequential command execution
2. **MapReduce Workflows**: Parallel processing with map and reduce phases

---

## Standard Workflows

### Basic Structure

```yaml
# Simple array format (most common)
- shell: "echo 'Starting workflow...'"
- claude: "/prodigy-analyze"
- shell: "cargo test"
```

### Full Configuration Format

```yaml
# Full format with environment and merge configuration
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

# Global environment variables
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secret environment variables (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

# Custom merge workflow
merge:
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional timeout in seconds
```
