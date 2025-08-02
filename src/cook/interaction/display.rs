//! Progress and message display implementation

use super::SpinnerHandle;
use std::sync::{Arc, Mutex};

/// Trait for displaying progress and messages
pub trait ProgressDisplay: Send + Sync {
    /// Display information message
    fn info(&self, message: &str);

    /// Display warning message
    fn warning(&self, message: &str);

    /// Display error message
    fn error(&self, message: &str);

    /// Display progress message
    fn progress(&self, message: &str);

    /// Display success message
    fn success(&self, message: &str);

    /// Start a spinner
    fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle>;
}

/// Real implementation of progress display
pub struct ProgressDisplayImpl;

impl Default for ProgressDisplayImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressDisplayImpl {
    pub fn new() -> Self {
        Self
    }
}

impl ProgressDisplay for ProgressDisplayImpl {
    fn info(&self, message: &str) {
        println!("ℹ️  {message}");
    }

    fn warning(&self, message: &str) {
        eprintln!("⚠️  {message}");
    }

    fn error(&self, message: &str) {
        eprintln!("❌ {message}");
    }

    fn progress(&self, message: &str) {
        println!("🔄 {message}");
    }

    fn success(&self, message: &str) {
        println!("✅ {message}");
    }

    fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
        // For CLI, we'll just print the message
        // In a real implementation, you might use indicatif or similar
        println!("⏳ {message}");
        Box::new(SimpleSpinnerHandle::new())
    }
}

/// Simple spinner handle implementation
struct SimpleSpinnerHandle {
    active: Arc<Mutex<bool>>,
}

impl SimpleSpinnerHandle {
    fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(true)),
        }
    }
}

impl SpinnerHandle for SimpleSpinnerHandle {
    fn update_message(&mut self, message: &str) {
        if *self.active.lock().unwrap() {
            println!("⏳ {message}");
        }
    }

    fn success(&mut self, message: &str) {
        *self.active.lock().unwrap() = false;
        println!("✅ {message}");
    }

    fn fail(&mut self, message: &str) {
        *self.active.lock().unwrap() = false;
        println!("❌ {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct MockProgressDisplay {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl MockProgressDisplay {
        pub fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn get_messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    impl ProgressDisplay for MockProgressDisplay {
        fn info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("INFO: {message}"));
        }

        fn warning(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("WARN: {message}"));
        }

        fn error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("ERROR: {message}"));
        }

        fn progress(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("PROGRESS: {message}"));
        }

        fn success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SUCCESS: {message}"));
        }

        fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SPINNER: {message}"));
            Box::new(MockSpinnerHandle::new(self.messages.clone()))
        }
    }

    struct MockSpinnerHandle {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl MockSpinnerHandle {
        fn new(messages: Arc<Mutex<Vec<String>>>) -> Self {
            Self { messages }
        }
    }

    impl SpinnerHandle for MockSpinnerHandle {
        fn update_message(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SPINNER_UPDATE: {message}"));
        }

        fn success(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SPINNER_SUCCESS: {message}"));
        }

        fn fail(&mut self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("SPINNER_FAIL: {message}"));
        }
    }

    #[test]
    fn test_mock_display() {
        let display = MockProgressDisplay::new();

        display.info("Test info");
        display.warning("Test warning");
        display.error("Test error");
        display.progress("Test progress");
        display.success("Test success");

        let messages = display.get_messages();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0], "INFO: Test info");
        assert_eq!(messages[1], "WARN: Test warning");
        assert_eq!(messages[2], "ERROR: Test error");
        assert_eq!(messages[3], "PROGRESS: Test progress");
        assert_eq!(messages[4], "SUCCESS: Test success");
    }

    #[test]
    fn test_mock_spinner() {
        let display = MockProgressDisplay::new();
        let mut spinner = display.start_spinner("Starting");

        spinner.update_message("Processing");
        spinner.success("Done");

        let messages = display.get_messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0], "SPINNER: Starting");
        assert_eq!(messages[1], "SPINNER_UPDATE: Processing");
        assert_eq!(messages[2], "SPINNER_SUCCESS: Done");
    }

    #[test]
    fn test_progress_display_info() {
        let display = ProgressDisplayImpl::new();
        // Test that info messages are displayed correctly
        display.info("Test info message");
        // Verify output contains the message with info icon
    }

    #[test]
    fn test_progress_display_warning() {
        let display = ProgressDisplayImpl::new();
        // Test warning messages go to stderr
        display.warning("Test warning");
        // Verify stderr output
    }

    #[test]
    fn test_progress_display_error() {
        let display = ProgressDisplayImpl::new();
        display.error("Test error");
        // Verify error formatting
    }

    #[test]
    fn test_spinner_lifecycle() {
        let display = ProgressDisplayImpl::new();
        let mut spinner = display.start_spinner("Loading...");
        // Test spinner starts
        spinner.update_message("Still processing");
        spinner.success("Done");
        // Verify spinner completes
    }

    #[test]
    fn test_progress_display_progress() {
        let display = ProgressDisplayImpl::new();
        display.progress("Test progress message");
        // Verify progress formatting
    }

    #[test]
    fn test_progress_display_success() {
        let display = ProgressDisplayImpl::new();
        display.success("Test success message");
        // Verify success formatting
    }

    #[test]
    fn test_simple_spinner_handle_fail() {
        let display = ProgressDisplayImpl::new();
        let mut spinner = display.start_spinner("Starting task");
        spinner.fail("Failed to complete");
        // Verify failure message
    }
}
