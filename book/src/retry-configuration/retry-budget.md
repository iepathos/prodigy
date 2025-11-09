## Retry Budget

A retry budget limits the total time spent on retries to prevent indefinite retry loops:

```yaml
retry:
  attempts: 10
  retry_budget: 5m
  backoff:
    exponential:
      base: 2.0
  initial_delay: 1s
```

In this example:
- Allows up to 10 retry attempts
- **BUT** stops retrying if total time exceeds 5 minutes
- Useful for preventing workflows from hanging indefinitely

**Without retry budget**: Exponential backoff with 10 attempts could take hours
**With retry budget**: Guarantees workflow fails within 5 minutes

