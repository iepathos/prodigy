## Complete Example

Here's a complete workflow demonstrating Prodigy's core features in a single file. This example combines environment configuration, workflow commands, and custom merge behavior.

**Source**: Based on `workflows/implement.yml`

```yaml
# Environment configuration
env:
  RUST_BACKTRACE: 1              # Standard environment variable

env_files:
  - .env                         # Load variables from .env file (dotenv format)

profiles:
  ci:                            # Activate with: prodigy run workflow.yml --profile ci
    CI: "true"
    VERBOSE: "true"

# Workflow commands
commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy -- -D warnings"
  - shell: "cargo test --all"
  - claude: "/prodigy-lint"

# Custom merge workflow (simplified format)
merge:
  - shell: "cargo test"          # Validate before merging
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

### Key Features Demonstrated

1. **Environment Variables** (lines 2-4): Define variables available to all commands
   - See [Environment Variables](../environment/index.md) for details

2. **Environment Files** (lines 6-7): Load variables from `.env` files in dotenv format
   - See [Environment Files](../environment/environment-files.md) for syntax

3. **Profiles** (lines 9-12): Define environment sets activated via `--profile` flag
   - Example: `prodigy run workflow.yml --profile ci`
   - See [Environment Profiles](../environment/environment-profiles.md) for advanced usage

4. **Workflow Commands** (lines 15-19): Execute shell and Claude commands sequentially
   - See [Command Types](../commands.md) for all command types

5. **Custom Merge Workflow** (lines 22-24): Customize the merge-back process
   - **Important**: Always include both `${merge.source_branch}` and `${merge.target_branch}` parameters
   - The simplified array format is shown here (supported by `MergeWorkflow`)
   - See next section for the full configuration format

### Alternative Merge Format (with timeout)

The merge block also supports a configuration format with timeout:

```yaml
merge:
  commands:
    - shell: "cargo test"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600                   # Optional: timeout in seconds (10 minutes)
```

**Source**: `src/config/mapreduce.rs:86-124`

### Real-World Example

For a production-grade workflow with validation and error handling, see the implementation workflow:

**File**: `workflows/implement.yml` (lines 32-41)

```yaml
merge:
  # Step 1: Merge master into worktree
  - claude: "/prodigy-merge-master"

  # Step 2: Run CI checks and fix any issues
  - claude: "/prodigy-ci"

  # Step 3: Merge worktree back to original branch
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

This demonstrates best practices:
- Sync with upstream before merging back
- Validate changes with CI checks
- Use proper merge parameters

### Next Steps

- [Error Handling](error-handling.md) - Add `on_failure` handlers for robust workflows
- [Variable Interpolation](../variables/available-variables.md) - Use dynamic values in commands
- [Advanced Features](../advanced/index.md) - Explore validation, error handling, and more

