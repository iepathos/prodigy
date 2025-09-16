// Main CLI integration test file
// This brings together all the CLI integration tests

mod cli_integration;

// Re-export all test modules so they run
use cli_integration::*;
