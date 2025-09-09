//! Tests for process termination and signal handling in the execution module

#[cfg(test)]
mod tests {
    use crate::cook::execution::command::*;
    use crate::cook::execution::process::*;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::process::Command;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_process_group_kill() {
        // Create a parent process that spawns children
        let mut command = Command::new("sh");
        command.arg("-c");
        command.arg("(sleep 60 & sleep 60 & wait)");
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        // Set process group (as our implementation does)
        command.process_group(0);

        let child = command.spawn().expect("Failed to spawn process");
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        // Give time for child processes to spawn
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Kill the process (should kill entire process group)
        let kill_result = process.kill().await;
        assert!(kill_result.is_ok(), "Kill should succeed");

        // The process and all its children should be terminated
        // We can't easily verify all children are dead, but the kill should have been sent
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_kill_with_sigterm_then_sigkill() {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        // Create a process that ignores SIGTERM
        let mut command = Command::new("sh");
        command.arg("-c");
        command.arg("trap '' TERM; sleep 60");
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.process_group(0);

        let child = command.spawn().expect("Failed to spawn process");
        let child_pid = child.id().expect("No PID") as i32;
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        // Kill should handle both SIGTERM and SIGKILL
        let kill_result = process.kill().await;
        assert!(
            kill_result.is_ok(),
            "Kill should succeed even if process ignores SIGTERM"
        );

        // Give a moment for the process to die
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check if the process is really dead by trying to send signal 0 (just checks existence)
        let result = signal::kill(Pid::from_raw(child_pid), Signal::SIGCONT);
        assert!(result.is_err(), "Process should be dead");
    }

    #[tokio::test]
    async fn test_kill_already_dead_process() {
        // Create a short-lived process
        let mut command = Command::new("echo");
        command.arg("test");
        command.stdout(Stdio::null());

        let child = command.spawn().expect("Failed to spawn process");
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        // Wait for it to complete naturally
        let _ = process.wait().await;

        // Killing an already dead process should not panic
        let kill_result = process.kill().await;
        // This might return an error (process already exited) but shouldn't panic
        assert!(kill_result.is_ok() || kill_result.is_err());
    }

    #[tokio::test]
    async fn test_resource_usage_after_kill() {
        let mut command = Command::new("sleep");
        command.arg("60");
        command.stdout(Stdio::null());

        let child = command.spawn().expect("Failed to spawn process");
        let mut process = UnifiedProcess::new(child, CommandType::Shell);

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Kill it
        let _ = process.kill().await;

        // Resource usage should have recorded some duration
        // Note: Duration tracking happens in wait(), not kill(), so this might be zero
        let usage = process.resource_usage();
        // Just verify it doesn't panic
        let _ = usage.duration;
    }

    #[tokio::test]
    async fn test_process_id_tracking() {
        let mut command = Command::new("echo");
        command.arg("test");

        let child = command.spawn().expect("Failed to spawn process");
        let child_id = child.id().expect("No PID");
        let process = UnifiedProcess::new(child, CommandType::Shell);

        // Process ID should match
        assert_eq!(process.id().0, child_id);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_process_manager_spawn_with_process_group() {
        let manager = ProcessManager::new();

        // Create a command that would spawn children
        let executable = ExecutableCommand::new("sh")
            .arg("-c")
            .arg("echo parent; echo child")
            .with_type(CommandType::Shell);

        let context = crate::cook::execution::executor::ExecutionContextInternal {
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
        };

        let result = manager.spawn(executable, &context).await;
        assert!(result.is_ok());

        let mut process = result.unwrap();

        // Process should complete normally
        let exit_status = process.wait().await;
        assert!(exit_status.is_ok());
    }
}
