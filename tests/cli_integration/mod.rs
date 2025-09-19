// CLI Integration Tests for Prodigy
//
// These tests verify end-to-end CLI functionality by simulating
// command invocations and checking output, exit codes, and effects.

pub mod argument_parsing_tests;
pub mod batch_command_tests;
pub mod configuration_tests;
pub mod cook_command_tests;
pub mod dlq_command_tests;
pub mod dry_run_tests;
pub mod events_command_tests;
pub mod exec_command_tests;
pub mod resume_command_tests;
pub mod resume_integration_tests;
pub mod signal_handling_tests;
pub mod test_utils;
pub mod verbose_output_tests;
pub mod worktree_command_tests;
