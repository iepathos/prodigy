//! Execution context management for command execution
//!
//! This module provides context types and conversions for command execution.

use std::collections::HashMap;
use std::path::PathBuf;

/// Context for command execution
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub worktree_path: PathBuf,
    pub worktree_name: String,
    pub item_id: String,
    pub variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub environment: HashMap<String, String>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(worktree_path: PathBuf, worktree_name: String, item_id: String) -> Self {
        Self {
            worktree_path,
            worktree_name,
            item_id,
            variables: HashMap::new(),
            captured_outputs: HashMap::new(),
            environment: HashMap::new(),
        }
    }

    /// Add a variable to the context
    pub fn with_variable(mut self, key: String, value: String) -> Self {
        self.variables.insert(key, value);
        self
    }

    /// Add multiple variables to the context
    pub fn with_variables(mut self, vars: HashMap<String, String>) -> Self {
        self.variables.extend(vars);
        self
    }

    /// Add captured output to the context
    pub fn with_captured_output(mut self, key: String, value: String) -> Self {
        self.captured_outputs.insert(key, value);
        self
    }

    /// Add environment variable to the context
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.environment.insert(key, value);
        self
    }

    /// Get a variable value by key
    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Get captured output by key
    pub fn get_captured_output(&self, key: &str) -> Option<&String> {
        self.captured_outputs.get(key)
    }

    /// Get environment variable by key
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.environment.get(key)
    }
}
