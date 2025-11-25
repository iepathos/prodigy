//! Pure execution planning module for workflow orchestration
//!
//! This module contains pure functions for planning workflow execution without
//! performing any I/O operations. Following the "functional core, imperative shell"
//! pattern, these functions:
//!
//! - Take inputs and return outputs deterministically
//! - Have no side effects (no file system, network, or database operations)
//! - Are easily testable without mocks
//! - Enable composable execution planning
//!
//! # Architecture
//!
//! The module is organized into three submodules:
//!
//! - `mode_detection`: Determines the execution mode from configuration
//! - `resource_allocation`: Calculates resource requirements for execution
//! - `execution_planning`: Composes mode detection and resource allocation into execution plans
//!
//! # Example
//!
//! ```ignore
//! use prodigy::core::orchestration::{plan_execution, ExecutionPlan, ExecutionMode};
//!
//! let config = CookConfig { /* ... */ };
//! let plan = plan_execution(&config);
//!
//! assert_eq!(plan.mode, ExecutionMode::MapReduce);
//! assert_eq!(plan.parallel_budget, 10);
//! ```

mod execution_planning;
mod mode_detection;
mod resource_allocation;

// Re-export main types and functions
pub use execution_planning::{plan_execution, ExecutionPlan, Phase, PhaseType};
pub use mode_detection::{detect_execution_mode, ExecutionMode};
pub use resource_allocation::{calculate_resources, ResourceRequirements};
