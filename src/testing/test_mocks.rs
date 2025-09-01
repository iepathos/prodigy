//! Test utilities for mocking external tools
//!
//! This module provides realistic mock responses for external tools like cargo and git,
//! enabling reliable testing without actual subprocess execution.

use crate::subprocess::MockProcessRunner;

/// Mock responses for cargo commands
pub struct CargoMocks;

impl CargoMocks {
    /// Cargo check successful output with no warnings
    pub fn check_success() -> String {
        r#"{"reason":"compiler-message","message":{"$message_type":"diagnostic","message":"2 warnings emitted","code":null,"level":"warning","spans":[],"children":[],"rendered":"warning: 2 warnings emitted\n"},"target":{"name":"mmm"}}
{"reason":"build-finished","success":true}"#.to_string()
    }

    /// Cargo check output with warnings
    pub fn check_with_warnings() -> String {
        r#"{"reason":"compiler-message","message":{"$message_type":"diagnostic","message":"unused variable: `x`","code":{"code":"unused_variables","explanation":null},"level":"warning","spans":[{"file_name":"src/main.rs","byte_start":123,"byte_end":124,"line_start":10,"line_end":10,"column_start":9,"column_end":10,"is_primary":true,"text":[{"text":"    let x = 5;","highlight_start":9,"highlight_end":10}],"label":"help: if this is intentional, prefix it with an underscore: `_x`","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":"warning: unused variable: `x`\n  --> src/main.rs:10:9\n   |\n10 |     let x = 5;\n   |         ^ help: if this is intentional, prefix it with an underscore: `_x`\n"},"target":{"name":"mmm"}}
{"reason":"build-finished","success":true}"#.to_string()
    }

    /// Cargo clippy output with various lint warnings
    pub fn clippy_output() -> String {
        r#"{"reason":"compiler-message","message":{"$message_type":"diagnostic","message":"you should consider adding a `Default` implementation for `Config`","code":{"code":"clippy::new_without_default","explanation":null},"level":"warning","spans":[{"file_name":"src/config.rs","byte_start":234,"byte_end":260,"line_start":15,"line_end":15,"column_start":5,"column_end":31,"is_primary":true,"text":[{"text":"    pub fn new() -> Self {","highlight_start":5,"highlight_end":31}],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":"warning: you should consider adding a `Default` implementation for `Config`\n  --> src/config.rs:15:5\n   |\n15 |     pub fn new() -> Self {\n   |     ^^^^^^^^^^^^^^^^^^^^^\n   |\n   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#new_without_default\n"},"target":{"name":"mmm"}}
{"reason":"build-finished","success":true}"#.to_string()
    }

    /// Cargo build successful output
    pub fn build_success() -> String {
        r#"   Compiling mmm v0.1.0 (/path/to/project)
    Finished release [optimized] target(s) in 12.5s"#
            .to_string()
    }

    /// Cargo test output with all tests passing
    pub fn test_success() -> String {
        r#"   Compiling mmm v0.1.0 (/path/to/project)
    Finished test [unoptimized + debuginfo] target(s) in 2.3s
     Running unittests (target/debug/deps/prodigy-abc123)

running 42 tests
test context::tests::test_analyze ... ok
test metrics::tests::test_collect ... ok
test subprocess::tests::test_mock ... ok
...

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s"#
            .to_string()
    }
}

/// Mock responses for cargo-tarpaulin coverage tool
pub struct TarpaulinMocks;

impl TarpaulinMocks {
    /// Tarpaulin coverage report with good coverage
    pub fn coverage_report_good() -> String {
        r#"|| Uncovered Lines:
|| src/main.rs: 15, 23-25
|| src/lib.rs: 45
|| src/utils.rs: 78-82, 91
|| 
|| Tested/Total Lines:
|| src/main.rs: 85/100
|| src/lib.rs: 120/125  
|| src/utils.rs: 45/55
|| src/config.rs: 60/60
|| 
85.00% coverage, 310/365 lines covered, +2.30% change in coverage"#
            .to_string()
    }

    /// Tarpaulin coverage report with poor coverage
    pub fn coverage_report_poor() -> String {
        r#"|| Uncovered Lines:
|| src/main.rs: 10-50, 60-80
|| src/lib.rs: 20-60, 70-100
|| src/utils.rs: 15-90
|| 
|| Tested/Total Lines:
|| src/main.rs: 30/100
|| src/lib.rs: 25/125
|| src/utils.rs: 10/100
|| 
30.00% coverage, 65/325 lines covered, -5.00% change in coverage"#
            .to_string()
    }

    /// Tarpaulin XML output for parsing
    pub fn coverage_xml() -> String {
        r#"<coverage>
  <packages>
    <package name="mmm">
      <classes>
        <class name="src/main.rs" filename="src/main.rs" line-rate="0.85">
          <lines>
            <line number="1" hits="1"/>
            <line number="2" hits="1"/>
            <line number="15" hits="0"/>
            <line number="23" hits="0"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>"#
            .to_string()
    }
}

/// Mock responses for git commands
pub struct GitMocks;

impl GitMocks {
    /// Git status with clean working directory
    pub fn status_clean() -> String {
        String::new()
    }

    /// Git status with modified files
    pub fn status_dirty() -> String {
        r#"M  src/main.rs
M  src/lib.rs
?? temp.txt"#
            .to_string()
    }

    /// Git log output
    pub fn log_output() -> String {
        r#"abc1234 feat: add new feature
def5678 fix: resolve bug in parser
ghi9012 docs: update README"#
            .to_string()
    }
}

/// Mock response utilities
pub struct MockResponses;

impl MockResponses {
    /// Generate cargo check JSON output with configurable warnings/errors
    pub fn cargo_check_json(warnings: usize, errors: usize) -> String {
        let mut output = String::new();

        // Add warning messages
        for i in 0..warnings {
            output.push_str(&format!(
                r#"{{"reason":"compiler-message","message":{{"$message_type":"diagnostic","message":"warning {}: test warning","code":null,"level":"warning","spans":[],"children":[],"rendered":"warning: test warning {}\n"}},"target":{{"name":"mmm"}}}}"#,
                i + 1, i + 1
            ));
            output.push('\n');
        }

        // Add error messages
        for i in 0..errors {
            output.push_str(&format!(
                r#"{{"reason":"compiler-message","message":{{"$message_type":"diagnostic","message":"error {}: test error","code":null,"level":"error","spans":[],"children":[],"rendered":"error: test error {}\n"}},"target":{{"name":"mmm"}}}}"#,
                i + 1, i + 1
            ));
            output.push('\n');
        }

        // Add build finished
        let success = errors == 0;
        output.push_str(&format!(
            r#"{{"reason":"build-finished","success":{success}}}"#
        ));

        output
    }
}

/// Test setup utilities
pub struct TestMockSetup;

impl TestMockSetup {
    /// Setup mocks for successful analysis workflow
    pub fn setup_successful_analysis(mock: &mut MockProcessRunner) {
        // Cargo check - no errors
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"check".to_string())
                    && args.contains(&"--message-format=json".to_string())
            })
            .returns_stdout(&CargoMocks::check_success())
            .returns_exit_code(0)
            .finish();

        // Cargo clippy
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"clippy".to_string())
                    && args.contains(&"--message-format=json".to_string())
            })
            .returns_stdout(&CargoMocks::clippy_output())
            .returns_exit_code(0)
            .finish();

        // Cargo test
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"test".to_string()))
            .returns_stdout(&CargoMocks::test_success())
            .returns_exit_code(0)
            .finish();

        // Cargo tarpaulin for coverage
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"tarpaulin".to_string())
                    && args.contains(&"--print-summary".to_string())
            })
            .returns_stdout(&TarpaulinMocks::coverage_report_good())
            .returns_exit_code(0)
            .finish();

        // Cargo build for compile time
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"build".to_string())
                    && args.contains(&"--release".to_string())
            })
            .returns_stdout(&CargoMocks::build_success())
            .returns_exit_code(0)
            .finish();

        // Git status
        mock.expect_command("git")
            .with_args(|args| args.first() == Some(&"status".to_string()))
            .returns_stdout(&GitMocks::status_clean())
            .returns_exit_code(0)
            .finish();
    }

    /// Setup mocks for analysis with some failures
    pub fn setup_analysis_with_failures(mock: &mut MockProcessRunner) {
        // Cargo check - with warnings
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"check".to_string())
                    && args.contains(&"--message-format=json".to_string())
            })
            .returns_stdout(&CargoMocks::check_with_warnings())
            .returns_exit_code(0)
            .finish();

        // Cargo clippy - fails
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"clippy".to_string()))
            .returns_stderr("error: clippy not found")
            .returns_exit_code(1)
            .finish();

        // Cargo tarpaulin - poor coverage
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"tarpaulin".to_string()))
            .returns_stdout(&TarpaulinMocks::coverage_report_poor())
            .returns_exit_code(0)
            .finish();
    }

    /// Setup mocks for metrics collection
    pub fn setup_metrics_collection(mock: &mut MockProcessRunner) {
        // All the commands that metrics collection might run

        // Check if tarpaulin is available
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"tarpaulin".to_string())
                    && args.contains(&"--version".to_string())
            })
            .returns_stdout("cargo-tarpaulin version: 0.27.0")
            .returns_exit_code(0)
            .finish();

        // Test build check
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"test".to_string()) && args.contains(&"--no-run".to_string())
            })
            .returns_stdout("Compiling test v0.1.0")
            .returns_exit_code(0)
            .finish();

        // Clippy for lint warnings (both JSON and regular format)
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"clippy".to_string())
                    && args.contains(&"--message-format=json".to_string())
            })
            .returns_stdout(&CargoMocks::clippy_output())
            .returns_exit_code(0)
            .finish();

        // Clippy regular format (for quality analyzer)
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"clippy".to_string()) && args.contains(&"-W".to_string())
            })
            .returns_stderr("warning: test warning 1\nwarning: test warning 2\n")
            .returns_exit_code(0)
            .finish();

        // Build for compile time
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"build".to_string())
                    && args.contains(&"--release".to_string())
            })
            .returns_stdout(&CargoMocks::build_success())
            .returns_exit_code(0)
            .finish();

        // Check for type checking
        mock.expect_command("cargo")
            .with_args(|args| {
                args.first() == Some(&"check".to_string())
                    && args.contains(&"--message-format=json".to_string())
            })
            .returns_stdout(&CargoMocks::check_success())
            .returns_exit_code(0)
            .finish();
    }
}
