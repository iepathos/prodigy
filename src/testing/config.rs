//! Test configuration for dependency injection
//!
//! This module provides a configuration system for tests to avoid using
//! environment variables, enabling proper test isolation and parallel execution.

use std::collections::HashMap;
use std::sync::Arc;

/// Test configuration for dependency injection
///
/// This replaces environment variable usage in tests, providing:
/// - Thread-safe configuration
/// - Test isolation
/// - Type safety
/// - No global state
#[derive(Debug, Clone, Default)]
pub struct TestConfiguration {
    /// Enables test mode behavior
    pub test_mode: bool,

    /// Commands that should simulate no changes during tests
    pub no_changes_commands: Vec<String>,

    /// Skip commit validation in tests
    pub skip_commit_validation: bool,

    /// Enable focus tracking for tests
    pub track_focus: bool,

    /// Worktree name for worktree tests
    pub worktree_name: Option<String>,

    /// Additional arguments for flexible configuration
    pub additional_args: HashMap<String, String>,
}

impl TestConfiguration {
    /// Create a new test configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for constructing test configuration
    pub fn builder() -> TestConfigurationBuilder {
        TestConfigurationBuilder::default()
    }

    /// Check if test mode is enabled
    pub fn is_test_mode(&self) -> bool {
        self.test_mode
    }

    /// Check if a command should simulate no changes
    pub fn should_simulate_no_changes(&self, command: &str) -> bool {
        self.no_changes_commands.iter().any(|cmd| cmd == command)
    }

    /// Get a custom argument value
    pub fn get_arg(&self, key: &str) -> Option<&String> {
        self.additional_args.get(key)
    }

    /// Convert to Arc for sharing across threads
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

/// Builder for TestConfiguration
#[derive(Debug, Default)]
pub struct TestConfigurationBuilder {
    test_mode: Option<bool>,
    no_changes_commands: Option<Vec<String>>,
    skip_commit_validation: Option<bool>,
    track_focus: Option<bool>,
    worktree_name: Option<String>,
    additional_args: HashMap<String, String>,
}

impl TestConfigurationBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable test mode
    pub fn test_mode(mut self, enabled: bool) -> Self {
        self.test_mode = Some(enabled);
        self
    }

    /// Set commands that should simulate no changes
    pub fn no_changes_commands(mut self, commands: Vec<String>) -> Self {
        self.no_changes_commands = Some(commands);
        self
    }

    /// Add a single command that should simulate no changes
    pub fn add_no_changes_command(mut self, command: impl Into<String>) -> Self {
        let mut commands = self.no_changes_commands.unwrap_or_default();
        commands.push(command.into());
        self.no_changes_commands = Some(commands);
        self
    }

    /// Skip commit validation
    pub fn skip_commit_validation(mut self, skip: bool) -> Self {
        self.skip_commit_validation = Some(skip);
        self
    }

    /// Enable focus tracking
    pub fn track_focus(mut self, enabled: bool) -> Self {
        self.track_focus = Some(enabled);
        self
    }

    /// Set worktree name
    pub fn worktree_name(mut self, name: impl Into<String>) -> Self {
        self.worktree_name = Some(name.into());
        self
    }

    /// Add a custom argument
    pub fn add_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional_args.insert(key.into(), value.into());
        self
    }

    /// Build the configuration
    pub fn build(self) -> TestConfiguration {
        TestConfiguration {
            test_mode: self.test_mode.unwrap_or(false),
            no_changes_commands: self.no_changes_commands.unwrap_or_default(),
            skip_commit_validation: self.skip_commit_validation.unwrap_or(false),
            track_focus: self.track_focus.unwrap_or(false),
            worktree_name: self.worktree_name,
            additional_args: self.additional_args,
        }
    }
}

/// Common test configurations
pub struct TestConfigurations;

impl TestConfigurations {
    /// Standard test configuration with test mode enabled
    pub fn test_mode() -> TestConfiguration {
        TestConfiguration::builder().test_mode(true).build()
    }

    /// Configuration for testing with no-change commands
    pub fn with_no_changes(commands: Vec<String>) -> TestConfiguration {
        TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(commands)
            .build()
    }

    /// Configuration for worktree tests
    pub fn worktree(name: impl Into<String>) -> TestConfiguration {
        TestConfiguration::builder()
            .test_mode(true)
            .worktree_name(name)
            .build()
    }

    /// Configuration with all test features enabled
    pub fn full_test() -> TestConfiguration {
        TestConfiguration::builder()
            .test_mode(true)
            .skip_commit_validation(true)
            .track_focus(true)
            .build()
    }

    /// Production configuration (all test features disabled)
    pub fn production() -> TestConfiguration {
        TestConfiguration::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration() {
        let config = TestConfiguration::new();
        assert!(!config.test_mode);
        assert!(config.no_changes_commands.is_empty());
        assert!(!config.skip_commit_validation);
        assert!(!config.track_focus);
        assert!(config.worktree_name.is_none());
        assert!(config.additional_args.is_empty());
    }

    #[test]
    fn test_builder_with_all_options() {
        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec!["cmd1".to_string(), "cmd2".to_string()])
            .skip_commit_validation(true)
            .track_focus(true)
            .worktree_name("test-worktree")
            .add_arg("key", "value")
            .build();

        assert!(config.test_mode);
        assert_eq!(config.no_changes_commands.len(), 2);
        assert!(config.skip_commit_validation);
        assert!(config.track_focus);
        assert_eq!(config.worktree_name, Some("test-worktree".to_string()));
        assert_eq!(config.get_arg("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_should_simulate_no_changes() {
        let config = TestConfiguration::builder()
            .add_no_changes_command("test-cmd")
            .add_no_changes_command("another-cmd")
            .build();

        assert!(config.should_simulate_no_changes("test-cmd"));
        assert!(config.should_simulate_no_changes("another-cmd"));
        assert!(!config.should_simulate_no_changes("unknown-cmd"));
    }

    #[test]
    fn test_common_configurations() {
        let test_mode = TestConfigurations::test_mode();
        assert!(test_mode.test_mode);

        let with_no_changes = TestConfigurations::with_no_changes(vec!["cmd".to_string()]);
        assert!(with_no_changes.test_mode);
        assert_eq!(with_no_changes.no_changes_commands.len(), 1);

        let worktree = TestConfigurations::worktree("wt-test");
        assert!(worktree.test_mode);
        assert_eq!(worktree.worktree_name, Some("wt-test".to_string()));

        let full_test = TestConfigurations::full_test();
        assert!(full_test.test_mode);
        assert!(full_test.skip_commit_validation);
        assert!(full_test.track_focus);

        let production = TestConfigurations::production();
        assert!(!production.test_mode);
    }

    #[test]
    fn test_into_arc() {
        let config = TestConfiguration::builder().test_mode(true).build();

        let arc_config = config.into_arc();
        assert!(arc_config.test_mode);
    }
}
