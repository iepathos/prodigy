---
number: 66
title: CLI Integration Testing
category: testing
priority: high
status: draft
dependencies: []
created: 2025-09-16
---

# Specification 66: CLI Integration Testing

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The main CLI module currently has only 8.4% test coverage (64/762 lines) with 698 uncovered lines. As the primary user interface, comprehensive CLI testing is essential for ensuring reliable command parsing, execution, and error handling. Improving coverage here would add ~2.7% to overall project coverage.

## Objective

Increase CLI/main module test coverage from 8.4% to 60%+ by implementing integration tests for all commands, argument parsing, configuration handling, and user interaction flows.

## Requirements

### Functional Requirements
- Test all CLI commands (cook, exec, batch, resume, worktree, init, events, dlq)
- Test argument parsing and validation
- Test configuration file loading and merging
- Test verbose output levels (-v, -vv, -vvv)
- Test error reporting and exit codes
- Test signal handling (SIGINT, SIGTERM)
- Test file path resolution and validation
- Test interactive prompts and auto-accept mode

### Non-Functional Requirements
- Tests must use command-line invocation simulation
- Tests must verify stdout/stderr output
- Tests must check exit codes
- Tests must complete within 30 seconds total

## Acceptance Criteria

- [ ] CLI module coverage reaches 60% or higher
- [ ] All commands have at least one integration test
- [ ] Argument validation is comprehensively tested
- [ ] Error messages are verified for common failures
- [ ] Signal handling is tested
- [ ] Configuration loading is verified
- [ ] Output formatting is tested at all verbosity levels
- [ ] All tests pass in CI environment

## Technical Details

### Implementation Approach

#### Commands to Test

1. **cook Command**
   ```bash
   prodigy cook workflow.yaml
   prodigy cook workflow.yaml -n 3 --worktree
   prodigy cook workflow.yaml --args KEY=value
   prodigy cook workflow.yaml -y  # auto-accept
   ```

2. **exec Command**
   ```bash
   prodigy exec "claude: /refactor main.rs"
   prodigy exec "shell: npm test" --retry 3
   prodigy exec "shell: sleep 10" --timeout 5
   ```

3. **batch Command**
   ```bash
   prodigy batch "*.rs" --command "claude: /lint {}"
   prodigy batch "src/**/*.ts" --parallel 10
   prodigy batch "*.py" --command "shell: black {}" --retry 2
   ```

4. **worktree Command**
   ```bash
   prodigy worktree ls
   prodigy worktree clean
   prodigy worktree clean --force
   prodigy worktree create feature-branch
   ```

5. **events Command**
   ```bash
   prodigy events list
   prodigy events show <job-id>
   prodigy events tail <job-id>
   prodigy events clean --older-than 7d
   ```

6. **dlq Command**
   ```bash
   prodigy dlq list
   prodigy dlq show <job-id>
   prodigy dlq reprocess <job-id>
   prodigy dlq clear <job-id>
   ```

### Test Structure

```rust
// tests/cli/integration_tests.rs
mod cook_command_tests;
mod exec_command_tests;
mod batch_command_tests;
mod worktree_command_tests;
mod events_command_tests;
mod dlq_command_tests;
mod argument_parsing_tests;
mod configuration_tests;

// Test utilities
pub struct CliTest {
    temp_dir: TempDir,
    command: Command,
}

impl CliTest {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let mut command = Command::new("cargo");
        command.arg("run").arg("--").arg("--path").arg(temp_dir.path());
        Self { temp_dir, command }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.command.arg(arg);
        self
    }

    pub async fn run(&mut self) -> CliOutput {
        let output = self.command.output().await.unwrap();
        CliOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        }
    }
}
```

### Key Test Scenarios

```rust
#[tokio::test]
async fn test_cook_with_invalid_workflow() {
    let output = CliTest::new()
        .arg("cook")
        .arg("nonexistent.yaml")
        .run()
        .await;

    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("File not found"));
}

#[tokio::test]
async fn test_verbose_levels() {
    // Test -v (debug)
    let output = CliTest::new()
        .arg("-v")
        .arg("cook")
        .arg("test.yaml")
        .run()
        .await;
    assert!(output.stderr.contains("[DEBUG]"));

    // Test -vv (trace)
    let output = CliTest::new()
        .arg("-vv")
        .arg("cook")
        .arg("test.yaml")
        .run()
        .await;
    assert!(output.stderr.contains("[TRACE]"));
}

#[tokio::test]
async fn test_signal_handling() {
    let mut child = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("cook")
        .arg("long-running.yaml")
        .spawn()
        .unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    child.kill().await.unwrap();

    let status = child.wait().await.unwrap();
    assert!(!status.success());
}

#[tokio::test]
async fn test_configuration_loading() {
    let config_content = r#"
        default_max_iterations: 5
        auto_worktree: true
    "#;

    let test = CliTest::new()
        .with_config(config_content)
        .arg("cook")
        .arg("test.yaml");

    // Verify config is applied
}
```

### Error Scenarios

```rust
#[tokio::test]
async fn test_missing_required_args() {
    let output = CliTest::new()
        .arg("batch")  // Missing pattern
        .run()
        .await;

    assert_eq!(output.exit_code, 2);
    assert!(output.stderr.contains("required arguments"));
}

#[tokio::test]
async fn test_invalid_timeout_value() {
    let output = CliTest::new()
        .arg("exec")
        .arg("shell: echo test")
        .arg("--timeout")
        .arg("not-a-number")
        .run()
        .await;

    assert_eq!(output.exit_code, 2);
    assert!(output.stderr.contains("invalid value"));
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: CLI parser, command handlers
- **External Dependencies**: clap for argument parsing

## Testing Strategy

- **Integration Tests**: Full command execution paths
- **Error Tests**: Invalid inputs and failure scenarios
- **Output Tests**: Verify formatted output
- **Signal Tests**: Interruption handling
- **Configuration Tests**: Settings and overrides

## Documentation Requirements

- **Test Documentation**: Purpose of each test scenario
- **Output Examples**: Expected command outputs
- **Error Catalog**: Common error messages and codes

## Implementation Notes

### Testing Utilities

```rust
// Helper for creating test workflows
pub fn create_test_workflow(name: &str) -> PathBuf {
    let content = format!(r#"
        name: {}
        steps:
          - shell: "echo 'Test workflow'"
    "#, name);

    let path = temp_dir.join(format!("{}.yaml", name));
    std::fs::write(&path, content).unwrap();
    path
}

// Helper for mocking git repositories
pub fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    Command::new("git")
        .arg("init")
        .current_dir(&dir)
        .output()
        .unwrap();
    dir
}
```

### Exit Code Standards

- 0: Success
- 1: General error
- 2: Argument parsing error
- 3: Configuration error
- 130: Interrupted (SIGINT)
- 143: Terminated (SIGTERM)

## Migration and Compatibility

Tests are additive only; no changes to CLI interface required.