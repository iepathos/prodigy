//! Checkpoint effects for I/O operations
//!
//! This module contains Effect-based I/O operations for checkpoint management,
//! following the "pure core, imperative shell" pattern.
//!
//! ## Organization
//!
//! - `storage.rs` - Checkpoint save/load effects
//! - `signals.rs` - Signal handling for graceful shutdown

pub mod signals;
pub mod storage;

// Re-export commonly used items
pub use signals::{
    save_checkpoint_on_shutdown, shutdown_signal, CheckpointOnShutdown, ShutdownSignal,
};
pub use storage::{
    load_checkpoint, load_checkpoint_effect, save_checkpoint, save_checkpoint_effect,
    should_save_checkpoint, update_checkpoint_state, CheckpointStorageEnv, CheckpointStorageError,
};
