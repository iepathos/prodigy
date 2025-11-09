## Jitter for Distributed Systems

Jitter adds randomness to retry delays to prevent the "thundering herd" problem where many clients retry at the same time.

```yaml
retry:
  attempts: 5
  backoff:
    exponential:
      base: 2.0
  initial_delay: 10s
  jitter: true
  jitter_factor: 0.5
```

With `jitter_factor: 0.5`:
- A 10s delay becomes a random delay between **7.5s and 12.5s**
- A 20s delay becomes a random delay between **15s and 25s**

The jitter is applied as: `delay + random(-delay * factor / 2, +delay * factor / 2)`

The implementation uses Rust's `random_range` with inclusive bounds on both ends. For example, with a 10s delay and factor 0.5: `10s + random(-2.5s, +2.5s)` = 7.5s to 12.5s

**When to use jitter**:
- Multiple clients accessing the same service
- Distributed systems with many workers
- Rate-limited APIs
- Preventing synchronized retry storms

