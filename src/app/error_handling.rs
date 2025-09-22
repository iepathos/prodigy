//! Error handling utilities
//!
//! This module provides centralized error handling for the application.

use tracing::error;

/// Handle fatal errors and exit with appropriate status code
pub fn handle_fatal_error(error: anyhow::Error) -> ! {
    use crate::error::ProdigyError;

    error!("Fatal error: {}", error);

    // Check if it's a ProdigyError for better handling
    let exit_code = if let Some(prodigy_err) = error.downcast_ref::<ProdigyError>() {
        // Use the user-friendly message for ProdigyError
        eprintln!("{}", prodigy_err.user_message());

        // Show developer message in debug mode
        if tracing::enabled!(tracing::Level::DEBUG) {
            eprintln!("\nDebug information:\n{}", prodigy_err.developer_message());
        }

        prodigy_err.exit_code()
    } else {
        // Fallback for non-ProdigyError errors
        eprintln!("Error: {error}");

        // Try to determine exit code based on error message
        if error.to_string().contains("No workflow ID provided")
            || error.to_string().contains("required")
            || error.to_string().contains("Please specify")
        {
            2 // ARGUMENT_ERROR
        } else {
            1 // GENERAL_ERROR
        }
    };

    std::process::exit(exit_code)
}
