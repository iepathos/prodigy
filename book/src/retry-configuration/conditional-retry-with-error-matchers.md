## Conditional Retry with Error Matchers

By default, Prodigy retries all errors. Use `retry_on` to retry only specific error types:

**Note**: All error matching is case-insensitive. Error messages are normalized to lowercase before pattern comparison.

