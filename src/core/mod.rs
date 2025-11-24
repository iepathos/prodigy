//! Core business logic module with pure functions
//!
//! This module contains pure functions that implement business logic without any I/O operations.
//! Following the "functional core, imperative shell" pattern, all functions here:
//! - Take inputs and return outputs
//! - Have no side effects
//! - Don't perform file system, network, or database operations
//! - Are easily testable without mocks

pub mod config;
pub mod mapreduce;
pub mod orchestration;
pub mod session;
pub mod validation;
pub mod workflow;
