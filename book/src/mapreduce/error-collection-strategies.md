## Error Collection Strategies

The `error_collection` field controls how errors are reported during workflow execution:

```yaml
error_policy:
  # Collect all errors and report at workflow end (default)
  error_collection: aggregate

  # OR: Report each error immediately as it occurs
  error_collection: immediate

  # OR: Report errors in batches of N items
  error_collection: batched:10
```

**Strategy Behaviors:**
- `aggregate` - Collect all errors in memory and report at the end
  - Use for: Final summary reports, batch processing where individual failures don't need immediate attention
  - Trade-off: Low noise, but you won't see errors until completion
- `immediate` - Log/report each error as soon as it happens
  - Use for: Debugging, development, real-time monitoring
  - Trade-off: More verbose, but immediate visibility
- `batched:N` - Report errors in batches of N items
  - Use for: Progress updates without spam, monitoring long-running jobs
  - Trade-off: Balance between noise and visibility (e.g., `batched:10` reports every 10 failures)

