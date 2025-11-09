## Backoff Strategy Comparison

| Strategy | Attempt 1 | Attempt 2 | Attempt 3 | Attempt 4 | Attempt 5 | Best For |
|----------|-----------|-----------|-----------|-----------|-----------|----------|
| Fixed (2s) | 2s | 2s | 2s | 2s | 2s | Simple retry |
| Linear (+2s) | 1s | 3s | 5s | 7s | 9s | Gradual backoff |
| Exponential (base 2.0) | 1s | 2s | 4s | 8s | 16s | Most failures |
| Fibonacci | 1s | 1s | 2s | 3s | 5s | Distributed systems |

