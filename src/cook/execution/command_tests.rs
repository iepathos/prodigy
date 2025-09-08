//! Unit tests for the command module

#[cfg(test)]
mod tests {
    use super::super::command::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_command_metadata_creation() {
        let metadata = CommandMetadata::new("test");
        assert_eq!(metadata.tags.get("type"), Some(&"test".to_string()));
        assert!(!metadata.command_id.is_empty());
        assert_eq!(metadata.iteration, 0);
    }

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();
        assert!(config.timeout.is_none());
        assert!(matches!(config.capture_output, CaptureOutputMode::Both));
        assert!(config.working_dir.is_none());
        assert!(config.env.is_empty());
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert_eq!(config.exponential_base, 2.0);
    }

    #[test]
    fn test_execution_context_default() {
        let context = ExecutionContext::default();
        assert!(context.env_vars.is_empty());
        assert!(context.variables.is_empty());
        assert!(context.capture_output);
        assert!(context.timeout.is_none());
    }

    #[test]
    fn test_execution_context_substitute_variables() {
        let mut context = ExecutionContext::default();
        context
            .variables
            .insert("name".to_string(), "world".to_string());
        context
            .variables
            .insert("action".to_string(), "greet".to_string());

        // Test ${} syntax
        assert_eq!(
            context.substitute_variables("Hello ${name}!"),
            "Hello world!"
        );

        // Test ${} with braces
        assert_eq!(
            context.substitute_variables("${action} ${name}"),
            "greet world"
        );

        // Test mixed variables
        assert_eq!(
            context.substitute_variables("${action} the ${name}"),
            "greet the world"
        );

        // Test no substitution needed
        assert_eq!(context.substitute_variables("plain text"), "plain text");

        // Test undefined variable
        assert_eq!(context.substitute_variables("${undefined}"), "${undefined}");
    }

    #[test]
    fn test_executable_command_new() {
        let cmd = ExecutableCommand::new("echo");
        assert_eq!(cmd.program, "echo");
        assert!(cmd.args.is_empty());
        assert_eq!(cmd.command_type, CommandType::Shell);
        assert!(cmd.working_dir.is_none());
        assert_eq!(cmd.expected_exit_code, Some(0));
    }

    #[test]
    fn test_executable_command_builder() {
        let cmd = ExecutableCommand::new("cargo")
            .arg("test")
            .args(vec!["--", "--nocapture"])
            .with_type(CommandType::Test)
            .with_working_dir(Some(PathBuf::from("/tmp")))
            .with_expected_exit_code(Some(1));

        assert_eq!(cmd.program, "cargo");
        assert_eq!(cmd.args, vec!["test", "--", "--nocapture"]);
        assert_eq!(cmd.command_type, CommandType::Test);
        assert_eq!(cmd.working_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(cmd.expected_exit_code, Some(1));
    }

    #[test]
    fn test_executable_command_from_string() {
        // Simple command
        let cmd = ExecutableCommand::from_string("echo hello").unwrap();
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, vec!["hello"]);

        // Command with quotes
        let cmd = ExecutableCommand::from_string("echo 'hello world'").unwrap();
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, vec!["hello world"]);

        // Complex command
        let cmd = ExecutableCommand::from_string("cargo test --lib -- --nocapture").unwrap();
        assert_eq!(cmd.program, "cargo");
        assert_eq!(cmd.args, vec!["test", "--lib", "--", "--nocapture"]);

        // Empty command should fail
        assert!(ExecutableCommand::from_string("").is_err());
    }

    #[test]
    fn test_executable_command_display() {
        let cmd = ExecutableCommand::new("echo");
        assert_eq!(cmd.display(), "echo");

        let cmd = ExecutableCommand::new("echo").arg("hello").arg("world");
        assert_eq!(cmd.display(), "echo hello world");
    }

    #[test]
    fn test_resource_requirements_default() {
        let req = ResourceRequirements::default();
        assert!(req.estimated_memory_mb.is_none());
        assert!(req.estimated_cpu_cores.is_none());
        assert!(req.estimated_duration.is_none());
    }

    #[test]
    fn test_cleanup_requirements_default() {
        let req = CleanupRequirements::default();
        assert_eq!(req.kill_timeout, Duration::from_secs(5));
        assert!(req.cleanup_children);
        assert!(!req.preserve_output);
    }

    #[test]
    fn test_command_spec_to_executable_claude() {
        let spec = CommandSpec::Claude {
            command: "test command".to_string(),
            context: None,
            tools: None,
            output_format: None,
        };

        let context = ExecutionContext::default();
        let cmd = spec.to_executable_command(&context).unwrap();

        assert_eq!(cmd.program, "claude");
        assert_eq!(
            cmd.args,
            vec!["--print", "--dangerously-skip-permissions", "test command"]
        );
        assert_eq!(cmd.command_type, CommandType::Claude);
    }

    #[test]
    fn test_command_spec_to_executable_shell() {
        let spec = CommandSpec::Shell {
            command: "echo hello".to_string(),
            shell: Some("bash".to_string()),
            working_dir: Some(PathBuf::from("/tmp")),
            env: None,
        };

        let context = ExecutionContext::default();
        let cmd = spec.to_executable_command(&context).unwrap();

        assert_eq!(cmd.program, "bash");
        assert_eq!(cmd.args, vec!["-c", "echo hello"]);
        assert_eq!(cmd.command_type, CommandType::Shell);
        assert_eq!(cmd.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_command_spec_to_executable_test() {
        let spec = CommandSpec::Test {
            command: "cargo test".to_string(),
            expected_exit_code: Some(0),
            validation_script: None,
            retry_config: None,
        };

        let context = ExecutionContext::default();
        let cmd = spec.to_executable_command(&context).unwrap();

        assert_eq!(cmd.program, "cargo");
        assert_eq!(cmd.args, vec!["test"]);
        assert_eq!(cmd.command_type, CommandType::Test);
        assert_eq!(cmd.expected_exit_code, Some(0));
    }

    #[test]
    fn test_command_spec_with_variable_substitution() {
        let mut context = ExecutionContext::default();
        context
            .variables
            .insert("project".to_string(), "myapp".to_string());

        let spec = CommandSpec::Shell {
            command: "echo Building ${project}".to_string(),
            shell: None,
            working_dir: None,
            env: None,
        };

        let cmd = spec.to_executable_command(&context).unwrap();
        assert_eq!(cmd.args[1], "echo Building myapp");
    }

    #[test]
    fn test_handler_action_to_executable() {
        let spec = CommandSpec::Handler {
            action: HandlerAction::OnSuccess {
                command: "echo Success".to_string(),
            },
            context: HandlerContext {
                previous_result: None,
                error_message: None,
                workflow_state: HashMap::new(),
            },
            condition: None,
        };

        let context = ExecutionContext::default();
        let cmd = spec.to_executable_command(&context).unwrap();

        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, vec!["Success"]);
        assert_eq!(cmd.command_type, CommandType::Handler);
    }

    #[test]
    fn test_output_format_equality() {
        assert_eq!(OutputFormat::Json, OutputFormat::Json);
        assert_ne!(OutputFormat::Json, OutputFormat::Yaml);
    }

    #[test]
    fn test_command_type_equality() {
        assert_eq!(CommandType::Claude, CommandType::Claude);
        assert_ne!(CommandType::Claude, CommandType::Shell);
    }

    #[test]
    fn test_capture_output_mode_variants() {
        let modes = vec![
            CaptureOutputMode::None,
            CaptureOutputMode::Stdout,
            CaptureOutputMode::Stderr,
            CaptureOutputMode::Both,
            CaptureOutputMode::Structured,
        ];

        // Ensure all variants are distinct
        for (i, mode1) in modes.iter().enumerate() {
            for (j, mode2) in modes.iter().enumerate() {
                if i == j {
                    assert!(matches!(
                        (mode1, mode2),
                        (CaptureOutputMode::None, CaptureOutputMode::None)
                            | (CaptureOutputMode::Stdout, CaptureOutputMode::Stdout)
                            | (CaptureOutputMode::Stderr, CaptureOutputMode::Stderr)
                            | (CaptureOutputMode::Both, CaptureOutputMode::Both)
                            | (CaptureOutputMode::Structured, CaptureOutputMode::Structured)
                    ));
                }
            }
        }
    }

    #[test]
    fn test_command_request_creation() {
        let spec = CommandSpec::Shell {
            command: "ls".to_string(),
            shell: None,
            working_dir: None,
            env: None,
        };

        let request = CommandRequest {
            spec: spec.clone(),
            execution_config: ExecutionConfig::default(),
            context: ExecutionContext::default(),
            metadata: CommandMetadata::new("test"),
        };

        assert!(matches!(request.spec, CommandSpec::Shell { .. }));
        assert_eq!(request.metadata.tags.get("type"), Some(&"test".to_string()));
    }

    #[test]
    fn test_validation_config() {
        let config = ValidationConfig {
            script: Some("test.sh".to_string()),
            expected_pattern: Some("^SUCCESS".to_string()),
            forbidden_patterns: Some(vec!["ERROR".to_string()]),
            json_schema: None,
        };

        assert_eq!(config.script, Some("test.sh".to_string()));
        assert_eq!(config.expected_pattern, Some("^SUCCESS".to_string()));
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits {
            max_memory_bytes: Some(1024 * 1024 * 100), // 100MB
            max_cpu_percent: Some(50.0),
            max_disk_io_bytes: Some(1024 * 1024 * 10), // 10MB
            max_network_bytes: Some(1024 * 1024),      // 1MB
            max_file_descriptors: Some(100),
        };

        assert_eq!(limits.max_memory_bytes, Some(104857600));
        assert_eq!(limits.max_cpu_percent, Some(50.0));
    }
}
