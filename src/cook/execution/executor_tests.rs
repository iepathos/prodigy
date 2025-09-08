//! Unit tests for the unified command executor

#[cfg(test)]
mod tests {
    use super::super::command::*;
    use super::super::executor::*;
    use super::super::output::{OutputProcessor, ProcessOutput, ProcessedOutput};
    use super::super::process::ProcessManager;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    // Mock observability collector for testing
    struct MockObservabilityCollector;

    #[async_trait]
    impl ObservabilityCollector for MockObservabilityCollector {
        async fn record_command_start(&self, _context: &ExecutionContextInternal) {}
        async fn record_command_complete(&self, _result: &Result<CommandResult>) {}
    }

    fn create_test_executor() -> UnifiedCommandExecutor {
        UnifiedCommandExecutor::new(
            Arc::new(ProcessManager::new()),
            Arc::new(OutputProcessor::new()),
            Arc::new(MockObservabilityCollector),
            Arc::new(ResourceMonitor),
        )
    }

    fn create_test_request(spec: CommandSpec) -> CommandRequest {
        CommandRequest {
            spec,
            execution_config: ExecutionConfig::default(),
            context: ExecutionContext::default(),
            metadata: CommandMetadata::new("test"),
        }
    }

    #[tokio::test]
    async fn test_command_status_success() {
        let status = CommandStatus::Success;
        assert!(matches!(status, CommandStatus::Success));
    }

    #[tokio::test]
    async fn test_command_status_failed() {
        let status = CommandStatus::Failed {
            reason: FailureReason::NonZeroExit(1),
            retryable: true,
        };

        match status {
            CommandStatus::Failed { reason, retryable } => {
                assert!(matches!(reason, FailureReason::NonZeroExit(1)));
                assert!(retryable);
            }
            _ => panic!("Expected Failed status"),
        }
    }

    #[tokio::test]
    async fn test_command_result_is_success() {
        let result = CommandResult {
            command_id: "test-id".to_string(),
            command_spec: CommandSpec::Shell {
                command: "echo test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::Success,
            output: ProcessedOutput {
                content: ProcessOutput::empty(),
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(1),
            resource_usage: ResourceUsage::default(),
            exit_code: Some(0),
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert!(result.is_success());
        assert!(!result.is_retryable());
    }

    #[tokio::test]
    async fn test_command_result_is_retryable() {
        let result = CommandResult {
            command_id: "test-id".to_string(),
            command_spec: CommandSpec::Shell {
                command: "test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::Failed {
                reason: FailureReason::NonZeroExit(1),
                retryable: true,
            },
            output: ProcessedOutput {
                content: ProcessOutput::empty(),
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(1),
            resource_usage: ResourceUsage::default(),
            exit_code: Some(1),
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert!(!result.is_success());
        assert!(result.is_retryable());
    }

    #[tokio::test]
    async fn test_command_result_output_text() {
        let mut output = ProcessOutput::empty();
        output.stdout = Some("test output".to_string());

        let result = CommandResult {
            command_id: "test-id".to_string(),
            command_spec: CommandSpec::Shell {
                command: "echo test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::Success,
            output: ProcessedOutput {
                content: output,
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(1),
            resource_usage: ResourceUsage::default(),
            exit_code: Some(0),
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert_eq!(result.get_output_text(), Some("test output"));
    }

    #[tokio::test]
    async fn test_validation_issue_levels() {
        let error = ValidationIssue {
            level: ValidationLevel::Error,
            message: "Error message".to_string(),
        };
        assert!(matches!(error.level, ValidationLevel::Error));

        let warning = ValidationIssue {
            level: ValidationLevel::Warning,
            message: "Warning message".to_string(),
        };
        assert!(matches!(warning.level, ValidationLevel::Warning));

        let info = ValidationIssue {
            level: ValidationLevel::Info,
            message: "Info message".to_string(),
        };
        assert!(matches!(info.level, ValidationLevel::Info));
    }

    #[tokio::test]
    async fn test_executor_capabilities() {
        let executor = create_test_executor();
        let capabilities = executor.capabilities();

        assert_eq!(capabilities.supported_command_types.len(), 4);
        assert!(capabilities
            .supported_command_types
            .contains(&CommandType::Claude));
        assert!(capabilities
            .supported_command_types
            .contains(&CommandType::Shell));
        assert!(capabilities
            .supported_command_types
            .contains(&CommandType::Test));
        assert!(capabilities
            .supported_command_types
            .contains(&CommandType::Handler));

        assert_eq!(capabilities.max_concurrent_executions, Some(10));
        assert!(capabilities.timeout_support);
        assert!(capabilities.resource_limiting_support);
        assert!(capabilities.security_context_support);
    }

    #[tokio::test]
    async fn test_executor_supports_all_types() {
        let executor = create_test_executor();

        assert!(executor.supports(&CommandType::Claude));
        assert!(executor.supports(&CommandType::Shell));
        assert!(executor.supports(&CommandType::Test));
        assert!(executor.supports(&CommandType::Handler));
    }

    #[tokio::test]
    async fn test_estimate_resources_claude() {
        let executor = create_test_executor();
        let request = create_test_request(CommandSpec::Claude {
            command: "test".to_string(),
            context: None,
            tools: None,
            output_format: None,
        });

        let estimate = executor.estimate_resources(&request).await.unwrap();
        assert_eq!(estimate.estimated_memory_mb, Some(512));
        assert_eq!(estimate.estimated_cpu_percent, Some(10.0));
        assert_eq!(estimate.confidence, 0.5);
    }

    #[tokio::test]
    async fn test_estimate_resources_shell_git() {
        let executor = create_test_executor();
        let request = create_test_request(CommandSpec::Shell {
            command: "git status".to_string(),
            shell: None,
            working_dir: None,
            env: None,
        });

        let estimate = executor.estimate_resources(&request).await.unwrap();
        assert_eq!(estimate.estimated_duration, Some(Duration::from_secs(5)));
        assert_eq!(estimate.estimated_memory_mb, Some(128));
        assert_eq!(estimate.estimated_cpu_percent, Some(20.0));
        assert_eq!(estimate.confidence, 0.8);
    }

    #[tokio::test]
    async fn test_estimate_resources_test() {
        let executor = create_test_executor();
        let request = create_test_request(CommandSpec::Test {
            command: "cargo test".to_string(),
            expected_exit_code: Some(0),
            validation_script: None,
            retry_config: None,
        });

        let estimate = executor.estimate_resources(&request).await.unwrap();
        assert_eq!(estimate.estimated_duration, Some(Duration::from_secs(30)));
        assert_eq!(estimate.estimated_memory_mb, Some(512));
        assert_eq!(estimate.estimated_cpu_percent, Some(80.0));
        assert_eq!(estimate.confidence, 0.7);
    }

    #[tokio::test]
    async fn test_execution_metadata_creation() {
        let metadata = ExecutionMetadata::new();
        assert!(!metadata.hostname.is_empty());
        assert!(metadata.process_id.is_some());
        assert!(!metadata.working_directory.as_os_str().is_empty());
    }

    #[tokio::test]
    async fn test_resource_usage_default() {
        let usage = ResourceUsage::default();
        assert_eq!(usage.cpu_time, Duration::from_secs(0));
        assert_eq!(usage.wall_clock_time, Duration::from_secs(0));
        assert_eq!(usage.peak_memory_bytes, 0);
        assert_eq!(usage.disk_read_bytes, 0);
        assert_eq!(usage.disk_write_bytes, 0);
        assert!(usage.network_bytes.is_none());
    }

    #[tokio::test]
    async fn test_validation_result() {
        let passed = ValidationResult {
            passed: true,
            issues: Vec::new(),
        };
        assert!(passed.passed);
        assert!(passed.issues.is_empty());

        let failed = ValidationResult {
            passed: false,
            issues: vec![ValidationIssue {
                level: ValidationLevel::Error,
                message: "Test failed".to_string(),
            }],
        };
        assert!(!failed.passed);
        assert_eq!(failed.issues.len(), 1);
    }

    #[tokio::test]
    async fn test_failure_reason_variants() {
        let non_zero = FailureReason::NonZeroExit(1);
        assert!(matches!(non_zero, FailureReason::NonZeroExit(1)));

        let process_error = FailureReason::ProcessError("error".to_string());
        assert!(matches!(
            process_error,
            FailureReason::ProcessError(ref s) if s == "error"
        ));

        let validation_failed = FailureReason::ValidationFailed(vec![]);
        assert!(matches!(
            validation_failed,
            FailureReason::ValidationFailed(ref v) if v.is_empty()
        ));

        let security = FailureReason::SecurityViolation("violation".to_string());
        assert!(matches!(
            security,
            FailureReason::SecurityViolation(ref s) if s == "violation"
        ));

        let resource = FailureReason::ResourceExhaustion("exhausted".to_string());
        assert!(matches!(
            resource,
            FailureReason::ResourceExhaustion(ref s) if s == "exhausted"
        ));

        let internal = FailureReason::InternalError("internal".to_string());
        assert!(matches!(
            internal,
            FailureReason::InternalError(ref s) if s == "internal"
        ));
    }

    #[tokio::test]
    async fn test_command_error() {
        let error = CommandError {
            message: "Command failed".to_string(),
            details: Some("Additional details".to_string()),
        };

        assert_eq!(error.message, "Command failed");
        assert_eq!(error.details, Some("Additional details".to_string()));
    }

    #[tokio::test]
    async fn test_execution_context_builder() {
        let request = create_test_request(CommandSpec::Shell {
            command: "test".to_string(),
            shell: None,
            working_dir: None,
            env: None,
        });

        let context = ExecutionContextBuilder::new()
            .with_id(uuid::Uuid::new_v4())
            .with_request(&request)
            .with_resource_limits(&None)
            .build()
            .unwrap();

        assert!(matches!(context.request.spec, CommandSpec::Shell { .. }));
        assert!(context.resource_limits.is_none());
    }

    #[tokio::test]
    async fn test_execution_context_builder_missing_request() {
        let result = ExecutionContextBuilder::new()
            .with_id(uuid::Uuid::new_v4())
            .build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resource_monitor_validate_limits() {
        let monitor = ResourceMonitor;
        let limits = ResourceLimits {
            max_memory_bytes: Some(1024 * 1024),
            max_cpu_percent: Some(50.0),
            max_disk_io_bytes: None,
            max_network_bytes: None,
            max_file_descriptors: None,
        };

        // Should not fail for now (TODO implementation)
        assert!(monitor.validate_limits(&limits).await.is_ok());
    }

    #[tokio::test]
    async fn test_resource_monitor_check_resources() {
        let monitor = ResourceMonitor;
        let requirements = ResourceRequirements {
            estimated_memory_mb: Some(256),
            estimated_cpu_cores: Some(2.0),
            estimated_duration: Some(Duration::from_secs(10)),
        };

        // Should not fail for now (TODO implementation)
        assert!(monitor.check_resources(&requirements).await.is_ok());
    }

    #[tokio::test]
    async fn test_command_status_timed_out() {
        let result = CommandResult {
            command_id: "test".to_string(),
            command_spec: CommandSpec::Shell {
                command: "test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::TimedOut,
            output: ProcessedOutput {
                content: ProcessOutput::empty(),
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(60),
            resource_usage: ResourceUsage::default(),
            exit_code: None,
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert!(!result.is_success());
        assert!(result.is_retryable());
    }

    #[tokio::test]
    async fn test_command_status_cancelled() {
        let result = CommandResult {
            command_id: "test".to_string(),
            command_spec: CommandSpec::Shell {
                command: "test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::Cancelled,
            output: ProcessedOutput {
                content: ProcessOutput::empty(),
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(0),
            resource_usage: ResourceUsage::default(),
            exit_code: None,
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert!(!result.is_success());
        assert!(!result.is_retryable());
    }

    #[tokio::test]
    async fn test_command_status_resource_limit_exceeded() {
        let result = CommandResult {
            command_id: "test".to_string(),
            command_spec: CommandSpec::Shell {
                command: "test".to_string(),
                shell: None,
                working_dir: None,
                env: None,
            },
            status: CommandStatus::ResourceLimitExceeded,
            output: ProcessedOutput {
                content: ProcessOutput::empty(),
                format: OutputFormat::PlainText,
                processing_duration: Duration::from_secs(0),
                warnings: Vec::new(),
            },
            execution_time: Duration::from_secs(10),
            resource_usage: ResourceUsage::default(),
            exit_code: None,
            error: None,
            validation_result: None,
            metadata: ExecutionMetadata::new(),
        };

        assert!(!result.is_success());
        assert!(result.is_retryable());
    }
}
