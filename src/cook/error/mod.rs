//! Error handling with context preservation
//!
//! This module provides error context preservation using Stillwater's ContextError type.
//! It allows errors to accumulate context as they propagate up the call stack, providing
//! comprehensive debugging information.

pub mod ext;

#[cfg(test)]
mod tests;

pub use ext::{ContextResult, ResultExt};
pub use stillwater::ContextError;
