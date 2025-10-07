//! Cook orchestrator module
//!
//! Coordinates all cook operations using specialized components.

pub mod builder;
mod core;

// Re-export public types and traits from core
pub use core::{CookConfig, CookOrchestrator, DefaultCookOrchestrator, ExecutionEnvironment};
// Re-export builder
pub use builder::OrchestratorBuilder;
