## Environment Variables in Configuration

Many MapReduce configuration fields support environment variable interpolation, allowing you to parameterize workflows:

**Supported Fields:**
- `max_parallel` - Control parallelism dynamically
- `agent_timeout_secs` - Adjust timeouts per environment
- `setup.timeout` - Configure setup phase timeouts
- `merge.timeout` - Control merge operation timeouts
- Any string field in your workflow

**Usage Example:**

```yaml
name: configurable-mapreduce
mode: mapreduce

# Define environment variables (see Variables chapter for details)
env:
  MAX_WORKERS: "10"
  AGENT_TIMEOUT: "300"

map:
  input: "items.json"
  json_path: "$[*]"
  max_parallel: "$MAX_WORKERS"      # Use environment variable
  agent_timeout_secs: "$AGENT_TIMEOUT"
  agent_template:
    - claude: "/process ${item}"

setup:
  timeout: "$SETUP_TIMEOUT"  # Can also reference env vars
  commands:
    - shell: "prepare-data.sh"
```

**Running with Different Values:**

```bash
# Development: Lower parallelism
MAX_WORKERS=5 AGENT_TIMEOUT=600 prodigy run workflow.yml

# Production: Higher parallelism
MAX_WORKERS=20 AGENT_TIMEOUT=300 prodigy run workflow.yml
```

See the [Variables chapter](./variables.md) for comprehensive environment variable documentation including profiles, secrets, and advanced usage.

