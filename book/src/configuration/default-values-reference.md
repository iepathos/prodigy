## Default Values Reference

Quick reference of all default values:

| Setting | Default Value |
|---------|--------------|
| `log_level` | `"info"` |
| `max_concurrent_specs` | `1` |
| `auto_commit` | `true` |
| `spec_dir` | `"specs"` |
| `storage.backend` | `"file"` |
| `storage.use_global` | `true` |
| `storage.enable_locking` | `true` |
| `storage.enable_cache` | `false` |
| `storage.connection_pool_size` | `10` |
| `storage.timeout` | `30s` |
| `storage.retry_policy.max_retries` | `3` |
| `storage.retry_policy.initial_delay` | `1s` |
| `storage.retry_policy.max_delay` | `30s` |
| `storage.retry_policy.backoff_multiplier` | `2.0` |
| `storage.file.max_file_size` | `104857600` (100MB) |
| `storage.cache.max_entries` | `1000` |
| `storage.cache.ttl` | `3600s` (1 hour) |

