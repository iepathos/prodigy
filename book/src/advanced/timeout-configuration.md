## Timeout Configuration

Set execution timeouts at the command level:

```yaml
# Command-level timeout (in seconds)
- shell: "cargo bench"
  timeout: 600  # 10 minutes

# Timeout for long-running operations
- claude: "/analyze-codebase"
  timeout: 1800  # 30 minutes
```

