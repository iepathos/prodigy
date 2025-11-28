//! Pure functions for checkpoint management
//!
//! This module contains pure functions (no I/O) for checkpoint operations,
//! following the "pure core, imperative shell" pattern. All functions here
//! are testable without async runtime or mocking.
//!
//! ## Organization
//!
//! - `preparation.rs` - Checkpoint creation and preparation
//! - `triggers.rs` - Checkpoint trigger predicates
//! - `state_transitions.rs` - Work item state machine
//! - `validation.rs` - Validation with error accumulation

pub mod preparation;
pub mod state_transitions;
pub mod triggers;
pub mod validation;

// Re-export commonly used items
pub use preparation::{calculate_integrity_hash, prepare_checkpoint, reset_in_progress_items};
pub use state_transitions::{transition_work_item, TransitionError, WorkItemEvent, WorkItemStatus};
pub use triggers::{should_checkpoint, CheckpointTriggerConfig};
pub use validation::{validate_checkpoint, CheckpointValidationError};
