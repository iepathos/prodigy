//! Process management and lifecycle handling

use super::command::{CleanupRequirements, CommandType, ExecutableCommand, ResourceRequirements};
use super::executor::ExecutionContextInternal;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::Mutex;

/// Process ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessId(pub u32);

/// Process manager for unified command execution
pub struct ProcessManager {
    resource_monitor: Arc<super::executor::ResourceMonitor>,
    security_context: Arc<SecurityContext>,
    cleanup_registry: Arc<Mutex<HashMap<ProcessId, CleanupHandler>>>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            resource_monitor: Arc::new(super::executor::ResourceMonitor),
            security_context: Arc::new(SecurityContext),
            cleanup_registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_monitors(
        resource_monitor: Arc<super::executor::ResourceMonitor>,
        security_context: Arc<SecurityContext>,
    ) -> Self {
        Self {
            resource_monitor,
            security_context,
            cleanup_registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new process
    pub async fn spawn(
        &self,
        executable: ExecutableCommand,
        context: &ExecutionContextInternal,
    ) -> Result<UnifiedProcess> {
        // Security validation
        self.security_context.validate_command(&executable).await?;

        // Resource availability check
        self.resource_monitor
            .check_resources(executable.resource_requirements())
            .await?;

        // Create process with unified configuration
        let mut command = self.create_system_command(&executable, context)?;

        // Apply security restrictions
        command = self.apply_security_context(command, context)?;

        // Apply resource limits
        command = self.apply_resource_limits(command, executable.resource_requirements())?;

        // Spawn process
        let child = command
            .spawn()
            .with_context(|| format!("Failed to spawn command: {}", executable.display()))?;

        let process = UnifiedProcess::new(child, executable.command_type);

        // Register for cleanup
        let cleanup_handler =
            CleanupHandler::new(process.id(), executable.cleanup_requirements().clone());
        self.cleanup_registry
            .lock()
            .await
            .insert(process.id(), cleanup_handler);

        Ok(process)
    }

    /// Create system command from executable
    fn create_system_command(
        &self,
        executable: &ExecutableCommand,
        context: &ExecutionContextInternal,
    ) -> Result<Command> {
        let mut command = Command::new(&executable.program);

        // Add arguments
        command.args(&executable.args);

        // Set working directory
        if let Some(ref working_dir) = executable.working_dir {
            command.current_dir(working_dir);
        } else {
            command.current_dir(&context.request.context.working_dir);
        }

        // Set environment variables
        for (key, value) in &executable.env {
            command.env(key, value);
        }

        // Add context environment variables
        for (key, value) in &context.request.context.env_vars {
            command.env(key, value);
        }

        // Configure stdio based on command type
        match executable.command_type {
            CommandType::Claude => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::piped());
            }
            CommandType::Shell => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::null());
            }
            CommandType::Test => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::null());
            }
            CommandType::Handler => {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.stdin(Stdio::null());
            }
        }

        Ok(command)
    }

    /// Apply security context to command
    fn apply_security_context(
        &self,
        mut command: Command,
        _context: &ExecutionContextInternal,
    ) -> Result<Command> {
        // Set process group for cleanup
        #[cfg(unix)]
        {
            command.process_group(0);
        }

        // TODO: Add additional security restrictions
        // - User/group restrictions
        // - Capability dropping
        // - Seccomp filters
        // - Namespace isolation

        Ok(command)
    }

    /// Apply resource limits to command
    fn apply_resource_limits(
        &self,
        command: Command,
        _requirements: &ResourceRequirements,
    ) -> Result<Command> {
        // TODO: Apply resource limits
        // - Memory limits
        // - CPU limits
        // - File descriptor limits
        // - Process limits

        Ok(command)
    }

    /// Cleanup a process
    pub async fn cleanup_process(&self, process_id: ProcessId) -> Result<()> {
        if let Some(cleanup_handler) = self.cleanup_registry.lock().await.remove(&process_id) {
            cleanup_handler.cleanup().await?;
        }
        Ok(())
    }
}

/// Unified process wrapper
pub struct UnifiedProcess {
    child: Child,
    command_type: CommandType,
    started_at: Instant,
    resource_usage: ResourceUsage,
}

impl UnifiedProcess {
    pub fn new(child: Child, command_type: CommandType) -> Self {
        Self {
            child,
            command_type,
            started_at: Instant::now(),
            resource_usage: ResourceUsage::default(),
        }
    }

    /// Wait for process completion
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        let exit_status = self.child.wait().await?;
        self.resource_usage.duration = self.started_at.elapsed();
        Ok(exit_status)
    }

    /// Kill the process
    pub async fn kill(&mut self) -> Result<()> {
        // On Unix, kill the entire process group to ensure all child processes are terminated
        #[cfg(unix)]
        {
            if let Some(pid) = self.child.id() {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                
                // Try to kill the process group (negative PID)
                let pgid = Pid::from_raw(-(pid as i32));
                let _ = signal::kill(pgid, Signal::SIGTERM);
                
                // Give it a moment to terminate gracefully
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                
                // Force kill if still running
                if let Ok(None) = self.child.try_wait() {
                    let _ = signal::kill(pgid, Signal::SIGKILL);
                }
            }
        }
        
        // Always try the standard kill as well
        self.child.kill().await?;
        Ok(())
    }

    /// Get stdout handle
    pub fn stdout(&mut self) -> &mut Option<ChildStdout> {
        &mut self.child.stdout
    }

    /// Get stderr handle
    pub fn stderr(&mut self) -> &mut Option<ChildStderr> {
        &mut self.child.stderr
    }

    /// Get process ID
    pub fn id(&self) -> ProcessId {
        ProcessId(self.child.id().unwrap_or(0))
    }

    /// Get command type
    pub fn command_type(&self) -> CommandType {
        self.command_type
    }

    /// Get resource usage
    pub fn resource_usage(&self) -> &ResourceUsage {
        &self.resource_usage
    }
}

/// Resource usage tracking
#[derive(Debug, Default)]
pub struct ResourceUsage {
    pub duration: std::time::Duration,
    pub peak_memory: Option<u64>,
    pub cpu_usage: Option<f32>,
}

/// Cleanup handler for process termination
pub struct CleanupHandler {
    pub process_id: ProcessId,
    pub requirements: CleanupRequirements,
}

impl CleanupHandler {
    pub fn new(process_id: ProcessId, requirements: CleanupRequirements) -> Self {
        Self {
            process_id,
            requirements,
        }
    }

    /// Perform cleanup
    pub async fn cleanup(&self) -> Result<()> {
        // Kill process group if needed
        #[cfg(unix)]
        if self.requirements.cleanup_children {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            // Kill the process group (negative PID)
            let pgid = Pid::from_raw(-(self.process_id.0 as i32));
            // Try graceful termination first
            let _ = kill(pgid, Signal::SIGTERM);

            // Wait for graceful termination
            tokio::time::sleep(self.requirements.kill_timeout).await;

            // Force kill if still running
            let _ = kill(pgid, Signal::SIGKILL);
        }

        Ok(())
    }
}

/// Security context for process execution
pub struct SecurityContext;

impl SecurityContext {
    /// Validate command for security issues
    pub async fn validate_command(&self, executable: &ExecutableCommand) -> Result<()> {
        // Check for command injection attempts
        for arg in &executable.args {
            if arg.contains("$") || arg.contains("`") || arg.contains("$(") {
                // Allow these in specific cases (e.g., shell commands)
                if executable.command_type != CommandType::Shell {
                    anyhow::bail!("Potential command injection detected in arguments");
                }
            }
        }

        // Check for path traversal
        if let Some(ref working_dir) = executable.working_dir {
            let canonical = working_dir
                .canonicalize()
                .unwrap_or_else(|_| working_dir.clone());
            if canonical
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                anyhow::bail!("Path traversal detected in working directory");
            }
        }

        // Check for dangerous commands
        let dangerous_commands = [
            "rm", "dd", "mkfs", "format", "fdisk", "shutdown", "reboot", "kill", "pkill",
        ];

        if dangerous_commands.contains(&executable.program.as_str()) {
            // Allow in specific contexts
            match executable.command_type {
                CommandType::Shell | CommandType::Handler => {
                    // Log warning but allow
                    tracing::warn!("Potentially dangerous command: {}", executable.program);
                }
                _ => {
                    anyhow::bail!("Dangerous command not allowed: {}", executable.program);
                }
            }
        }

        Ok(())
    }
}
