//! User interaction handling for cook operations
//!
//! Provides abstractions for prompts, progress display, and user input.

pub mod display;
pub mod prompts;

pub use display::{ProgressDisplay, ProgressDisplayImpl};
pub use prompts::{UserPrompter, UserPrompterImpl};

use anyhow::Result;
use async_trait::async_trait;

/// Trait for user interaction
#[async_trait]
pub trait UserInteraction: Send + Sync {
    /// Prompt user for yes/no confirmation
    async fn prompt_yes_no(&self, message: &str) -> Result<bool>;

    /// Prompt user for text input
    async fn prompt_text(&self, message: &str, default: Option<&str>) -> Result<String>;

    /// Display information message
    fn display_info(&self, message: &str);

    /// Display warning message
    fn display_warning(&self, message: &str);

    /// Display error message
    fn display_error(&self, message: &str);

    /// Display progress
    fn display_progress(&self, message: &str);

    /// Start a spinner
    fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle>;

    /// Display success message
    fn display_success(&self, message: &str);
}

/// Handle for controlling a spinner
pub trait SpinnerHandle: Send + Sync {
    /// Update spinner message
    fn update_message(&mut self, message: &str);

    /// Stop spinner with success
    fn success(&mut self, message: &str);

    /// Stop spinner with failure
    fn fail(&mut self, message: &str);
}

/// Default implementation of user interaction
pub struct DefaultUserInteraction {
    prompter: UserPrompterImpl,
    display: ProgressDisplayImpl,
}

impl Default for DefaultUserInteraction {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultUserInteraction {
    pub fn new() -> Self {
        Self {
            prompter: UserPrompterImpl::new(),
            display: ProgressDisplayImpl::new(),
        }
    }
}

#[async_trait]
impl UserInteraction for DefaultUserInteraction {
    async fn prompt_yes_no(&self, message: &str) -> Result<bool> {
        self.prompter.prompt_yes_no(message).await
    }

    async fn prompt_text(&self, message: &str, default: Option<&str>) -> Result<String> {
        self.prompter.prompt_text(message, default).await
    }

    fn display_info(&self, message: &str) {
        self.display.info(message);
    }

    fn display_warning(&self, message: &str) {
        self.display.warning(message);
    }

    fn display_error(&self, message: &str) {
        self.display.error(message);
    }

    fn display_progress(&self, message: &str) {
        self.display.progress(message);
    }

    fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
        self.display.start_spinner(message)
    }

    fn display_success(&self, message: &str) {
        self.display.success(message);
    }
}

#[cfg(test)]
pub mod mocks {
    use super::*;
    use std::sync::Mutex;

    pub struct MockUserInteraction {
        pub yes_no_responses: Mutex<Vec<bool>>,
        pub text_responses: Mutex<Vec<String>>,
        pub messages: Mutex<Vec<String>>,
    }

    impl Default for MockUserInteraction {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockUserInteraction {
        pub fn new() -> Self {
            Self {
                yes_no_responses: Mutex::new(Vec::new()),
                text_responses: Mutex::new(Vec::new()),
                messages: Mutex::new(Vec::new()),
            }
        }

        pub fn add_yes_no_response(&self, response: bool) {
            self.yes_no_responses.lock().unwrap().push(response);
        }

        pub fn add_text_response(&self, response: String) {
            self.text_responses.lock().unwrap().push(response);
        }

        pub fn get_messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl UserInteraction for MockUserInteraction {
        async fn prompt_yes_no(&self, message: &str) -> Result<bool> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("PROMPT: {message}"));
            self.yes_no_responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }

        async fn prompt_text(&self, message: &str, _default: Option<&str>) -> Result<String> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("TEXT: {message}"));
            self.text_responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }

        fn display_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("INFO: {message}"));
        }

        fn display_warning(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("WARN: {message}"));
        }

        fn display_error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("ERROR: {message}"));
        }

        fn display_progress(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("PROGRESS: {message}"));
        }

        fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SPINNER: {message}"));
            Box::new(MockSpinnerHandle)
        }

        fn display_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SUCCESS: {message}"));
        }
    }

    pub struct MockSpinnerHandle;

    impl SpinnerHandle for MockSpinnerHandle {
        fn update_message(&mut self, _message: &str) {}
        fn success(&mut self, _message: &str) {}
        fn fail(&mut self, _message: &str) {}
    }
}
