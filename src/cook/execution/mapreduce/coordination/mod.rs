//! Execution coordination for MapReduce operations
//!
//! This module orchestrates the execution of MapReduce jobs,
//! coordinating between different phases and managing resources.

pub mod executor;
pub mod orchestrator;
pub mod scheduler;

// Re-export main types
pub use executor::MapReduceCoordinator;
pub use orchestrator::PhaseOrchestrator;
pub use scheduler::{SchedulingStrategy, WorkScheduler};
