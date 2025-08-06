//! Mock implementations for testing
//!
//! This module provides comprehensive mock implementations for all external dependencies.

pub mod claude;
pub mod git;
pub mod subprocess;
pub mod fs;

pub use claude::*;
pub use git::*;
pub use subprocess::*;
pub use fs::*;