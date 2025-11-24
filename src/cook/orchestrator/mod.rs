//! Cook orchestrator module
//!
//! Coordinates all cook operations using specialized components.
//!
//! ## Architecture
//!
//! The orchestrator uses Stillwater's Effect pattern for composable, testable workflows:
//!
//! - **Pure Core** (`pure`): Workflow classification, validation, decision logic (no I/O)
//! - **Effect Composition** (`effects`): Setup, execution as composable effects
//! - **Environment Injection** (`environment`): Dependency injection for testing
//!
//! See individual module documentation for details.

mod argument_processing;
pub mod builder;
pub mod construction;
mod core;
pub mod effects;
pub mod environment;
mod execution_pipeline;
mod normalization;
pub mod pure;
mod session_ops;
pub mod workflow_classifier;
mod workflow_execution;

#[cfg(test)]
mod core_tests;

// Re-export public types and traits from core
pub use core::{CookConfig, CookOrchestrator, DefaultCookOrchestrator, ExecutionEnvironment};
// Re-export builder
pub use builder::OrchestratorBuilder;
// Re-export construction helpers
pub use construction::{
    create_env_config, create_workflow_executor, create_workflow_state_base, generate_session_id,
    new_orchestrator, new_orchestrator_with_test_config,
};
// Re-export effect-based types
pub use effects::{
    execute_plan_effect, execute_workflow, finalize_session_effect, run_workflow_effect,
    setup_environment_effect, ExecutionEnvironment as EffectExecutionEnvironment, OrchEffect,
    StepContext, StepResult, WorkflowResult, WorkflowSession,
};
// Re-export environment
pub use environment::OrchestratorEnv;
// Re-export pure functions
pub use pure::{
    classify_workflow, validate_workflow as validate_workflow_pure, IterationDecision,
    WorkflowError, WorkflowType,
};
