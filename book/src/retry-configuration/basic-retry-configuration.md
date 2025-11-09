## Basic Retry Configuration

The simplest retry configuration uses default values:

```yaml
retry:
  attempts: 3
```

This will:
- Retry up to 3 times
- Use exponential backoff (base 2.0)
- Start with 1 second delay
- Cap delays at 30 seconds

