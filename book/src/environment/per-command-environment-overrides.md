## Per-Command Environment Overrides

**IMPORTANT:** WorkflowStepCommand does NOT have an `env` field. All per-command environment changes must use shell syntax.

**Note:** The legacy Command struct (structured format) has an `env` field via CommandMetadata, but the modern WorkflowStepCommand format does not. For workflows using the modern `claude:`/`shell:` syntax, use shell-level environment syntax (`ENV=value command`).

You can override environment variables for individual commands using shell environment syntax:

```yaml
env:
  RUST_LOG: info
  API_URL: "https://api.example.com"

# Steps go directly in the workflow
- shell: "cargo run"  # Uses RUST_LOG=info from global env

# Override environment for this command only using shell syntax
- shell: "RUST_LOG=debug cargo run --verbose"

# Change directory and set environment in shell
- shell: "cd frontend && PATH=./node_modules/.bin:$PATH npm run build"
```

**Shell-based Environment Techniques:**

- **Single variable override:** `ENV_VAR=value command`
- **Multiple variables:** `VAR1=value1 VAR2=value2 command`
- **Change directory:** `cd path && command`
- **Combine both:** `cd path && ENV_VAR=value command`

**Note:** A `StepEnvironment` struct exists in the internal runtime (`EnvironmentConfig`), but it is not currently exposed in the WorkflowStepCommand YAML syntax. All per-command environment changes must use shell syntax as shown above.

---

