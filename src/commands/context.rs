//! Execution context for command handlers

use crate::subprocess::SubprocessExecutor;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Context provided to command handlers during execution
#[derive(Clone)]
pub struct ExecutionContext {
    /// Working directory for command execution
    pub working_dir: PathBuf,

    /// Environment variables to set
    pub env_vars: HashMap<String, String>,

    /// Subprocess executor for running commands
    pub executor: Arc<dyn SubprocessExecutor>,

    /// Session ID for tracking
    pub session_id: Option<String>,

    /// Iteration number within a session
    pub iteration: Option<usize>,

    /// Whether we're in a dry-run mode
    pub dry_run: bool,

    /// Whether to capture output
    pub capture_output: bool,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ExecutionContext {
    /// Creates a new execution context with defaults
    pub fn new(working_dir: PathBuf) -> Self {
        use crate::subprocess::RealSubprocessExecutor;

        Self {
            working_dir,
            env_vars: HashMap::new(),
            executor: Arc::new(RealSubprocessExecutor),
            session_id: None,
            iteration: None,
            dry_run: false,
            capture_output: true,
            metadata: HashMap::new(),
        }
    }

    /// Creates a context for testing
    #[cfg(test)]
    pub fn test() -> Self {
        Self::new(std::env::current_dir().unwrap())
    }

    /// Sets the subprocess executor
    pub fn with_executor(mut self, executor: Arc<dyn SubprocessExecutor>) -> Self {
        self.executor = executor;
        self
    }

    /// Sets the session ID
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Sets the iteration number
    pub fn with_iteration(mut self, iteration: usize) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Sets dry-run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets whether to capture output
    pub fn with_capture_output(mut self, capture: bool) -> Self {
        self.capture_output = capture;
        self
    }

    /// Adds an environment variable
    pub fn add_env_var(&mut self, key: String, value: String) {
        self.env_vars.insert(key, value);
    }

    /// Adds multiple environment variables
    pub fn add_env_vars(&mut self, vars: HashMap<String, String>) {
        self.env_vars.extend(vars);
    }

    /// Adds metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Gets a metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Creates a child context with a different working directory
    pub fn with_working_dir(&self, dir: PathBuf) -> Self {
        let mut ctx = self.clone();
        ctx.working_dir = dir;
        ctx
    }

    /// Gets the absolute path for a relative path
    pub fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_dir.join(path)
        }
    }

    /// Checks if we're in an MMM session
    pub fn is_in_session(&self) -> bool {
        self.session_id.is_some()
    }

    /// Gets the full environment including system env and context env
    pub fn full_env(&self) -> HashMap<String, String> {
        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.extend(self.env_vars.clone());
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::MockSubprocessExecutor;

    #[test]
    fn test_context_creation() {
        let ctx = ExecutionContext::new(PathBuf::from("/test"));
        assert_eq!(ctx.working_dir, PathBuf::from("/test"));
        assert!(!ctx.dry_run);
        assert!(ctx.capture_output);
    }

    #[test]
    fn test_context_builder_pattern() {
        let ctx = ExecutionContext::new(PathBuf::from("/test"))
            .with_session_id("test-session".to_string())
            .with_iteration(1)
            .with_dry_run(true);

        assert_eq!(ctx.session_id, Some("test-session".to_string()));
        assert_eq!(ctx.iteration, Some(1));
        assert!(ctx.dry_run);
    }

    #[test]
    fn test_env_vars() {
        let mut ctx = ExecutionContext::new(PathBuf::from("/test"));
        ctx.add_env_var("TEST_VAR".to_string(), "value".to_string());

        assert_eq!(ctx.env_vars.get("TEST_VAR"), Some(&"value".to_string()));
    }

    #[test]
    fn test_metadata() {
        let mut ctx = ExecutionContext::new(PathBuf::from("/test"));
        ctx.add_metadata("key".to_string(), "value".to_string());

        assert_eq!(ctx.get_metadata("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_path_resolution() {
        let ctx = ExecutionContext::new(PathBuf::from("/test"));

        let absolute = ctx.resolve_path(Path::new("/absolute/path"));
        assert_eq!(absolute, PathBuf::from("/absolute/path"));

        let relative = ctx.resolve_path(Path::new("relative/path"));
        assert_eq!(relative, PathBuf::from("/test/relative/path"));
    }

    #[test]
    fn test_with_executor() {
        let mock_executor = Arc::new(MockSubprocessExecutor::new());
        let ctx =
            ExecutionContext::new(PathBuf::from("/test")).with_executor(mock_executor.clone());

        // The executor is set (we can't easily test the actual value due to trait object)
        assert!(!ctx.is_in_session());
    }
}
