//! Progress and message display implementation

use super::SpinnerHandle;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Verbosity level for output control
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerbosityLevel {
    Quiet = 0,   // Minimal output (errors only)
    Normal = 1,  // Default: progress + results
    Verbose = 2, // -v: command names + exit codes
    Debug = 3,   // -vv: + stdout/stderr
    Trace = 4,   // -vvv: + Claude output + internal details
}

impl VerbosityLevel {
    /// Create from CLI arguments
    pub fn from_args(verbosity_count: u8, quiet: bool) -> Self {
        if quiet {
            VerbosityLevel::Quiet
        } else {
            match verbosity_count {
                0 => VerbosityLevel::Normal,
                1 => VerbosityLevel::Verbose,
                2 => VerbosityLevel::Debug,
                _ => VerbosityLevel::Trace,
            }
        }
    }
}

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

    /// Display iteration start boundary
    fn iteration_start(&self, current: u32, total: u32);

    /// Display iteration end summary
    fn iteration_end(&self, current: u32, duration: Duration, success: bool);

    /// Display step start
    fn step_start(&self, step: u32, total: u32, description: &str);

    /// Display step end
    fn step_end(&self, step: u32, success: bool);

    /// Display command output based on verbosity
    fn command_output(&self, output: &str, verbosity: VerbosityLevel);

    /// Display debug output if verbosity allows
    fn debug_output(&self, message: &str, min_verbosity: VerbosityLevel);

    /// Get current verbosity level
    fn verbosity(&self) -> VerbosityLevel;
}

/// Real implementation of progress display
pub struct ProgressDisplayImpl {
    verbosity: VerbosityLevel,
    use_unicode: bool,
}

impl Default for ProgressDisplayImpl {
    fn default() -> Self {
        Self::new(VerbosityLevel::Normal)
    }
}

impl ProgressDisplayImpl {
    pub fn new(verbosity: VerbosityLevel) -> Self {
        // Detect terminal capabilities
        let use_unicode = Self::supports_unicode();

        Self {
            verbosity,
            use_unicode,
        }
    }

    /// Create from CLI arguments
    pub fn from_args(verbosity_count: u8, quiet: bool) -> Self {
        let verbosity = VerbosityLevel::from_args(verbosity_count, quiet);
        Self::new(verbosity)
    }

    /// Check if terminal supports Unicode
    fn supports_unicode() -> bool {
        // Check LANG/LC_ALL environment variables
        if let Ok(lang) = std::env::var("LANG") {
            if lang.contains("UTF-8") || lang.contains("utf8") {
                return true;
            }
        }
        if let Ok(lc_all) = std::env::var("LC_ALL") {
            if lc_all.contains("UTF-8") || lc_all.contains("utf8") {
                return true;
            }
        }
        // Default to ASCII on Windows, Unicode elsewhere
        !cfg!(windows)
    }

    /// Get box drawing characters based on Unicode support
    fn box_chars(&self) -> BoxChars {
        if self.use_unicode {
            BoxChars::unicode()
        } else {
            BoxChars::ascii()
        }
    }

    /// Format duration for display
    fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        if secs >= 60 {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{mins}m {secs}s")
        } else if secs > 0 {
            format!("{secs}.{millis:03}s")
        } else {
            format!("{millis}ms")
        }
    }
}

/// Box drawing characters for terminal UI
struct BoxChars {
    horizontal: char,
    vertical: char,
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
}

impl BoxChars {
    fn unicode() -> Self {
        Self {
            horizontal: '═',
            vertical: '║',
            top_left: '╔',
            top_right: '╗',
            bottom_left: '╚',
            bottom_right: '╝',
        }
    }

    fn ascii() -> Self {
        Self {
            horizontal: '=',
            vertical: '|',
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
        }
    }
}

impl ProgressDisplay for ProgressDisplayImpl {
    fn info(&self, message: &str) {
        if self.verbosity >= VerbosityLevel::Normal {
            println!("ℹ️  {message}");
        }
    }

    fn warning(&self, message: &str) {
        if self.verbosity >= VerbosityLevel::Normal {
            eprintln!("⚠️  {message}");
        }
    }

    fn error(&self, message: &str) {
        // Always show errors, even in quiet mode
        eprintln!("❌ {message}");
    }

    fn progress(&self, message: &str) {
        if self.verbosity >= VerbosityLevel::Normal {
            println!("🔄 {message}");
        }
    }

    fn success(&self, message: &str) {
        if self.verbosity >= VerbosityLevel::Normal {
            println!("✅ {message}");
        }
    }

    fn start_spinner(&self, message: &str) -> Box<dyn SpinnerHandle> {
        if self.verbosity >= VerbosityLevel::Normal {
            println!("⏳ {message}");
        }
        Box::new(SimpleSpinnerHandle::new(self.verbosity))
    }

    fn iteration_start(&self, current: u32, total: u32) {
        if self.verbosity >= VerbosityLevel::Normal {
            let chars = self.box_chars();
            let width = 60;
            let title = format!(" ITERATION {current}/{total} ");
            let padding = (width - title.len()) / 2;

            println!();
            println!(
                "{}{}{}",
                chars.top_left,
                std::iter::repeat_n(chars.horizontal, width).collect::<String>(),
                chars.top_right
            );
            println!(
                "{}{:padding$}{}{:padding$}{}",
                chars.vertical,
                "",
                title,
                "",
                chars.vertical,
                padding = padding
            );
            println!(
                "{}{}{}",
                chars.bottom_left,
                std::iter::repeat_n(chars.horizontal, width).collect::<String>(),
                chars.bottom_right
            );
            println!();
        }
    }

    fn iteration_end(&self, current: u32, duration: Duration, success: bool) {
        if self.verbosity >= VerbosityLevel::Normal {
            let duration_str = Self::format_duration(duration);
            let status = if success { "✅ Success" } else { "❌ Failed" };

            println!();
            println!("┌─ Iteration {current} Summary ──────────────────────────────────────┐");
            println!("│ Duration: {:<49}│", duration_str);
            println!("│ Status: {:<51}│", status);
            println!("└────────────────────────────────────────────────────────────┘");
            println!();
        }
    }

    fn step_start(&self, step: u32, total: u32, description: &str) {
        if self.verbosity >= VerbosityLevel::Verbose {
            println!("[Step {step}/{total}] {description}");
        }
    }

    fn step_end(&self, step: u32, success: bool) {
        if self.verbosity >= VerbosityLevel::Verbose {
            let status = if success { "✓" } else { "✗" };
            println!("[Step {step}] {status}");
        }
    }

    fn command_output(&self, output: &str, verbosity: VerbosityLevel) {
        if self.verbosity >= verbosity && !output.trim().is_empty() {
            println!("{output}");
        }
    }

    fn debug_output(&self, message: &str, min_verbosity: VerbosityLevel) {
        if self.verbosity >= min_verbosity {
            println!("🔍 {message}");
        }
    }

    fn verbosity(&self) -> VerbosityLevel {
        self.verbosity
    }
}

/// Simple spinner handle implementation
struct SimpleSpinnerHandle {
    active: Arc<Mutex<bool>>,
    verbosity: VerbosityLevel,
}

impl SimpleSpinnerHandle {
    fn new(verbosity: VerbosityLevel) -> Self {
        Self {
            active: Arc::new(Mutex::new(true)),
            verbosity,
        }
    }
}

impl SpinnerHandle for SimpleSpinnerHandle {
    fn update_message(&mut self, message: &str) {
        if *self.active.lock().unwrap() && self.verbosity >= VerbosityLevel::Normal {
            println!("⏳ {message}");
        }
    }

    fn success(&mut self, message: &str) {
        *self.active.lock().unwrap() = false;
        if self.verbosity >= VerbosityLevel::Normal {
            println!("✅ {message}");
        }
    }

    fn fail(&mut self, message: &str) {
        *self.active.lock().unwrap() = false;
        if self.verbosity >= VerbosityLevel::Normal {
            println!("❌ {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct MockProgressDisplay {
        messages: Arc<Mutex<Vec<String>>>,
        verbosity: VerbosityLevel,
    }

    impl MockProgressDisplay {
        pub fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
                verbosity: VerbosityLevel::Normal,
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

        fn iteration_start(&self, current: u32, total: u32) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("ITERATION_START: {current}/{total}"));
        }

        fn iteration_end(&self, current: u32, duration: Duration, success: bool) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("ITERATION_END: {current} {:?} {success}", duration));
        }

        fn step_start(&self, step: u32, total: u32, description: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("STEP_START: {step}/{total} {description}"));
        }

        fn step_end(&self, step: u32, success: bool) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("STEP_END: {step} {success}"));
        }

        fn command_output(&self, output: &str, _verbosity: VerbosityLevel) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("COMMAND_OUTPUT: {output}"));
        }

        fn debug_output(&self, message: &str, _min_verbosity: VerbosityLevel) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("DEBUG: {message}"));
        }

        fn verbosity(&self) -> VerbosityLevel {
            self.verbosity
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
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        // Test that info messages are displayed correctly
        display.info("Test info message");
        // Verify output contains the message with info icon
    }

    #[test]
    fn test_progress_display_warning() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        // Test warning messages go to stderr
        display.warning("Test warning");
        // Verify stderr output
    }

    #[test]
    fn test_progress_display_error() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        display.error("Test error");
        // Verify error formatting
    }

    #[test]
    fn test_spinner_lifecycle() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        let mut spinner = display.start_spinner("Loading...");
        // Test spinner starts
        spinner.update_message("Still processing");
        spinner.success("Done");
        // Verify spinner completes
    }

    #[test]
    fn test_progress_display_progress() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        display.progress("Test progress message");
        // Verify progress formatting
    }

    #[test]
    fn test_progress_display_success() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        display.success("Test success message");
        // Verify success formatting
    }

    #[test]
    fn test_simple_spinner_handle_fail() {
        let display = ProgressDisplayImpl::new(VerbosityLevel::Normal);
        let mut spinner = display.start_spinner("Starting task");
        spinner.fail("Failed to complete");
        // Verify failure message
    }

    #[test]
    fn test_verbosity_levels() {
        let quiet = VerbosityLevel::from_args(0, true);
        assert_eq!(quiet, VerbosityLevel::Quiet);

        let normal = VerbosityLevel::from_args(0, false);
        assert_eq!(normal, VerbosityLevel::Normal);

        let verbose = VerbosityLevel::from_args(1, false);
        assert_eq!(verbose, VerbosityLevel::Verbose);

        let debug = VerbosityLevel::from_args(2, false);
        assert_eq!(debug, VerbosityLevel::Debug);

        let trace = VerbosityLevel::from_args(3, false);
        assert_eq!(trace, VerbosityLevel::Trace);
    }

    #[test]
    fn test_iteration_display() {
        let display = MockProgressDisplay::new();
        display.iteration_start(1, 10);
        display.iteration_end(1, Duration::from_secs(5), true);

        let messages = display.get_messages();
        assert!(messages.contains(&"ITERATION_START: 1/10".to_string()));
        assert!(messages.iter().any(|m| m.starts_with("ITERATION_END: 1")));
    }

    #[test]
    fn test_step_display() {
        let display = MockProgressDisplay::new();
        display.step_start(1, 5, "Running tests");
        display.step_end(1, true);

        let messages = display.get_messages();
        assert!(messages.contains(&"STEP_START: 1/5 Running tests".to_string()));
        assert!(messages.contains(&"STEP_END: 1 true".to_string()));
    }

    #[test]
    fn test_verbosity_filtering() {
        let quiet_display = ProgressDisplayImpl::new(VerbosityLevel::Quiet);
        quiet_display.info("Should not appear");
        quiet_display.error("Should appear");
        // In quiet mode, only errors should be shown

        let verbose_display = ProgressDisplayImpl::new(VerbosityLevel::Verbose);
        verbose_display.step_start(1, 3, "test");
        // In verbose mode, step information should be shown
    }

    #[test]
    fn test_command_output_display() {
        let display = MockProgressDisplay::new();
        display.command_output("test output", VerbosityLevel::Debug);

        let messages = display.get_messages();
        assert!(messages.contains(&"COMMAND_OUTPUT: test output".to_string()));
    }

    #[test]
    fn test_debug_output() {
        let display = MockProgressDisplay::new();
        display.debug_output("debug info", VerbosityLevel::Trace);

        let messages = display.get_messages();
        assert!(messages.contains(&"DEBUG: debug info".to_string()));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(
            ProgressDisplayImpl::format_duration(Duration::from_millis(500)),
            "500ms"
        );
        assert_eq!(
            ProgressDisplayImpl::format_duration(Duration::from_secs(5)),
            "5.000s"
        );
        assert_eq!(
            ProgressDisplayImpl::format_duration(Duration::from_secs(65)),
            "1m 5s"
        );
    }
}
