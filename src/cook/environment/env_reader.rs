//! Environment Variable Reader Abstraction
//!
//! Provides a trait-based abstraction for reading environment variables,
//! enabling pure, testable code that doesn't rely on global mutable state.
//!
//! # Philosophy (Spec 189)
//!
//! Following Stillwater's "pure core, imperative shell" philosophy:
//! - Environment access is an effect that should be explicit
//! - Production code uses `RealEnvReader` which reads from `std::env`
//! - Tests use `stillwater::MockEnv` for isolated, deterministic testing
//! - No code should use `std::env::set_var` or `std::env::remove_var`
//!
//! # Usage
//!
//! ## Production Code
//!
//! ```no_run
//! use prodigy::cook::environment::env_reader::{EnvReader, RealEnvReader};
//!
//! fn process_with_config<E: EnvReader>(env_reader: &E) -> Result<(), std::env::VarError> {
//!     let api_key = env_reader.var("API_KEY")?;
//!     // Use api_key...
//!     Ok(())
//! }
//!
//! // In production, use RealEnvReader
//! let reader = RealEnvReader;
//! process_with_config(&reader).unwrap();
//! ```
//!
//! ## Test Code
//!
//! ```no_run
//! // In tests, use stillwater::MockEnv for isolated environment testing
//! // Example test structure (use in actual test files):
//! //
//! // use stillwater::MockEnv;
//! //
//! // fn test_process_with_config() {
//! //     let env = MockEnv::new()
//! //         .with_env("API_KEY", "test-key-123");
//! //
//! //     process_with_config(&env).unwrap();
//! // }
//! ```

use std::ffi::OsString;

/// Trait for reading environment variables
///
/// Abstracts environment variable access to enable testability and avoid
/// global mutable state. Implementations must be thread-safe.
pub trait EnvReader: Send + Sync {
    /// Read an environment variable as a String
    ///
    /// # Errors
    ///
    /// Returns `VarError::NotPresent` if the variable is not set,
    /// or `VarError::NotUnicode` if the value contains invalid UTF-8.
    fn var(&self, key: &str) -> Result<String, std::env::VarError>;

    /// Read an environment variable as an OsString
    ///
    /// Returns `None` if the variable is not set.
    fn var_os(&self, key: &str) -> Option<OsString>;
}

/// Production implementation that reads from std::env
///
/// This is a zero-cost abstraction - all methods directly delegate to `std::env`.
#[derive(Clone, Default, Debug)]
pub struct RealEnvReader;

impl EnvReader for RealEnvReader {
    fn var(&self, key: &str) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

// Stillwater's MockEnv already implements the interface we need for testing.
// Users should use `stillwater::MockEnv` directly in tests.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_env_reader_reads_actual_env() {
        let reader = RealEnvReader;

        // Set a test variable
        std::env::set_var("PRODIGY_TEST_ENV_READER", "test_value");

        // Read it through the abstraction
        let value = reader.var("PRODIGY_TEST_ENV_READER").unwrap();
        assert_eq!(value, "test_value");

        // Clean up
        std::env::remove_var("PRODIGY_TEST_ENV_READER");
    }

    #[test]
    fn test_real_env_reader_returns_error_for_missing() {
        let reader = RealEnvReader;

        match reader.var("PRODIGY_NONEXISTENT_VAR_12345") {
            Err(std::env::VarError::NotPresent) => {
                // Expected
            }
            other => panic!("Expected NotPresent error, got: {:?}", other),
        }
    }

    #[test]
    fn test_real_env_reader_var_os() {
        let reader = RealEnvReader;

        std::env::set_var("PRODIGY_TEST_OS_VAR", "test_value");

        let value = reader.var_os("PRODIGY_TEST_OS_VAR");
        assert_eq!(value, Some(OsString::from("test_value")));

        std::env::remove_var("PRODIGY_TEST_OS_VAR");
    }

    #[test]
    fn test_real_env_reader_var_os_missing() {
        let reader = RealEnvReader;

        let value = reader.var_os("PRODIGY_NONEXISTENT_VAR_12345");
        assert_eq!(value, None);
    }
}
