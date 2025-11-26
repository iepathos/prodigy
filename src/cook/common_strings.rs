//! Common strings interning for performance optimization
//!
//! Provides static string constants and interning for frequently used strings
//! to reduce memory allocations and improve comparison performance.

use once_cell::sync::Lazy;
use std::sync::Arc;

/// Common command names as static strings
pub mod commands {
    pub const CLAUDE: &str = "claude";
    pub const SHELL: &str = "shell";
    pub const TEST: &str = "test";
    pub const FOREACH: &str = "foreach";
    pub const HANDLER: &str = "handler";
    pub const BUILD: &str = "build";
    pub const VALIDATE: &str = "validate";
    pub const COMMIT: &str = "commit";
    pub const PUSH: &str = "push";
    pub const PULL: &str = "pull";
    pub const MERGE: &str = "merge";
    pub const CHECKOUT: &str = "checkout";
    pub const STATUS: &str = "status";
    pub const LOG: &str = "log";
}

/// Common variable names as static strings
pub mod variables {
    pub const SHELL_OUTPUT: &str = "shell.output";
    pub const SHELL_LAST_OUTPUT: &str = "shell.last_output";
    pub const SHELL_EXIT_CODE: &str = "shell.exit_code";
    pub const CLAUDE_OUTPUT: &str = "claude.output";
    pub const ITEM: &str = "item";
    pub const ITEM_ID: &str = "item.id";
    pub const ITEM_INDEX: &str = "item.index";
    pub const MAP_RESULTS: &str = "map.results";
    pub const MAP_SUCCESSFUL: &str = "map.successful";
    pub const MAP_FAILED: &str = "map.failed";
    pub const MAP_TOTAL: &str = "map.total";
    pub const REDUCE_OUTPUT: &str = "reduce.output";
    pub const WORKTREE: &str = "worktree";
    pub const WORKTREE_NAME: &str = "worktree.name";
    pub const WORKTREE_PATH: &str = "worktree.path";
    pub const SESSION_ID: &str = "session.id";
    pub const PROJECT_ROOT: &str = "project.root";
    pub const PROJECT_NAME: &str = "project.name";
}

/// Common path patterns as static strings
pub mod paths {
    pub const DOT_PRODIGY: &str = ".prodigy";
    pub const SESSION_STATE: &str = "session_state.json";
    pub const CHECKPOINT: &str = "checkpoint.json";
    pub const EVENTS: &str = "events";
    pub const DLQ: &str = "dlq";
    pub const WORKTREES: &str = "worktrees";
    pub const STATE: &str = "state";
    pub const MAPREDUCE: &str = "mapreduce";
    pub const JOBS: &str = "jobs";
}

/// Common status values as static strings
pub mod status {
    pub const IN_PROGRESS: &str = "InProgress";
    pub const COMPLETED: &str = "Completed";
    pub const FAILED: &str = "Failed";
    pub const PENDING: &str = "Pending";
    pub const RUNNING: &str = "Running";
    pub const SUCCESS: &str = "Success";
    pub const ERROR: &str = "Error";
    pub const SKIPPED: &str = "Skipped";
    pub const RETRY: &str = "Retry";
}

/// String interning for dynamic strings that are frequently used
pub struct StringInterner {
    cache: std::sync::RwLock<std::collections::HashMap<String, Arc<str>>>,
}

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Intern a string, returning an `Arc<str>` that can be cheaply cloned
    pub fn intern(&self, s: &str) -> Arc<str> {
        // Fast path: check if already interned with read lock
        {
            let cache = match self.cache.read() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    // If lock is poisoned, we can still access the data
                    poisoned.into_inner()
                }
            };
            if let Some(interned) = cache.get(s) {
                return Arc::clone(interned);
            }
        }

        // Slow path: intern the string with write lock
        let mut cache = match self.cache.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // If lock is poisoned, we can still access the data
                poisoned.into_inner()
            }
        };
        cache
            .entry(s.to_string())
            .or_insert_with(|| Arc::from(s))
            .clone()
    }

    /// Get the number of interned strings
    pub fn len(&self) -> usize {
        match self.cache.read() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// Check if the interner is empty
    pub fn is_empty(&self) -> bool {
        match self.cache.read() {
            Ok(guard) => guard.is_empty(),
            Err(poisoned) => poisoned.into_inner().is_empty(),
        }
    }

    /// Clear all interned strings
    pub fn clear(&self) {
        match self.cache.write() {
            Ok(mut guard) => guard.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Global string interner for command names
pub static COMMAND_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

/// Global string interner for variable names
pub static VARIABLE_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

/// Helper function to get or intern a command name
pub fn intern_command(name: &str) -> Arc<str> {
    // Check if it's a known static command first
    match name {
        commands::CLAUDE => Arc::from(commands::CLAUDE),
        commands::SHELL => Arc::from(commands::SHELL),
        commands::TEST => Arc::from(commands::TEST),
        commands::FOREACH => Arc::from(commands::FOREACH),
        commands::HANDLER => Arc::from(commands::HANDLER),
        commands::BUILD => Arc::from(commands::BUILD),
        commands::VALIDATE => Arc::from(commands::VALIDATE),
        commands::COMMIT => Arc::from(commands::COMMIT),
        _ => COMMAND_INTERNER.intern(name),
    }
}

/// Helper function to get or intern a variable name
pub fn intern_variable(name: &str) -> Arc<str> {
    // Check if it's a known static variable first
    match name {
        variables::SHELL_OUTPUT => Arc::from(variables::SHELL_OUTPUT),
        variables::SHELL_LAST_OUTPUT => Arc::from(variables::SHELL_LAST_OUTPUT),
        variables::SHELL_EXIT_CODE => Arc::from(variables::SHELL_EXIT_CODE),
        variables::CLAUDE_OUTPUT => Arc::from(variables::CLAUDE_OUTPUT),
        variables::ITEM => Arc::from(variables::ITEM),
        variables::ITEM_ID => Arc::from(variables::ITEM_ID),
        variables::ITEM_INDEX => Arc::from(variables::ITEM_INDEX),
        variables::MAP_RESULTS => Arc::from(variables::MAP_RESULTS),
        variables::MAP_SUCCESSFUL => Arc::from(variables::MAP_SUCCESSFUL),
        variables::MAP_FAILED => Arc::from(variables::MAP_FAILED),
        variables::MAP_TOTAL => Arc::from(variables::MAP_TOTAL),
        variables::REDUCE_OUTPUT => Arc::from(variables::REDUCE_OUTPUT),
        variables::WORKTREE => Arc::from(variables::WORKTREE),
        variables::WORKTREE_NAME => Arc::from(variables::WORKTREE_NAME),
        variables::WORKTREE_PATH => Arc::from(variables::WORKTREE_PATH),
        variables::SESSION_ID => Arc::from(variables::SESSION_ID),
        variables::PROJECT_ROOT => Arc::from(variables::PROJECT_ROOT),
        variables::PROJECT_NAME => Arc::from(variables::PROJECT_NAME),
        _ => VARIABLE_INTERNER.intern(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interning() {
        let interner = StringInterner::new();

        let s1 = interner.intern("test_string");
        let s2 = interner.intern("test_string");

        // Should return the same Arc
        assert!(Arc::ptr_eq(&s1, &s2));

        // Should have only one entry
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn test_command_interning_static() {
        let cmd1 = intern_command("claude");
        let cmd2 = intern_command("claude");

        // Should return the same static string
        assert_eq!(&*cmd1, commands::CLAUDE);
        assert_eq!(&*cmd2, commands::CLAUDE);
    }

    #[test]
    fn test_variable_interning_dynamic() {
        let var1 = intern_variable("custom.variable");
        let var2 = intern_variable("custom.variable");

        // Should be interned and point to same memory
        assert!(Arc::ptr_eq(&var1, &var2));
    }

    #[test]
    fn test_interner_clear() {
        let interner = StringInterner::new();
        interner.intern("test1");
        interner.intern("test2");
        assert_eq!(interner.len(), 2);

        interner.clear();
        assert_eq!(interner.len(), 0);
    }
}
