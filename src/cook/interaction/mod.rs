//! User interaction handling for cook operations
//!
//! Provides abstractions for prompts, progress display, and user input.

pub mod display;
pub mod prompts;

pub use display::{ProgressDisplay, ProgressDisplayImpl};
pub use prompts::{UserPrompter, UserPrompterImpl};
#[cfg(test)]
pub use tests::MockUserInteraction;

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
pub mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Mock implementation for testing
    pub struct MockUserInteraction {
        messages: Arc<Mutex<Vec<String>>>,
        yes_no_responses: Arc<Mutex<Vec<bool>>>,
        text_responses: Arc<Mutex<Vec<String>>>,
    }

    impl Default for MockUserInteraction {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockUserInteraction {
        pub fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
                yes_no_responses: Arc::new(Mutex::new(Vec::new())),
                text_responses: Arc::new(Mutex::new(Vec::new())),
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
                .push(format!("Prompt: {message}"));
            self.yes_no_responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No yes/no response configured"))
        }

        async fn prompt_text(&self, message: &str, default: Option<&str>) -> Result<String> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Text prompt: {message} (default: {default:?})"));
            self.text_responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No text response configured"))
        }

        fn display_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Info: {message}"));
        }

        fn display_warning(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Warning: {message}"));
        }

        fn display_error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Error: {message}"));
        }

        fn display_progress(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Progress: {message}"));
        }

        fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Spinner started: {message}"));
            Box::new(MockSpinnerHandle {
                messages: self.messages.clone(),
            })
        }

        fn display_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Success: {message}"));
        }
    }

    struct MockSpinnerHandle {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl SpinnerHandle for MockSpinnerHandle {
        fn update_message(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Spinner update: {message}"));
        }

        fn success(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Spinner success: {message}"));
        }

        fn fail(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("Spinner fail: {message}"));
        }
    }

    #[test]
    fn test_default_user_interaction_creation() {
        let interaction = DefaultUserInteraction::new();
        // Just verify it can be created
        interaction.display_info("Test message");
    }

    #[tokio::test]
    async fn test_mock_user_interaction() {
        let mock = MockUserInteraction::new();

        // Test display methods
        mock.display_info("Information message");
        mock.display_warning("Warning message");
        mock.display_error("Error message");
        mock.display_progress("Progress message");
        mock.display_success("Success message");

        let messages = mock.get_messages();
        assert_eq!(messages.len(), 5);
        assert!(messages[0].contains("Information message"));
        assert!(messages[1].contains("Warning message"));
        assert!(messages[2].contains("Error message"));
        assert!(messages[3].contains("Progress message"));
        assert!(messages[4].contains("Success message"));
    }

    #[tokio::test]
    async fn test_mock_prompts() {
        let mock = MockUserInteraction::new();

        // Test yes/no prompt
        mock.add_yes_no_response(true);
        let result = mock.prompt_yes_no("Continue?").await.unwrap();
        assert!(result);

        // Test text prompt
        mock.add_text_response("user input".to_string());
        let text = mock
            .prompt_text("Enter name:", Some("default"))
            .await
            .unwrap();
        assert_eq!(text, "user input");

        let messages = mock.get_messages();
        assert!(messages.iter().any(|m| m.contains("Continue?")));
        assert!(messages.iter().any(|m| m.contains("Enter name:")));
    }

    #[test]
    fn test_spinner_handle() {
        let mock = MockUserInteraction::new();
        let mut spinner = mock.start_spinner("Loading...");

        spinner.update_message("Still loading...");
        spinner.success("Done!");

        let messages = mock.get_messages();
        assert!(messages
            .iter()
            .any(|m| m.contains("Spinner started: Loading")));
        assert!(messages
            .iter()
            .any(|m| m.contains("Spinner update: Still loading")));
        assert!(messages.iter().any(|m| m.contains("Spinner success: Done")));
    }

    #[test]
    fn test_display_formatting() {
        let display = DefaultUserInteraction::new();

        // Test that messages are properly formatted
        display.display_info("Test info");
        display.display_warning("Test warning");
        display.display_error("Test error");
        display.display_progress("Test progress");
        display.display_success("Test success");

        // No panics means the test passes
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mock = MockUserInteraction::new();

        // Test missing yes/no response
        let result = mock.prompt_yes_no("Should fail").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No yes/no response"));

        // Test missing text response
        let result = mock.prompt_text("Should also fail", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No text response"));
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
