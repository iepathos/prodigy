//! Prodigy CLI Application Entry Point
//!
//! This is the main entry point for the Prodigy CLI application.
//! It provides a thin composition layer that:
//! - Parses CLI arguments
//! - Initializes the application
//! - Routes commands to their implementations
//! - Handles errors gracefully

use clap::Parser;
use tracing::error;

// Import the modularized components
use prodigy::app::{handle_fatal_error, initialize_app, AppConfig};
use prodigy::cli::{execute_command, Cli};

#[tokio::main]
async fn main() {
    // Parse command line arguments
    let cli = Cli::parse();

    // Create application configuration
    let app_config = match AppConfig::new(cli.verbose) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to initialize application configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize the application (logging, storage migration, etc.)
    if let Err(e) = initialize_app(app_config).await {
        error!("Application initialization failed: {}", e);
        // Continue anyway - most initialization failures are non-fatal
    }

    // Execute the requested command
    let result = execute_command(cli.command, cli.verbose).await;

    // Handle any errors that occurred during command execution
    if let Err(e) = result {
        handle_fatal_error(e);
    }
}
