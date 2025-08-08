//! Built-in command handlers

pub mod cargo;
pub mod claude;
pub mod file;
pub mod git;
pub mod shell;

pub use cargo::CargoHandler;
pub use claude::ClaudeHandler;
pub use file::FileHandler;
pub use git::GitHandler;
pub use shell::ShellHandler;

/// Creates a vector of all built-in handlers
pub fn all_handlers() -> Vec<Box<dyn crate::commands::CommandHandler>> {
    vec![
        Box::new(ShellHandler::new()),
        Box::new(ClaudeHandler::new()),
        Box::new(GitHandler::new()),
        Box::new(CargoHandler::new()),
        Box::new(FileHandler::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_handlers() {
        let handlers = all_handlers();
        assert_eq!(handlers.len(), 5);

        let names: Vec<_> = handlers.iter().map(|h| h.name()).collect();
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"git"));
        assert!(names.contains(&"cargo"));
        assert!(names.contains(&"file"));
    }
}
