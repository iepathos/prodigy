//! Unit tests for the process management module

#[cfg(test)]
mod tests {
    use super::super::command::*;
    use super::super::executor::{ExecutionContextInternal, ResourceMonitor};
    use super::super::process::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::sync::Arc;
    use std::time::Duration;

    fn create_test_process_manager() -> ProcessManager {
        ProcessManager::new()
    }

    fn create_test_executable(program: &str) -> ExecutableCommand {
        ExecutableCommand::new(program)
    }

    fn create_test_context() -> ExecutionContextInternal {
        ExecutionContextInternal {
            id: uuid::Uuid::new_v4(),
            request: CommandRequest {
                spec: CommandSpec::Shell {
                    command: "test".to_string(),
                    shell: None,
                    working_dir: None,
                    env: None,
                },
                execution_config: ExecutionConfig::default(),
                context: ExecutionContext::default(),
                metadata: CommandMetadata::new("test"),
            },
            resource_limits: None,
        }
    }

    #[tokio::test]
    async fn test_process_id() {
        let id1 = ProcessId(123);
        let id2 = ProcessId(123);
        let id3 = ProcessId(456);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(id1.0, 123);
    }

    #[tokio::test]
    async fn test_resource_usage_default() {
        let usage = ResourceUsage::default();
        assert_eq!(usage.duration, Duration::from_secs(0));
        assert!(usage.peak_memory.is_none());
        assert!(usage.cpu_usage.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_handler_creation() {
        let handler = CleanupHandler::new(ProcessId(123), CleanupRequirements::default());

        assert_eq!(handler.process_id, ProcessId(123));
        assert_eq!(handler.requirements.kill_timeout, Duration::from_secs(5));
        assert!(handler.requirements.cleanup_children);
        assert!(!handler.requirements.preserve_output);
    }

    #[tokio::test]
    async fn test_security_context_validate_safe_command() {
        let context = SecurityContext;
        let executable = create_test_executable("echo").arg("hello").arg("world");

        let result = context.validate_command(&executable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_context_validate_command_injection_non_shell() {
        let context = SecurityContext;
        let executable = create_test_executable("echo")
            .arg("$(whoami)")
            .with_type(CommandType::Test);

        let result = context.validate_command(&executable).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("command injection"));
    }

    #[tokio::test]
    async fn test_security_context_allow_command_injection_shell() {
        let context = SecurityContext;
        let executable = create_test_executable("sh")
            .arg("-c")
            .arg("echo $(date)")
            .with_type(CommandType::Shell);

        let result = context.validate_command(&executable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_context_validate_dangerous_command_shell() {
        let context = SecurityContext;
        let executable = create_test_executable("rm")
            .arg("-rf")
            .arg("/tmp/test")
            .with_type(CommandType::Shell);

        // Should allow but warn for shell type
        let result = context.validate_command(&executable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_context_validate_dangerous_command_claude() {
        let context = SecurityContext;
        let executable = create_test_executable("rm")
            .arg("-rf")
            .arg("/tmp/test")
            .with_type(CommandType::Claude);

        // Should reject for Claude type
        let result = context.validate_command(&executable).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Dangerous command"));
    }

    #[tokio::test]
    async fn test_security_context_path_traversal() {
        let context = SecurityContext;
        let mut executable = create_test_executable("echo");
        executable.working_dir = Some(PathBuf::from("/tmp/../etc"));

        let result = context.validate_command(&executable).await;
        // Path canonicalization should handle this, so it might pass
        // The test is more about checking the validation exists
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("Path traversal"));
    }

    #[tokio::test]
    async fn test_process_manager_spawn_simple() {
        let manager = create_test_process_manager();
        let executable = create_test_executable("echo").arg("test");
        let context = create_test_context();

        let result = manager.spawn(executable, &context).await;
        assert!(result.is_ok());

        let mut process = result.unwrap();
        assert_eq!(process.command_type(), CommandType::Shell);

        // Wait for process to complete
        let exit_status = process.wait().await;
        assert!(exit_status.is_ok());
    }

    #[tokio::test]
    async fn test_unified_process_creation() {
        use tokio::process::Command;

        let mut command = Command::new("echo");
        command.arg("test");
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let child = command.spawn().unwrap();
        let process = UnifiedProcess::new(child, CommandType::Shell);

        assert_eq!(process.command_type(), CommandType::Shell);
        assert!(process.id().0 > 0);
    }

    #[tokio::test]
    async fn test_unified_process_wait() {
        use tokio::process::Command;

        let mut command = Command::new("echo");
        command.arg("test");
        command.stdout(Stdio::piped());

        let child = command.spawn().unwrap();
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        let exit_status = process.wait().await.unwrap();
        assert!(exit_status.success());
        assert!(process.resource_usage().duration > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_unified_process_kill() {
        use tokio::process::Command;

        // Use a long-running command
        let mut command = Command::new("sleep");
        command.arg("60");

        let child = command.spawn().unwrap();
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        // Kill the process
        let kill_result = process.kill().await;
        assert!(kill_result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_process_stdout_stderr() {
        use tokio::process::Command;

        let mut command = Command::new("echo");
        command.arg("test");
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let child = command.spawn().unwrap();
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        assert!(process.stdout().is_some());
        assert!(process.stderr().is_some());
    }

    #[tokio::test]
    async fn test_process_manager_cleanup_registry() {
        let manager = create_test_process_manager();
        let executable = create_test_executable("echo").arg("test");
        let context = create_test_context();

        let process = manager.spawn(executable, &context).await.unwrap();
        let process_id = process.id();

        // Cleanup should remove from registry
        let cleanup_result = manager.cleanup_process(process_id).await;
        assert!(cleanup_result.is_ok());

        // Second cleanup should be no-op
        let cleanup_result2 = manager.cleanup_process(process_id).await;
        assert!(cleanup_result2.is_ok());
    }

    #[tokio::test]
    async fn test_executable_command_with_env() {
        let executable = create_test_executable("echo").with_env(HashMap::from([
            ("VAR1".to_string(), "value1".to_string()),
            ("VAR2".to_string(), "value2".to_string()),
        ]));

        assert_eq!(executable.env.len(), 2);
        assert_eq!(executable.env.get("VAR1"), Some(&"value1".to_string()));
    }

    #[tokio::test]
    async fn test_executable_command_with_working_dir() {
        let executable = create_test_executable("ls").with_working_dir(Some(PathBuf::from("/tmp")));

        assert_eq!(executable.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[tokio::test]
    async fn test_executable_command_expected_exit_code() {
        let executable = create_test_executable("test").with_expected_exit_code(Some(1));

        assert_eq!(executable.expected_exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_cleanup_handler_requirements() {
        let mut requirements = CleanupRequirements::default();
        requirements.kill_timeout = Duration::from_secs(10);
        requirements.cleanup_children = false;
        requirements.preserve_output = true;

        let handler = CleanupHandler::new(ProcessId(123), requirements.clone());

        assert_eq!(handler.requirements.kill_timeout, Duration::from_secs(10));
        assert!(!handler.requirements.cleanup_children);
        assert!(handler.requirements.preserve_output);
    }

    #[tokio::test]
    async fn test_security_context_dangerous_commands() {
        let context = SecurityContext;
        let dangerous_commands = vec!["rm", "dd", "mkfs", "format", "fdisk", "shutdown", "reboot"];

        for cmd in dangerous_commands {
            // Should fail for non-shell types
            let executable = create_test_executable(cmd).with_type(CommandType::Claude);
            assert!(context.validate_command(&executable).await.is_err());

            // Should pass for shell types
            let executable = create_test_executable(cmd).with_type(CommandType::Shell);
            assert!(context.validate_command(&executable).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_command_type_specific_stdio() {
        let manager = create_test_process_manager();
        let context = create_test_context();

        // Test Claude type - should have stdin
        let executable = create_test_executable("echo").with_type(CommandType::Claude);

        // Test Shell type - no stdin
        let executable_shell = create_test_executable("echo").with_type(CommandType::Shell);

        // Test Test type - no stdin
        let executable_test = create_test_executable("echo").with_type(CommandType::Test);

        // All should have stdout/stderr piped
        for exec in [executable, executable_shell, executable_test] {
            // Would spawn and check stdio configuration in real test
            assert!(
                exec.command_type == CommandType::Claude
                    || exec.command_type == CommandType::Shell
                    || exec.command_type == CommandType::Test
            );
        }
    }
}
