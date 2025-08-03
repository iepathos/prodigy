//! Unit tests for the analyze module

use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_analyze_command_creation() {
    let cmd = AnalyzeCommand {
        analysis_type: "context".to_string(),
        output: "json".to_string(),
        save: false,
        verbose: false,
        path: None,
        run_coverage: false,
        no_commit: false,
    };

    assert_eq!(cmd.analysis_type, "context");
    assert_eq!(cmd.output, "json");
    assert!(!cmd.save);
    assert!(!cmd.verbose);
    assert!(cmd.path.is_none());
    assert!(!cmd.run_coverage);
}

#[tokio::test]
async fn test_analyze_with_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let cmd = AnalyzeCommand {
        analysis_type: "context".to_string(),
        output: "json".to_string(),
        save: false,
        verbose: false,
        path: Some(temp_dir.path().to_path_buf()),
        run_coverage: false,
        no_commit: false,
    };

    assert!(cmd.path.is_some());
    assert_eq!(cmd.path.unwrap(), temp_dir.path());
}

#[tokio::test]
async fn test_run_analyze_with_invalid_type() {
    let _cmd = AnalyzeCommand {
        analysis_type: "invalid".to_string(),
        output: "json".to_string(),
        save: false,
        verbose: false,
        path: None,
        run_coverage: false,
        no_commit: false,
    };

    // This should fail with exit code 1
    // We can't easily test process::exit, so we'll test the command module directly
}

#[tokio::test]
async fn test_analyze_command_all_fields() {
    let temp_dir = TempDir::new().unwrap();
    let cmd = AnalyzeCommand {
        analysis_type: "all".to_string(),
        output: "pretty".to_string(),
        save: true,
        verbose: true,
        path: Some(temp_dir.path().to_path_buf()),
        run_coverage: true,
        no_commit: false,
    };

    assert_eq!(cmd.analysis_type, "all");
    assert_eq!(cmd.output, "pretty");
    assert!(cmd.save);
    assert!(cmd.verbose);
    assert!(cmd.path.is_some());
    assert!(cmd.run_coverage);
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use crate::analyze::command;
    use anyhow::Result;
    use std::fs;

    fn create_test_project(dir: &std::path::Path) -> Result<()> {
        // Create a minimal Rust project structure
        fs::create_dir_all(dir.join("src"))?;
        fs::write(
            dir.join("Cargo.toml"),
            r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        )?;
        fs::write(
            dir.join("src/main.rs"),
            r#"
fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_main() {
        assert_eq!(2 + 2, 4);
    }
}
"#,
        )?;
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_context_analysis() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: false,
            verbose: true,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Hangs waiting for external tools - needs timeout/mocking"]
    async fn test_execute_metrics_analysis() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "metrics".to_string(),
            output: "summary".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Hangs waiting for external tools - needs timeout/mocking"]
    async fn test_execute_all_analysis() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "all".to_string(),
            output: "pretty".to_string(),
            save: true,
            verbose: true,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());

        // Check that metrics were saved
        assert!(temp_dir.path().join(".mmm/metrics/current.json").exists());
    }

    #[tokio::test]
    async fn test_context_analysis_output_formats() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        // Test JSON output
        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());

        // Test pretty output
        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "pretty".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Hangs waiting for external tools - needs timeout/mocking"]
    async fn test_metrics_analysis_output_formats() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        // Test all output formats for metrics
        for output_format in &["json", "pretty", "summary"] {
            let cmd = AnalyzeCommand {
                analysis_type: "metrics".to_string(),
                output: output_format.to_string(),
                save: false,
                verbose: false,
                path: Some(temp_dir.path().to_path_buf()),
                run_coverage: false,
        no_commit: false,
            };

            let result = command::execute(cmd).await;
            assert!(result.is_ok(), "Failed with output format: {output_format}");
        }
    }

    #[tokio::test]
    async fn test_analyze_with_save_flag() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: true,
            verbose: true,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());

        // Check that context was saved
        assert!(temp_dir.path().join(".mmm/context/analysis.json").exists());
    }

    #[tokio::test]
    async fn test_analyze_non_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        // Don't create Cargo.toml - simulate non-Rust project
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(temp_dir.path().join("src/main.py"), "print('Hello')").unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        // Should handle gracefully
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Hangs waiting for external tools - needs timeout/mocking"]
    async fn test_analyze_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        // Create empty Cargo.toml
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"empty\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "all".to_string(),
            output: "summary".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
        no_commit: false,
        };

        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // This test can hang when analyzing the full mmm project
    async fn test_analyze_without_path_uses_current_dir() {
        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: false,
            verbose: false,
            path: None,
            run_coverage: false,
        no_commit: false,
        };

        // This should use current directory
        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_with_run_coverage_flag() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path()).unwrap();

        let cmd = AnalyzeCommand {
            analysis_type: "context".to_string(),
            output: "json".to_string(),
            save: false,
            verbose: true,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: true,
            no_commit: false,
        };

        // This might fail if cargo-tarpaulin isn't installed, but should handle gracefully
        let result = command::execute(cmd).await;
        assert!(result.is_ok());
    }
}
