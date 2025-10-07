//! Cook orchestrator module
//!
//! Coordinates all cook operations using specialized components.

pub mod builder;
pub mod construction;
mod core;
mod normalization;
pub mod workflow_classifier;

// Re-export public types and traits from core
pub use core::{CookConfig, CookOrchestrator, DefaultCookOrchestrator, ExecutionEnvironment};
// Re-export builder
pub use builder::OrchestratorBuilder;
// Re-export construction helpers
pub use construction::{
    create_env_config, create_workflow_executor, create_workflow_state_base, generate_session_id,
    new_orchestrator, new_orchestrator_with_test_config,
};
