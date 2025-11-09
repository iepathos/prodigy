## Retry Metrics and Observability

The retry system tracks metrics for monitoring:

```rust
let executor = RetryExecutor::new(config);
// ... execute operations ...
let metrics = executor.metrics().await;

println!("Total attempts: {}", metrics.total_attempts);
println!("Successful: {}", metrics.successful_attempts);
println!("Failed: {}", metrics.failed_attempts);
println!("Retry history: {:?}", metrics.retries);
```

Metrics include:
- `total_attempts` - Total number of attempts made
- `successful_attempts` - Number of successful operations
- `failed_attempts` - Number of failed operations
- `retries` - Vec of (attempt_number, delay) pairs

