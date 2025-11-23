//! Error handling utilities
//!
//! This module provides centralized error handling for the application.

use tracing::error;

/// Handle fatal errors and exit with appropriate status code
///
/// This function processes errors and displays them according to their type:
/// - For `ProdigyError`: Shows user message always, developer message in verbose mode
/// - For other errors: Shows error message and attempts to determine exit code
///
/// # Verbose Mode Behavior
/// - `verbose = 0`: User-friendly messages only
/// - `verbose >= 1`: Includes full developer context with error chain
pub fn handle_fatal_error(error: anyhow::Error, verbose: u8) -> ! {
    use crate::error::ProdigyError;

    error!("Fatal error: {}", error);

    // Check if it's a ProdigyError for better handling
    let exit_code = if let Some(prodigy_err) = error.downcast_ref::<ProdigyError>() {
        // Use the user-friendly message for ProdigyError
        eprintln!("{}", prodigy_err.user_message());

        // Show developer message with full context chain in verbose mode
        if verbose >= 1 {
            eprintln!("\nContext Chain:\n{}", prodigy_err.developer_message());
        }

        prodigy_err.exit_code()
    } else {
        // Fallback for non-ProdigyError errors
        eprintln!("Error: {error}");

        // Show chain in verbose mode for non-ProdigyError errors
        if verbose >= 1 {
            eprintln!("\nError chain:");
            for (i, cause) in error.chain().enumerate() {
                eprintln!("  {}: {}", i, cause);
            }
        }

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
