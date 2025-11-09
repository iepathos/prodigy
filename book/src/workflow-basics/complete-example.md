## Complete Example

Here's a complete workflow combining multiple features:

```yaml
# Environment configuration
env:
  RUST_BACKTRACE: 1

env_files:
  - .env

profiles:
  ci:
    CI: "true"
    VERBOSE: "true"

# Workflow commands
commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy -- -D warnings"
  - shell: "cargo test --all"
  - claude: "/prodigy-lint"

# Custom merge workflow
merge:
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

