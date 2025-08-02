mod common;

use mmm::cook::metrics::collector::MetricsCollectorImpl;
use mmm::cook::metrics::MetricsCoordinator;
use mmm::metrics::registry::{MetricsConfig, MetricsRegistry};
use std::os::unix::process::ExitStatusExt;
use tempfile::TempDir;

// Mock command runner for testing
struct MockCommandRunner;

impl MockCommandRunner {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl mmm::cook::execution::CommandRunner for MockCommandRunner {
    async fn run_command(
        &self,
        _command: &str,
        _args: &[String],
    ) -> anyhow::Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![],
            stderr: vec![],
        })
    }
    async fn run_with_context(
        &self,
        _command: &str,
        _args: &[String],
        _context: &mmm::cook::execution::ExecutionContext,
    ) -> anyhow::Result<mmm::cook::execution::ExecutionResult> {
        Ok(mmm::cook::execution::ExecutionResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }
}

#[tokio::test]
async fn test_metrics_collection_lifecycle() {
    // Initialize test environment (sets MMM_TEST_MODE=true)
    common::init_test_env();

    let temp_dir = TempDir::new().unwrap();
    let config = MetricsConfig::default();
    let _registry = MetricsRegistry::new(config);
    let mock_runner = MockCommandRunner::new();
    let collector = MetricsCollectorImpl::new(mock_runner);

    // Test basic metrics collection
    let result = collector.collect_all(temp_dir.path()).await;
    assert!(result.is_ok());

    let metrics = result.unwrap();
    // In test mode, should have mock values
    assert_eq!(metrics.lint_warnings, 0);
    assert_eq!(metrics.test_coverage, Some(30.0)); // Mock value from MetricsCollector
}
